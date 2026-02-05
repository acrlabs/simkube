use std::collections::BTreeMap;

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
    TestOperation,
    TraceSelector,
    VarDef,
};
use crate::skel::metrics::*;

pub(super) type MatchContext = BTreeMap<String, Vec<String>>;

pub(super) fn apply_command_to_event(cmd: &Command, mut evt: TraceEvent) -> anyhow::Result<TraceEvent> {
    // Reinterpret the event as a JSON object so JSON pointers work for all fields
    // This seems sortof inefficient, we'll have to see how it works in practice.
    let mut applied_objs = serde_json::to_value(evt.applied_objs)?;
    let mut deleted_objs = serde_json::to_value(evt.deleted_objs)?;

    // Sadly chaining these together results in borrow-checker failures :(
    // Unwraps will succeed because these are required fields in the struct
    for obj in applied_objs.as_array_mut().unwrap().iter_mut() {
        apply_command_to_obj(cmd, obj, evt.ts)?
    }
    for obj in deleted_objs.as_array_mut().unwrap().iter_mut() {
        apply_command_to_obj(cmd, obj, evt.ts)?
    }

    evt.applied_objs = serde_json::from_value(applied_objs)?;
    evt.deleted_objs = serde_json::from_value(deleted_objs)?;
    Ok(evt)
}

fn apply_command_to_obj(cmd: &Command, obj: &mut Value, evt_ts: i64) -> EmptyResult {
    let mut context = MatchContext::new();
    let matched_counter = counter!(EVENT_MATCHED_COUNTER);
    let modified_counter = counter!(RESOURCE_MODIFIED_COUNTER);
    if trace_matches(&cmd.trace_selector, evt_ts, obj, &mut context)? {
        matched_counter.increment(1);
        match &cmd.action {
            CommandAction::Remove(ptr_str) => {
                if context.is_empty() {
                    let ptr = PointerBuf::parse(ptr_str)?;
                    patch_ext(obj, remove_operation(ptr))?;
                    modified_counter.increment(1);
                } else {
                    for (variable, pointers) in context {
                        remove_all_pointers(obj, ptr_str, &variable, &pointers)?;
                        modified_counter.increment(pointers.len() as u64);
                    }
                }
            },
        }
    }

    Ok(())
}

pub(super) fn trace_matches(
    trace_selector: &TraceSelector,
    evt_ts: i64,
    obj: &Value,
    ctx: &mut MatchContext,
) -> anyhow::Result<bool> {
    match trace_selector {
        TraceSelector::All => return Ok(true),
        TraceSelector::List(conditions) => {
            for cond in conditions {
                if !match cond {
                    Conditional::Time { op, ts } => time_conditional_matches(evt_ts, *op, *ts),
                    Conditional::Resource { ptr, op, val, var } => {
                        resource_conditional_matches(obj, ptr, *op, val, var, ctx)?
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
//   remove( spec.template.spec.containers[*].image == "localhost:5000/foo:latest",
//   spec.template.nodeSelector,)
//
// as saying "if _any_ container image in the pod matches `localhost:5000/foo:latest`, remove that
// pod's nodeSelector.  In this case, this function would receive
//
//   ptr_str = "/spec/template/spec/containers/*/image" val = "localhost:5000/foo:latest" maybe_var
//   = None
//
// Where this gets tricky is when we incorporate variables into the mix.  You can interpret
// variables as referencing the set of things that match.  For example,
//
//   remove( $x := spec.template.spec.containers[*] | $x.image == "localhost:5000/foo:latest",
//   $x.securityContext)
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
//   ptr_str = "/$x/image" val = "localhost:5000/foo:latest" maybe_var = Some({name: "$x", pointer:
//   "/spec/template/spec/containers/*"})
//
// and then it will store
//
//   "$x" = vec![ "/spec/template/spec/containers/0", "/spec/template/spec/containers/2", ]
//
// in the MatchContext object.  Note that we check for duplicate variable definitions at _parse
// time_, not at execution time, so inserting the variable here is safe/will not overwrite something
// else that uses the same name.
pub(super) fn resource_conditional_matches(
    obj: &Value,
    ptr_str: &str,
    op: TestOperation,
    val: &Option<Value>,
    maybe_var: &Option<VarDef>,
    ctx: &mut MatchContext,
) -> anyhow::Result<bool> {
    let ptr = if let Some(var) = &maybe_var {
        let replaced_ptr_str = variable_substitution(ptr_str, &var.name, &var.pointer);
        PointerBuf::parse(&replaced_ptr_str)?
    } else {
        PointerBuf::parse(ptr_str)?
    };

    let mut matched_ptrs = vec![];

    // A tricky subtlety is that for anything other than Exists/NotExists, we only test the fields
    // where the pointer exists AND the condition holds.  For example, in the following array:
    //
    //   [{"name": "container1"}, {"image": "container2"}, {"name": "container3"}]
    //
    // if we search for name != "container1", the result will only refer to the last array element,
    // not the middle array element, because the middle array element does not have a name field.
    // This feels like maybe a bit of a footgun but I also don't know an easy way to work around it
    // right now.
    for (matched_ptr, field) in matches(&ptr, obj).into_iter() {
        if match op {
            // Unwraps are safe/guaranteed by the parser
            TestOperation::Eq => field == val.as_ref().unwrap(),
            TestOperation::Ne => field != val.as_ref().unwrap(),
            TestOperation::Lt => unimplemented!(),
            TestOperation::Gt => unimplemented!(),
            TestOperation::Le => unimplemented!(),
            TestOperation::Ge => unimplemented!(),
            // This is counterintuitive, we want to include matching items here for _either_ exists
            // or not_exists: that way, when we check the matching length below, we know that if
            // the object exists when it's not supposed to, it will contribute to the count and we
            // return the right value
            TestOperation::Exists | TestOperation::NotExists => true,
        } {
            if let Some(var) = &maybe_var {
                let ptr_regex = Regex::new(&("^(".to_string() + &var.pointer.clone().replace("*", r"\d+") + ")"))?;
                // These unwraps _should_ be safe, since we already know that a match exists by
                // virtue of the fact that we're here visiting this field
                matched_ptrs.push(
                    ptr_regex
                        .captures(matched_ptr.as_str())
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str()
                        .into(),
                )
            } else {
                // This is just going to get thrown away later, but whatever
                matched_ptrs.push(matched_ptr.to_string());
            }
        }
    }

    // If nothing matches, there are no variables to concretize in the MatchContext,
    // and if something does match, we return false so we don't need to bother filling in
    // the MatchContext
    if op == TestOperation::NotExists {
        Ok(matched_ptrs.is_empty())
    } else if !matched_ptrs.is_empty() {
        if let Some(VarDef { name, pointer: _ }) = maybe_var {
            ctx.insert(name.to_string(), matched_ptrs);
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn variable_substitution(input: &str, variable_name: &str, variable_pointer: &str) -> String {
    let mut replaced_ptr_str = input.to_string();
    replaced_ptr_str = replaced_ptr_str.replace(variable_name, variable_pointer);

    // Vars might start or end with a slash, so remove any double-slashes
    replaced_ptr_str = replaced_ptr_str.replace("//", "/");
    replaced_ptr_str
}

fn remove_all_pointers(obj: &mut Value, ptr_str: &str, variable: &str, pointers: &Vec<String>) -> EmptyResult {
    let mut removed = 0;
    let mut parent_ptr = PointerBuf::new();
    for pointer in pointers {
        let replaced_ptr_str = variable_substitution(ptr_str, variable, pointer);
        let mut ptr = PointerBuf::parse(&replaced_ptr_str)?;

        // When we remove an entry from the array, the length of the array (and any subsequent array
        // indices) all need to shift down by one; we keep track of the shift with the "removed"
        // value.  However, if there are nested arrays, we need to reset the "removed" counter
        // whenever we transition to a new "parent" element, which we track in the parent_ptr
        // variable.
        //
        // The back of the pointer is guaranteed to exist at this point, so unwrap is safe
        if let Ok(Index::Num(index)) = ptr.back().unwrap().to_index() {
            ptr.pop_back();
            if ptr != parent_ptr {
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
