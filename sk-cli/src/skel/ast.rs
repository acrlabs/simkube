use std::collections::HashSet;

use anyhow::bail;
use pest::iterators::{
    Pair,
    Pairs,
};
use serde_json::Value;

use crate::skel::Rule;

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
pub(super) enum Conditional {
    Time {
        ts: i64,
        op: TestOperation,
    },
    Resource {
        ptr: String,
        op: TestOperation,
        val: Option<Value>,
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
    Remove(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Command {
    pub(super) trace_selector: TraceSelector,
    pub(super) action: CommandAction,
}

pub(super) fn parse_command(cmd: Pair<Rule>, trace_start_ts: i64) -> anyhow::Result<Command> {
    match cmd.as_rule() {
        Rule::remove_cmd => {
            let mut remove_args = cmd.into_inner();
            let (trace_selector, resource_ptr) =
                    // unwraps are safe, remove always takes 1-2 args
                    if remove_args.peek().unwrap().as_rule() == Rule::resource_path{
                        (TraceSelector::All, parse_resource_path(remove_args.next().unwrap()))
                    } else {
                        (
                            parse_trace_selector(remove_args.next().unwrap(), trace_start_ts)?,
                            parse_resource_path(remove_args.next().unwrap()),
                        )
                    };

            Ok(Command {
                trace_selector,
                action: CommandAction::Remove(resource_ptr),
            })
        },
        _ => unreachable!(),
    }
}

pub(super) fn parse_trace_selector(sel: Pair<Rule>, trace_start_ts: i64) -> anyhow::Result<TraceSelector> {
    Ok(match sel.as_rule() {
        Rule::trace_selector_all => TraceSelector::All,
        Rule::trace_selector_list => {
            let mut variables = HashSet::new();
            let mut conditions = Vec::new();

            for s in sel.into_inner() {
                match s.as_rule() {
                    Rule::resource_conditional => {
                        let cond = parse_resource_conditional(s.into_inner());
                        match cond {
                            Conditional::Resource { ref var, .. } => {
                                if let Some(var) = var {
                                    if let Some(ptr) = variables.get(&var.name) {
                                        bail!("variable {} already defined as {}", var.name, ptr);
                                    } else {
                                        variables.insert(var.name.clone());
                                    }
                                }
                            },
                            _ => unreachable!(),
                        }
                        conditions.push(cond);
                    },
                    Rule::ts_conditional => {
                        conditions.push(parse_ts_conditional(s.into_inner(), trace_start_ts));
                    },
                    _ => unreachable!(),
                }
            }
            TraceSelector::List(conditions)
        },
        _ => unreachable!(),
    })
}

pub(super) fn parse_resource_conditional(mut cond: Pairs<Rule>) -> Conditional {
    // Unwraps are safe/guaranteed by the grammar
    let test_type = cond.next().unwrap();
    let test_rule = test_type.as_rule();
    let mut test = test_type.into_inner();

    match test_rule {
        Rule::resource_test => parse_resource_test(test.next().unwrap(), None),
        Rule::var_test => {
            let var_name = test.next().unwrap().as_str().trim();
            let resource_path = parse_resource_path(test.next().unwrap());
            let var = VarDef { name: var_name.to_string(), pointer: resource_path };

            // the actual resource test _type_ is one layer deeper because
            // we don't mark resource_test as quiet so we can match on it above
            parse_resource_test(test.next().unwrap().into_inner().next().unwrap(), Some(var.clone()))
        },
        _ => unreachable!(),
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
                _ => unreachable!(),
            };
            trace_start_ts + delta_seconds
        },
        _ => unreachable!(),
    };

    Conditional::Time { ts, op }
}

fn parse_resource_test(test: Pair<Rule>, var: Option<VarDef>) -> Conditional {
    let rule_type = test.as_rule();
    let mut cond = test.into_inner();
    // Unwraps are safe/guaranteed by the grammar
    let resource_ptr = parse_resource_path(cond.next().unwrap());
    match rule_type {
        Rule::conditional_test => {
            let op = parse_test_operation(cond.next().unwrap());
            let val = parse_value(cond.next().unwrap());
            Conditional::Resource { ptr: resource_ptr, op, val: Some(val), var }
        },
        Rule::exists_test => Conditional::Resource {
            ptr: resource_ptr,
            op: TestOperation::Exists,
            val: None,
            var,
        },
        Rule::not_exists_test => Conditional::Resource {
            ptr: resource_ptr,
            op: TestOperation::NotExists,
            val: None,
            var,
        },
        _ => unreachable!(),
    }
}

pub(super) fn parse_resource_path(path: Pair<Rule>) -> String {
    let mut i = 0;

    let start = path.as_span().start();
    let path_str = path.as_str();
    let mut ptr = String::with_capacity(path.as_str().len() + 1); // +1 for leading slash
    ptr.push('/');
    for p in path.into_inner() {
        match p.as_rule() {
            Rule::var => continue,
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
            _ => unreachable!(),
        }
    }
    ptr.push_str(&path_str[i..].replace(".", "/").replace("[", "/").replace("]", ""));
    ptr
}

pub(super) fn parse_test_operation(op: Pair<Rule>) -> TestOperation {
    match op.as_rule() {
        Rule::eq => TestOperation::Eq,
        Rule::ne => TestOperation::Ne,
        Rule::gt => TestOperation::Gt,
        Rule::lt => TestOperation::Lt,
        Rule::ge => TestOperation::Ge,
        Rule::le => TestOperation::Le,
        _ => unreachable!(), // exists and !exists handled elsewhere
    }
}

pub(super) fn parse_value(val: Pair<Rule>) -> Value {
    match val.as_rule() {
        // For all intents and purposes, this unwrap is safe; the grammar ensures
        // that if we match Rule::number, we have a sequence of digits, which will
        // safely parse into an i64.  TECHNICALLY it is possible to write a number
        // that is larger than will fit in i64 and this will panic, but I do not
        // care about handling that case right now.
        Rule::number => Value::Number(val.as_str().parse::<i64>().unwrap().into()),
        Rule::string => Value::String(val.as_str().into()),
        Rule::true_val => Value::Bool(true),
        Rule::false_val => Value::Bool(false),
        _ => unreachable!(),
    }
}
