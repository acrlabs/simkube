use std::collections::HashSet;

use json_patch_ext::PointerBuf;
use serde_json::{
    Value,
    json,
};

use super::*;
use crate::skel::ast::{
    Command,
    CommandAction,
    Conditional,
    Rhs,
    TestOperation,
    TraceSelector,
    VarDef,
    parse_modify_command,
    parse_remove_command,
    parse_resource_conditional,
    parse_resource_path,
    parse_rhs,
    parse_trace_selector,
    parse_ts_conditional,
};

#[rstest]
fn test_parse_modify_command() {
    let expected = Command {
        trace_selector: TraceSelector::All,
        action: CommandAction::Apply("/metadata/labels/asdf".into(), Rhs::Value(json!("foo"))),
    };
    let cmd = SkelParser::parse(Rule::command, "modify(metadata.labels.asdf = \"foo\")")
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_ok_eq_x!(&parse_modify_command(cmd, 1234), &expected);
}

#[rstest]
fn test_parse_remove_command() {
    let expected = Command {
        trace_selector: TraceSelector::All,
        action: CommandAction::Remove("/metadata/labels".into()),
    };
    let cmd = SkelParser::parse(Rule::command, "remove(metadata.labels)")
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_ok_eq_x!(&parse_remove_command(cmd, 1234), &expected);
}

#[rstest]
#[case("*", TraceSelector::All, HashSet::new())]
#[case(
    "@t == 1234 && metadata.labels == \"foo\"
        && $x := metadata.labels | exists($x)
        && $y := metadata.annotations | !exists($y)",
    TraceSelector::List(vec![
        Conditional::Time{ts: 1234, op: TestOperation::Eq},
        Conditional::Resource{
            ptr: "/metadata/labels".into(),
            op: TestOperation::Eq,
            rhs: Some(Rhs::Value(Value::String("foo".into()))),
            var: None,
        },
        Conditional::Resource{
            ptr: "/$x".into(),
            op: TestOperation::Exists,
            rhs: None,
            var: Some(VarDef{ name: "$x".into(), pointer: "/metadata/labels".into() }),
        },
        Conditional::Resource{
            ptr: "/$y".into(),
            op: TestOperation::NotExists,
            rhs: None,
            var: Some(VarDef{ name: "$y".into(), pointer: "/metadata/annotations".into() }),
        },
    ]),
    HashSet::from(["$x".into(), "$y".into()]),
)]
fn test_parse_trace_selector(
    #[case] sel_str: &str,
    #[case] expected: TraceSelector,
    #[case] expected_vars: HashSet<String>,
) {
    let mut defined_vars = HashSet::new();
    let sel = SkelParser::parse(Rule::trace_selector_expr, sel_str).unwrap().next().unwrap();
    let parsed = parse_trace_selector(sel, 1234, &mut defined_vars);
    assert_ok_eq_x!(&parsed, &expected);
    assert_bag_eq!(defined_vars, expected_vars);
}


#[rstest]
#[case(
    "metadata.labels == 1234",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::Eq,
        rhs: Some(Rhs::Value(Value::Number(1234.into()))),
        var: None,
    },
)]
#[case(
    "exists(metadata.labels)",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::Exists,
        rhs: None,
        var: None,
    },
)]
#[case(
    "!exists(metadata.labels)",
    Conditional::Resource{
        ptr: "/metadata/labels".into(),
        op: TestOperation::NotExists,
        rhs: None,
        var: None,
    },
)]
#[case(
    "$x := metadata.labels | exists($x))",
    Conditional::Resource{
        ptr: "/$x".into(),
        op: TestOperation::Exists,
        rhs: None,
        var: Some(VarDef{name: "$x".into(), pointer: "/metadata/labels".into()}),
    },
)]
fn test_parse_resource_conditional(#[case] cond_str: &str, #[case] expected: Conditional) {
    let mut defined_vars = HashSet::new();
    let cond = SkelParser::parse(Rule::resource_conditional, cond_str)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_ok_eq_x!(&parse_resource_conditional(cond, &mut defined_vars), &expected);
    if let Conditional::Resource { var: Some(v), .. } = expected {
        assert_bag_eq!(defined_vars, [v.name]);
    }
}

#[rstest]
#[case::undefined_name("$x := metadata.labels | exists($y)", HashSet::new())]
#[case::duplicate_names("$x := metadata.labels | exists($x)", HashSet::from(["$x".into()]))]
fn test_parse_resource_conditional_errors(#[case] cond_str: &str, #[case] mut defined_vars: HashSet<String>) {
    let cond = SkelParser::parse(Rule::resource_conditional, cond_str)
        .unwrap()
        .next()
        .unwrap()
        .into_inner();
    assert_err!(&parse_resource_conditional(cond, &mut defined_vars));
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
fn test_parse_resource_path(#[case] path_str: &str, #[case] expected: &str) {
    let path = SkelParser::parse(Rule::resource_path, path_str).unwrap().next().unwrap();
    assert_ok_eq_x!(&parse_resource_path(path, &HashSet::new()), expected);
}

#[rstest]
#[case("\"asdf\"", Rhs::Value(Value::String("asdf".to_string())))]
#[case("1234", Rhs::Value(Value::Number(1234.into())))]
#[case("tRuE", Rhs::Value(Value::Bool(true)))]
#[case("False", Rhs::Value(Value::Bool(false)))]
fn test_skel_parse_rhs_val(#[case] rhs_str: &str, #[case] expected: Rhs) {
    let mut defined_vars = HashSet::new();
    let rhs = SkelParser::parse(Rule::val, rhs_str).unwrap().next().unwrap();
    assert_ok_eq_x!(&parse_rhs(rhs, &mut defined_vars), &expected);
    assert_is_empty!(defined_vars);
}

#[rstest]
#[case("metadata.name", "/metadata/name")]
#[case("$x.name", "/$x/name")]
fn test_skel_parse_rhs_path(#[case] path: &str, #[case] expected: &str) {
    let defined_vars = HashSet::from(["$x".into()]);
    let rhs = SkelParser::parse(Rule::resource_path, path).unwrap().next().unwrap();
    assert_ok_eq_x!(&parse_rhs(rhs, &defined_vars), &Rhs::Path(PointerBuf::parse(expected).unwrap()));
}

#[rstest]
fn test_skel_parse_rhs_undefined_var() {
    let rhs = SkelParser::parse(Rule::resource_path, "$x.path".into())
        .unwrap()
        .next()
        .unwrap();
    assert_err!(&parse_rhs(rhs, &HashSet::new()));
}
