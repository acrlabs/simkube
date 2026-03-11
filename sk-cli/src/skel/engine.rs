use anyhow::bail;
use json_patch_ext::Index;
use json_patch_ext::prelude::*;
use metrics::counter;
use regex::Regex;
use serde_json::Value;
use sk_core::prelude::*;
use sk_store::TraceEvent;

use crate::skel::ast::{
    Command,
    CommandAction,
    Conditional,
    Rhs,
    TestOperation,
    TraceSelector,
    VarDef,
};
use crate::skel::context::*;
use crate::skel::errors::SkelError;
use crate::skel::metric_names::*;

pub(super) fn process_event(cmd: &Command, mut evt: TraceEvent) -> anyhow::Result<TraceEvent> {
    // Reinterpret the event as a JSON object so JSON pointers work for all fields
    // This seems sortof inefficient, we'll have to see how it works in practice.
    let mut applied_objs = serde_json::to_value(evt.applied_objs)?;
    let mut deleted_objs = serde_json::to_value(evt.deleted_objs)?;

    // Sadly chaining these together results in borrow-checker failures :(
    // Unwraps will succeed because these are required fields in the struct
    for obj in applied_objs.as_array_mut().unwrap().iter_mut() {
        process_event_obj(cmd, obj, evt.ts)?
    }
    for obj in deleted_objs.as_array_mut().unwrap().iter_mut() {
        process_event_obj(cmd, obj, evt.ts)?
    }

    evt.applied_objs = serde_json::from_value(applied_objs)?;
    evt.deleted_objs = serde_json::from_value(deleted_objs)?;
    Ok(evt)
}

fn process_event_obj(cmd: &Command, obj: &mut Value, evt_ts: i64) -> EmptyResult {
    let mut ctx = MatchContext::new(obj.clone());
    let matched_counter = counter!(EVENT_MATCHED_COUNTER);
    if trace_matches(&cmd.trace_selector, evt_ts, &mut ctx)? {
        matched_counter.increment(1);
        match &cmd.action {
            CommandAction::Apply(ptr_str, rhs) => process_modify_event_obj(obj, ptr_str, rhs, &ctx)?,
            CommandAction::Remove(ptr_str) => process_remove_event_obj(obj, ptr_str, &ctx)?,
        }
    }

    Ok(())
}

pub(super) fn process_modify_event_obj(obj: &mut Value, ptr_str: &str, rhs: &Rhs, ctx: &MatchContext) -> EmptyResult {
    let modified_counter = counter!(RESOURCE_MODIFIED_COUNTER);
    let values = rhs_to_values(rhs, ctx)?;
    if values.len() != 1 {
        bail!(SkelError::MultipleMatchingValues(
            "only one matched value allowed in modify context".into(),
            // sorta dumb, but I can't pass the vector in to the error because it has references
            format!("{values:?}"),
        ));
    }
    let val = values[0];

    let reified_ptrs = reify_pointers(ptr_str, ctx)?;
    for ptr in reified_ptrs.iter().cloned() {
        // This is slightly inefficient, since we resolve the pointer twice, but I don't
        // think it's a huge deal right now.
        match ptr.resolve(obj) {
            Ok(_) => patch_ext(obj, replace_operation(ptr, val.clone()))?,
            Err(ResolveError::NotFound { .. }) => patch_ext(obj, add_operation(ptr, val.clone()))?,
            Err(e) => bail!(e),
        };
    }
    modified_counter.increment(reified_ptrs.len() as u64);
    Ok(())
}

pub(super) fn process_remove_event_obj(obj: &mut Value, ptr_str: &str, ctx: &MatchContext) -> EmptyResult {
    let modified_counter = counter!(RESOURCE_MODIFIED_COUNTER);
    let reified_ptrs = reify_pointers(ptr_str, ctx)?;
    remove_all_pointers(obj, &reified_ptrs)?;
    modified_counter.increment(reified_ptrs.len() as u64);
    Ok(())
}

pub(super) fn trace_matches(
    trace_selector: &TraceSelector,
    evt_ts: i64,
    ctx: &mut MatchContext,
) -> anyhow::Result<bool> {
    match trace_selector {
        TraceSelector::All => return Ok(true),
        TraceSelector::List(conditions) => {
            for cond in conditions {
                if !match cond {
                    Conditional::Time { op, ts } => time_conditional_matches(evt_ts, *op, *ts),
                    Conditional::Resource { ptr, op, rhs, var } => {
                        resource_conditional_matches(ptr, *op, rhs, var, ctx)?
                    },
                } {
                    return Ok(false);
                }
            }
        },
    };

    // If we get here, everything matched
    Ok(true)
}

pub(super) fn time_conditional_matches(evt_ts: i64, op: TestOperation, ts: i64) -> bool {
    match op {
        TestOperation::Eq => evt_ts == ts,
        TestOperation::Ne => evt_ts != ts,
        TestOperation::Lt => evt_ts < ts,
        TestOperation::Gt => evt_ts > ts,
        TestOperation::Le => evt_ts <= ts,
        TestOperation::Ge => evt_ts >= ts,
        _ => unreachable!(), // doesn't make sense to check if a timestamp exists or not
    }
}

// A resource conditional is a <resource_selector> <op> <val> tuple, but it gets complicated because
// you can define a variable that is referenced both in the conditional, as well as later-on in the
// action.  Each resource conditional only allows you to define a single variable, which is passed
// in here as the `maybe_var` option.  If we don't have such a variable defined, the task is simple:
// map the selector into the JSON Value(s) and check if the object at that/those location matches.
//
// The plurals above are important, because a resource selector can match multiple paths, e.g.,
//
//   /spec/template/spec/containers/*/image
//
// The matching logic creates a set of results.  If that set is non-empty, then this resource is
// considered to match the conditional (_unless_ the operator is NotExists, in which case the logic
// is reversed).  So you can interpret a command like
//
//   remove(spec.template.spec.containers[*].image == "localhost:5000/foo:latest",
//       spec.template.nodeSelector)
//
// as saying "if _any_ container image in the pod matches `localhost:5000/foo:latest`, remove that
// pod's nodeSelector.  In this case, this function would receive
//
//   lhs_selector = "/spec/template/spec/containers/*/image"
//   val = "localhost:5000/foo:latest"
//   maybe_var = None
//
// Where this gets tricky is when we incorporate variables into the mix.  You can interpret
// variables as referencing the set of things that match.  For example,
//
//   remove($x := spec.template.spec.containers[*] | $x.image == "localhost:5000/foo:latest",
//       $x.securityContext)
//
// has the same conditional as the above query; however, the action should be interpreted as "remove
// the securityContext for any container matching the conditional".  Handling this in the code is
// complex.  To construct the "set of things that $x points to" we need to use walk all the matched
// values via json_patch_ext, check the conditional, and then save the part of the path that is
// referenced by the variable as a concrete string.  For example, in the above conditional, if
// container 0 and 2 use the foo:latest image, then we need
//
//   $x -> {/spec/template/spec/containers/0, /spec/template/spec/containers/2}
//
// To accomplish this, we convert the pointer string that the user passes in for the $x definition
// into a regex, by replacing all * with \d+ (since _RIGHT NOW_ we can only use * in array index
// fields), and then we capture the matching substring of the full JSON path into the $x variable.
//
// The MatchContext aggregates all the variable definitions from all conditionals into their
// equivalent "matching sets"; this needs to be passed all the way up to the apply_command_to_event
// function so that the command can then reference the variable defintions.  I experimented with a
// few different ways of passing this information (including returning a (bool, VarSet) tuple,
// creating a ResourceMatch enum, or using a &mut output paramter in the call definition), and
// finally settled on the &mut output parameter as the cleanest, even though it isn't my favorite
// solution ever.
//
// Anyways, in the variable case, this function will receive
//
//   lhs_selector = "/$x/image"
//   val = "localhost:5000/foo:latest"
//   maybe_var = Some({name: "$x", pointer: "/spec/template/spec/containers/*"})
//
// and then it will store the desired value in the MatchContext.  Note that we check for duplicate
// variable definitions at _parse time_, not at execution time, so inserting the variable here is
// safe/will not overwrite something else that uses the same name.
pub(super) fn resource_conditional_matches(
    lhs_selector_str: &str,
    op: TestOperation,
    rhs: &Option<Rhs>,
    maybe_var: &Option<VarDef>,
    ctx: &mut MatchContext, // output parameter
) -> anyhow::Result<bool> {
    let lhs_selector = if let Some(var) = &maybe_var {
        // An undefined variable should have already been caught by the AST code, this unwrap is safe
        let replaced_selector_str = variable_substitution(lhs_selector_str, &var.name, &var.pointer).unwrap();
        PointerBuf::parse(replaced_selector_str)?
    } else {
        PointerBuf::parse(lhs_selector_str)?
    };

    let mut ctx_entry = MatchContextEntry::new();
    let mut match_found = false;

    // A tricky subtlety is that for anything other than Exists/NotExists, we only test the fields
    // where the pointer exists AND the condition holds.  For example, in the following array:
    //
    //   [{"name": "container1"}, {"image": "container2"}, {"name": "container3"}]
    //
    // if we search for name != "container1", the result will only refer to the last array element,
    // not the middle array element, because the middle array element does not have a name field.
    // This feels like maybe a bit of a footgun but I also don't know an easy way to work around it
    // right now.
    for (lhs_ptr, lhs) in matches(&lhs_selector, ctx.obj()).into_iter() {
        // This is extremely subtle: in the event that we're checking for non-existence,
        // we need to test that _all_ of the possible values don't match.  So we keep track of a
        // match_found variable that tracks if anything matches, and negate it if the operation is
        // NotExists and we found something.
        //
        // ..... it kinda seems like this is the wrong way to do it, but I can't boolean logic.
        if op == TestOperation::Exists || op == TestOperation::NotExists || lhs_op_rhs(lhs, op, rhs, ctx)? {
            match_found = true;
            if let Some(var) = &maybe_var {
                let var_ptr_regex = Regex::new(&("^(".to_string() + &var.pointer.clone().replace("*", r"\d+") + ")"))?;
                // These unwraps _should_ be safe, since we already know that a match exists by
                // virtue of the fact that we're here visiting this field
                let var_ptr = var_ptr_regex
                    .captures(lhs_ptr.as_str())
                    .unwrap()
                    .get(1)
                    .unwrap()
                    .as_str()
                    .to_string();
                let var_value = matches(&PointerBuf::parse(&var_ptr)?, ctx.obj()).first().unwrap().1.clone();
                ctx_entry.insert(var_ptr, var_value);
            }
        }
    }

    if op == TestOperation::NotExists {
        Ok(!match_found)
    } else if match_found {
        if let Some(VarDef { name, pointer: _ }) = maybe_var {
            ctx.insert(name.to_string(), ctx_entry);
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn variable_substitution(input: &str, variable_name: &str, variable_pointer: &str) -> Option<String> {
    let mut replaced_ptr_str = input.to_string();
    replaced_ptr_str = replaced_ptr_str.replace(variable_name, variable_pointer);

    // Vars might start or end with a slash, so remove any double-slashes
    replaced_ptr_str = replaced_ptr_str.replace("//", "/");
    (replaced_ptr_str != input).then_some(replaced_ptr_str)
}

pub(super) fn reify_pointers(ptr_str: &str, ctx: &MatchContext) -> anyhow::Result<Vec<PointerBuf>> {
    let mut reified_ptrs: Vec<String> = ctx
        .iter()
        .flat_map(|(variable, entry)| {
            entry
                .pointers()
                .iter()
                .filter_map(|p| variable_substitution(ptr_str, variable, p))
        })
        .collect();
    if reified_ptrs.is_empty() {
        reified_ptrs.push(ptr_str.into());
    }
    Ok(reified_ptrs.into_iter().map(PointerBuf::parse).collect::<Result<Vec<_>, _>>()?)
}

pub(super) fn rhs_to_values<'a, 'b>(rhs: &'b Rhs, ctx: &'a MatchContext) -> anyhow::Result<Vec<&'b Value>>
where
    'a: 'b,
{
    Ok(match rhs {
        Rhs::Value(v) => vec![v],
        Rhs::Path(p) => {
            let mut rhs_path = p.clone();
            if rhs_path.as_str().starts_with("/$") {
                let rhs_var_token = rhs_path.pop_front().unwrap(); // fucking "temporary value freed while still in use"
                let rhs_var = rhs_var_token.decoded();

                ctx[rhs_var.as_ref()] // The AST should have already confirmed that the variables are defined
                    .values()
                    .iter()
                    .filter_map(|v| rhs_path.resolve(v).ok()) // if the path doesn't resolve, filter it out
                    .clone()
                    .collect()
            } else {
                vec![rhs_path.resolve(ctx.obj())?]
            }
        },
    })
}

fn lhs_op_rhs(lhs: &Value, op: TestOperation, rhs: &Option<Rhs>, ctx: &MatchContext) -> anyhow::Result<bool> {
    let rhs_value_candidates = match rhs {
        Some(rhs) => rhs_to_values(rhs, ctx)?,
        None => unreachable!(), // cannot be None if we have something other than Exists/NotExists
    };

    Ok(match op {
        TestOperation::Eq => rhs_value_candidates.contains(&lhs),
        TestOperation::Ne => !rhs_value_candidates.contains(&lhs),
        TestOperation::Lt => unimplemented!(),
        TestOperation::Gt => unimplemented!(),
        TestOperation::Le => unimplemented!(),
        TestOperation::Ge => unimplemented!(),
        _ => unreachable!(), // exists/not-exists not valid in infix operator context
    })
}

fn remove_all_pointers(obj: &mut Value, pointers: &[PointerBuf]) -> EmptyResult {
    let mut removed = 0;
    let mut parent_ptr = PointerBuf::new();
    for mut ptr in pointers.iter().cloned() {
        // When we remove an entry from the array, the length of the array (and any subsequent array
        // indices) all need to shift down by one; we keep track of the shift with the "removed"
        // value.  However, if there are nested arrays, we need to reset the "removed" counter
        // whenever we transition to a new "parent" element, which we track in the parent_ptr
        // variable.
        //
        // The back of the pointer is guaranteed to exist at this point, so unwrap is safe
        if let Ok(Index::Num(index)) = ptr.back().unwrap().to_index() {
            ptr.pop_back();
            if *ptr != parent_ptr {
                removed = 0;
                parent_ptr = ptr.clone();
            }
            ptr.push_back(index - removed);
        }
        removed += 1;
        patch_ext(obj, remove_operation(ptr))?;
    }

    Ok(())
}
