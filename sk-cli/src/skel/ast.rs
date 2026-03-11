use std::collections::HashSet;

use anyhow::{
    anyhow,
    bail,
};
use json_patch_ext::PointerBuf;
use pest::iterators::{
    Pair,
    Pairs,
};
use serde_json::Value;

use crate::skel::Rule;
use crate::skel::errors::SkelError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestOperation {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Exists,
    NotExists,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct VarDef {
    pub(super) name: String,
    pub(super) pointer: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Rhs {
    Value(Value),
    Path(PointerBuf),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Conditional {
    Time {
        ts: i64,
        op: TestOperation,
    },
    Resource {
        ptr: String,
        op: TestOperation,
        rhs: Option<Rhs>,
        var: Option<VarDef>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum TraceSelector {
    All,
    List(Vec<Conditional>),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum CommandAction {
    Apply(String, Rhs),
    Remove(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Command {
    pub(super) trace_selector: TraceSelector,
    pub(super) action: CommandAction,
}

pub(super) fn parse_command(cmd: Pair<Rule>, trace_start_ts: i64) -> anyhow::Result<Command> {
    let rule = cmd.as_rule();
    let args = cmd.into_inner();

    Ok(match rule {
        Rule::modify_cmd => parse_modify_command(args, trace_start_ts)?,
        Rule::remove_cmd => parse_remove_command(args, trace_start_ts)?,
        x => unreachable!("Rule::{x:?}"),
    })
}

pub(super) fn parse_modify_command(mut args: Pairs<Rule>, trace_start_ts: i64) -> anyhow::Result<Command> {
    let defined_vars = HashSet::new();
    // unwraps are safe, remove always takes 1-2 args
    let (trace_selector, resource_ptr, rhs) = if args.peek().unwrap().as_rule() == Rule::assignment {
        let mut assignment = args.next().unwrap().into_inner();
        (
            TraceSelector::All,
            parse_resource_path(assignment.next().unwrap(), &defined_vars)?,
            parse_rhs(assignment.next().unwrap(), &defined_vars)?,
        )
    } else {
        let mut defined_vars = HashSet::new();
        let selector = parse_trace_selector(args.next().unwrap(), trace_start_ts, &mut defined_vars)?;
        let mut assignment = args.next().unwrap().into_inner();
        (
            selector,
            parse_resource_path(assignment.next().unwrap(), &defined_vars)?,
            parse_rhs(assignment.next().unwrap(), &defined_vars)?,
        )
    };

    Ok(Command {
        trace_selector,
        action: CommandAction::Apply(resource_ptr, rhs),
    })
}

pub(super) fn parse_remove_command(mut args: Pairs<Rule>, trace_start_ts: i64) -> anyhow::Result<Command> {
    let mut defined_vars = HashSet::new();
    // unwraps are safe, remove always takes 1-2 args
    let (trace_selector, resource_ptr) = if args.peek().unwrap().as_rule() == Rule::resource_path {
        (TraceSelector::All, parse_resource_path(args.next().unwrap(), &defined_vars)?)
    } else {
        (
            parse_trace_selector(args.next().unwrap(), trace_start_ts, &mut defined_vars)?,
            parse_resource_path(args.next().unwrap(), &defined_vars)?,
        )
    };

    Ok(Command {
        trace_selector,
        action: CommandAction::Remove(resource_ptr),
    })
}

pub(super) fn parse_trace_selector(
    sel: Pair<Rule>,
    trace_start_ts: i64,
    defined_vars: &mut HashSet<String>,
) -> anyhow::Result<TraceSelector> {
    Ok(match sel.as_rule() {
        Rule::trace_selector_all => TraceSelector::All,
        Rule::trace_selector_list => {
            let mut conditions = Vec::new();

            for s in sel.into_inner() {
                match s.as_rule() {
                    Rule::resource_conditional => {
                        let cond = parse_resource_conditional(s.into_inner(), defined_vars)?;
                        conditions.push(cond);
                    },
                    Rule::ts_conditional => {
                        conditions.push(parse_ts_conditional(s.into_inner(), trace_start_ts));
                    },
                    x => unreachable!("Rule::{x:?}"),
                }
            }
            TraceSelector::List(conditions)
        },
        x => unreachable!("Rule::{x:?}"),
    })
}

pub(super) fn parse_resource_conditional(
    mut cond: Pairs<Rule>,
    defined_vars: &mut HashSet<String>,
) -> anyhow::Result<Conditional> {
    // Unwraps are safe/guaranteed by the grammar
    let test_type = cond.next().unwrap();
    let test_rule = test_type.as_rule();
    let mut test = test_type.into_inner();

    match test_rule {
        Rule::resource_test => parse_resource_test(test.next().unwrap(), None, defined_vars),
        Rule::var_test => {
            let var_name = test.next().unwrap().as_str().trim();
            if defined_vars.contains(var_name) {
                bail!(SkelError::MultipleVariableDefinitions(var_name.into()));
            } else {
                defined_vars.insert(var_name.into());
            }
            let resource_path = parse_resource_path(test.next().unwrap(), defined_vars)?;
            let var = VarDef { name: var_name.to_string(), pointer: resource_path };

            // the actual resource test _type_ is one layer deeper because
            // we don't mark resource_test as quiet so we can match on it above
            parse_resource_test(test.next().unwrap().into_inner().next().unwrap(), Some(var.clone()), defined_vars)
        },
        x => unreachable!("Rule::{x:?}"),
    }
}

pub(super) fn parse_ts_conditional(mut cond: Pairs<Rule>, trace_start_ts: i64) -> Conditional {
    // Unwraps are safe/guaranteed by the grammar
    let op = parse_test_operation(cond.next().unwrap());
    let time = cond.next().unwrap();

    let ts = match time.as_rule() {
        Rule::absolute_ts => {
            // This unwrap is safe because the grammar says a time string is a sequence of digits
            time.as_str().parse::<i64>().unwrap()
        },
        Rule::relative_ts => {
            let tstr = time.as_str();
            // Same reasoning here, these unwraps are also safe
            let delta = tstr[..tstr.len() - 1].parse::<i64>().unwrap();
            let delta_seconds = match tstr.chars().last().unwrap() {
                's' => delta,
                'm' => delta * 60,
                'h' => delta * 3600,
                x => unreachable!("character: {x}"),
            };
            trace_start_ts + delta_seconds
        },
        x => unreachable!("Rule::{x:?}"),
    };

    Conditional::Time { ts, op }
}

fn parse_resource_test(
    test: Pair<Rule>,
    var: Option<VarDef>,
    defined_vars: &HashSet<String>,
) -> anyhow::Result<Conditional> {
    let rule_type = test.as_rule();
    let mut cond = test.into_inner();
    // If the resource test contains a variable, it must be a single variable that is used
    // in both the path string and the LHS of the test; that's why here we construct a new HashSet
    // containing only that variable name
    let local_defined_var = var.iter().map(|v| v.name.clone()).collect();

    // Unwraps are safe/guaranteed by the grammar
    let resource_path = cond.next().unwrap();
    let resource_path_str = resource_path.as_str().to_string();

    // Since we are overriding the defined variables, it might be confusing to return an
    // UndefinedVariable error; if parse_resource_path fails with an undefined variable, we map it
    // to InvalidLHS, otherwise we leave it alone
    let resource_ptr =
        parse_resource_path(resource_path, &local_defined_var).map_err(|e| match e.downcast::<SkelError>() {
            Ok(SkelError::UndefinedVariable(v)) => anyhow!(SkelError::InvalidLHS(v.clone(), resource_path_str)),
            Ok(skelerr) => anyhow!(skelerr),
            Err(err) => anyhow!(err),
        })?;
    Ok(match rule_type {
        Rule::conditional_test => {
            let op = parse_test_operation(cond.next().unwrap());
            let rhs = parse_rhs(cond.next().unwrap(), defined_vars)?;
            Conditional::Resource { ptr: resource_ptr, op, rhs: Some(rhs), var }
        },
        Rule::exists_test => Conditional::Resource {
            ptr: resource_ptr,
            op: TestOperation::Exists,
            rhs: None,
            var,
        },
        Rule::not_exists_test => Conditional::Resource {
            ptr: resource_ptr,
            op: TestOperation::NotExists,
            rhs: None,
            var,
        },
        x => unreachable!("Rule::{x:?}"),
    })
}

pub(super) fn parse_resource_path(path: Pair<Rule>, defined_vars: &HashSet<String>) -> anyhow::Result<String> {
    let mut i = 0;

    let start = path.as_span().start();
    let path_str = path.as_str();
    let mut ptr = String::with_capacity(path.as_str().len() + 1); // +1 for leading slash
    ptr.push('/');
    for p in path.into_inner() {
        match p.as_rule() {
            Rule::var => {
                let v = p.as_str();
                if !defined_vars.contains(v) {
                    bail!(SkelError::UndefinedVariable(v.into()))
                }
                continue;
            },
            Rule::quoted_path_part => {
                let (pstart, pend) = (p.as_span().start() - start, p.as_span().end() - start);
                // . and [ get converted to /, the closing bracket turns into an empty string
                // so we don't end up with double-slashes or slashes at the end of an array
                // selector, neither of which are handled correctly by json_patch_ext
                ptr.push_str(&path_str[i..pstart].replace(".", "/").replace("[", "/").replace("]", ""));
                // strip off the quotes on the inner string
                ptr.push_str(&path_str[pstart + 1..pend - 1].replace("/", "~1"));
                i = pend;
            },
            x => unreachable!("Rule::{x:?}"),
        }
    }
    ptr.push_str(&path_str[i..].replace(".", "/").replace("[", "/").replace("]", ""));
    Ok(ptr)
}

pub(super) fn parse_test_operation(op: Pair<Rule>) -> TestOperation {
    match op.as_rule() {
        Rule::eq => TestOperation::Eq,
        Rule::ne => TestOperation::Ne,
        Rule::gt => TestOperation::Gt,
        Rule::lt => TestOperation::Lt,
        Rule::ge => TestOperation::Ge,
        Rule::le => TestOperation::Le,
        x => unreachable!("Rule::{x:?}"), // exists and !exists handled elsewhere
    }
}

pub(super) fn parse_rhs(rhs: Pair<Rule>, defined_vars: &HashSet<String>) -> anyhow::Result<Rhs> {
    Ok(match rhs.as_rule() {
        // For all intents and purposes, this unwrap is safe; the grammar ensures
        // that if we match Rule::number, we have a sequence of digits, which will
        // safely parse into an i64.  TECHNICALLY it is possible to write a number
        // that is larger than will fit in i64 and this will panic, but I do not
        // care about handling that case right now.
        Rule::number => Rhs::Value(Value::Number(rhs.as_str().parse::<i64>().unwrap().into())),
        Rule::string => Rhs::Value(Value::String(rhs.as_str().into())),
        Rule::true_val => Rhs::Value(Value::Bool(true)),
        Rule::false_val => Rhs::Value(Value::Bool(false)),
        Rule::resource_path => {
            // We can call this with no variables or one variable, but if
            // there is a variable, it must have been previously defined
            if let Some((var_prefix, _)) = rhs.as_str().split_once('.')
                && var_prefix.starts_with('$')
                && !defined_vars.contains(var_prefix)
            {
                bail!(SkelError::UndefinedVariable(var_prefix.into()));
            }
            let res_path = parse_resource_path(rhs, defined_vars)?;
            Rhs::Path(PointerBuf::parse(res_path)?)
        },
        x => unreachable!("Rule::{x:?}"),
    })
}
