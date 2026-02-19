use serde_json::Value;

use super::*;
use crate::skel::ast::{
    Command,
    CommandAction,
    Conditional,
    TestOperation,
    TraceSelector,
    VarDef,
    parse_command,
    parse_resource_conditional,
    parse_resource_path,
    parse_trace_selector,
    parse_ts_conditional,
    parse_value,
};

#[rstest]
#[case(
    "remove(metadata.labels)",
    Command{
        trace_selector: TraceSelector::All,
        action: CommandAction::Remove("/metadata/labels".into()),
    },
)]
fn test_parse_command(#[case] cmd_str: &str, #[case] expected: Command) {
    let cmd = SkelParser::parse(Rule::command, cmd_str).unwrap().next().unwrap();
    assert_ok_eq_x!(&parse_command(cmd, 1234), &expected);
}

#[rstest]
#[case("*", TraceSelector::All)]
#[case(
    "@t == 1234 && metadata.labels == \"foo\"
        && $x := metadata.labels | exists($x)
        && $y := metadata.annotations | !exists($y)",
    TraceSelector::List(vec![
        Conditional::Time{ts: 1234, op: TestOperation::Eq},
        Conditional::Resource{
            ptr: "/metadata/labels".into(),
            op: TestOperation::Eq,
            val: Some(Value::String("foo".into())),
            var: None,
        },
        Conditional::Resource{
            ptr: "/$x".into(),
            op: TestOperation::Exists,
            val: None,
            var: Some(VarDef{ name: "$x".into(), pointer: "/metadata/labels".into() }),
        },
        Conditional::Resource{
            ptr: "/$y".into(),
            op: TestOperation::NotExists,
            val: None,
            var: Some(VarDef{ name: "$y".into(), pointer: "/metadata/annotations".into() }),
        },
    ])
)]
fn test_parse_trace_selector(#[case] sel_str: &str, #[case] expected: TraceSelector) {
    let sel = SkelParser::parse(Rule::trace_selector_expr, sel_str).unwrap().next().unwrap();
    assert_ok_eq_x!(&parse_trace_selector(sel, 1234), &expected);
}

#[rstest]
fn test_parse_trace_selector_dup_var_names() {
    let sel_str = "$x := metadata.labels | exists($x) && $x := metadata.annotations | !exists($x)";
    let sel = SkelParser::parse(Rule::trace_selector_expr, sel_str).unwrap().next().unwrap();
    assert_err!(&parse_trace_selector(sel, 1234));
}

#[rstest]
#[case(
    "metadata.labels == 1234",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::Eq,
        val: Some(Value::Number(1234.into())),
        var: None,
    },
)]
#[case(
    "exists(metadata.labels)",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::Exists,
        val: None,
        var: None,
    },
)]
#[case(
    "!exists(metadata.labels)",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::NotExists,
        val: None,
        var: None,
    },
)]
#[case(
    "$x := metadata.labels | exists($x))",
    Conditional::Resource{
        ptr: "/$x".into(),
        op: TestOperation::Exists,
        val: None,
        var: Some(VarDef{name: "$x".into(), pointer: "/metadata/labels".into()}),
    },
)]
fn test_parse_resource_conditional(#[case] cond_str: &str, #[case] expected: Conditional) {
    let cond = SkelParser::parse(Rule::resource_conditional, cond_str)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_eq!(parse_resource_conditional(cond), expected);
}

#[rstest]
#[case("@t == 1234", Conditional::Time{ ts: 1234, op: TestOperation::Eq })]
#[case("@t == 1234s", Conditional::Time{ ts: 2234, op: TestOperation::Eq })]
#[case("@t == 123m", Conditional::Time{ ts: 8380, op: TestOperation::Eq })]
#[case("@t == 12h", Conditional::Time{ ts: 44200, op: TestOperation::Eq })]
fn test_parse_ts_conditional(#[case] cond_str: &str, #[case] expected: Conditional) {
    let cond = SkelParser::parse(Rule::ts_conditional, cond_str)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_eq!(parse_ts_conditional(cond, 1000), expected);
}

#[rstest]
#[case("metadata.namespace", "/metadata/namespace")]
#[case("spec.template.spec.containers[*].env[*]", "/spec/template/spec/containers/*/env/*")]
#[case("metadata.labels.\"foo.bar/baz\"", "/metadata/labels/foo.bar~1baz")]
#[case("meta-data.namespace", "/meta-data/namespace")]
#[case("meta_data.namespace", "/meta_data/namespace")]
#[case("metadata.labels.$x", "/metadata/labels/$x")]
fn test_parse_resource_path(#[case] path_str: &str, #[case] expected: &str) {
    let path = SkelParser::parse(Rule::resource_path, path_str).unwrap().next().unwrap();
    assert_eq!(parse_resource_path(path), expected);
}

#[rstest]
#[case("\"asdf\"", Value::String("asdf".to_string()))]
#[case("1234", Value::Number(1234.into()))]
#[case("tRuE", Value::Bool(true))]
#[case("False", Value::Bool(false))]
fn test_skel_parse_value(#[case] val_str: &str, #[case] expected: Value) {
    let val = SkelParser::parse(Rule::val, val_str).unwrap().next().unwrap();
    assert_eq!(parse_value(val), expected);
}
