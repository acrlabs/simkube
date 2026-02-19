use serde_json::{
    Value,
    json,
};

use super::*;
use crate::skel::ast::{
    Conditional,
    TestOperation,
    TraceSelector,
    VarDef,
};
use crate::skel::engine::{
    MatchContext,
    resource_conditional_matches,
    time_conditional_matches,
    trace_matches,
    variable_substitution,
};

#[fixture]
fn test_obj() -> Value {
    json!({
        "metadata": {"labels": {"foo": "bar"}},
        "spec": {
            "template": {
                "spec": {
                    "containers": [
                        {"name": "container1"},
                        {"image": "asdf:latest"},  // huh, where'd the name go?
                        {"name": "container2"},
                    ]
                }
            }
        }
    })
}

#[rstest]
fn test_skel_trace_matches_all(test_obj: Value) {
    assert_ok_eq_x!(trace_matches(&TraceSelector::All, 1234, &test_obj, &mut MatchContext::new()), true);
}

#[rstest]
#[case(TestOperation::Eq, false)]
#[case(TestOperation::Ne, true)]
fn test_skel_trace_matches_list(test_obj: Value, #[case] op: TestOperation, #[case] expected: bool) {
    assert_ok_eq_x!(
        trace_matches(
            &TraceSelector::List(vec![
                Conditional::Time { ts: 1235, op },
                Conditional::Resource {
                    ptr: "/metadata/labels/foo".into(),
                    op: TestOperation::Eq,
                    val: Some(json!("bar")),
                    var: None
                }
            ]),
            1234,
            &test_obj,
            &mut MatchContext::new()
        ),
        expected
    );
}


#[rstest]
#[case(TestOperation::Eq, 1, 1, true)]
#[case(TestOperation::Eq, 1, 2, false)]
#[case(TestOperation::Ne, 1, 1, false)]
#[case(TestOperation::Ne, 1, 2, true)]
#[case(TestOperation::Gt, 1, 1, false)]
#[case(TestOperation::Gt, 1, 2, false)]
#[case(TestOperation::Gt, 2, 1, true)]
#[case(TestOperation::Lt, 1, 1, false)]
#[case(TestOperation::Lt, 1, 2, true)]
#[case(TestOperation::Lt, 2, 1, false)]
#[case(TestOperation::Ge, 1, 1, true)]
#[case(TestOperation::Ge, 1, 2, false)]
#[case(TestOperation::Ge, 2, 1, true)]
#[case(TestOperation::Le, 1, 1, true)]
#[case(TestOperation::Le, 1, 2, true)]
#[case(TestOperation::Le, 2, 1, false)]
fn test_skel_time_conditional_matches(
    #[case] op: TestOperation,
    #[case] n1: i64,
    #[case] n2: i64,
    #[case] expected: bool,
) {
    assert_eq!(time_conditional_matches(n1, op, n2), expected);
}

#[rstest]
#[case(TestOperation::Eq, "bar", true)]
#[case(TestOperation::Ne, "bar", false)]
#[case(TestOperation::Eq, "baz", false)]
#[case(TestOperation::Ne, "baz", true)]
fn test_skel_resource_conditional_matches(
    test_obj: Value,
    #[case] op: TestOperation,
    #[case] condition_value: &str,
    #[case] expected: bool,
) {
    let mut match_context = MatchContext::new();
    assert_eq!(
        resource_conditional_matches(
            &test_obj,
            "/metadata/labels/foo",
            op,
            &Some(json!(condition_value)),
            &None,
            &mut match_context,
        )
        .unwrap(),
        expected
    );
    assert_is_empty!(match_context);
}

#[rstest]
#[case(TestOperation::Exists, true)]
#[case(TestOperation::NotExists, false)]
fn test_skel_resource_conditional_matches_existence(
    test_obj: Value,
    #[case] op: TestOperation,
    #[case] expected: bool,
) {
    let mut match_context = MatchContext::new();
    assert_eq!(
        resource_conditional_matches(&test_obj, "/metadata/labels/foo", op, &None, &None, &mut match_context).unwrap(),
        expected
    );
    assert_is_empty!(match_context);
}

#[rstest]
#[case(TestOperation::Eq, "container1", true, vec!["/spec/template/spec/containers/0".to_string()])]
#[case(TestOperation::Ne, "container1", true, vec!["/spec/template/spec/containers/2".to_string()])]
#[case(TestOperation::Eq, "container3", false, vec![])]
#[case(TestOperation::Ne, "container3", true, vec!["/spec/template/spec/containers/0".to_string(), "/spec/template/spec/containers/2".to_string()])]
fn test_skel_resource_conditional_matches_variable(
    test_obj: Value,
    #[case] op: TestOperation,
    #[case] condition_value: &str,
    #[case] expected: bool,
    #[case] expected_ctx_ptrs: Vec<String>,
) {
    let mut match_context = MatchContext::new();
    assert_eq!(
        resource_conditional_matches(
            &test_obj,
            "/$x/name",
            op,
            &Some(json!(condition_value)),
            &Some(VarDef {
                name: "$x".into(),
                pointer: "/spec/template/spec/containers/*".into()
            }),
            &mut match_context,
        )
        .unwrap(),
        expected
    );
    if expected_ctx_ptrs.len() > 0 {
        assert_eq!(match_context["$x"], expected_ctx_ptrs);
    } else {
        assert_is_empty!(match_context);
    }
}

#[rstest]
#[case(TestOperation::Exists, true)]
#[case(TestOperation::NotExists, false)]
fn test_skel_resource_conditional_matches_variable_existence(
    test_obj: Value,
    #[case] op: TestOperation,
    #[case] expected: bool,
) {
    let mut match_context = MatchContext::new();
    assert_eq!(
        resource_conditional_matches(
            &test_obj,
            "/$x/name",
            op,
            &None,
            &Some(VarDef {
                name: "$x".into(),
                pointer: "/spec/template/spec/containers/*".into()
            }),
            &mut match_context,
        )
        .unwrap(),
        expected
    );
    if op == TestOperation::Exists {
        assert_eq!(
            match_context["$x"],
            vec!["/spec/template/spec/containers/0".to_string(), "/spec/template/spec/containers/2".to_string()]
        )
    } else {
        assert_is_empty!(match_context);
    }
}

#[rstest]
#[case("/$x/foo", "$x", "/metadata/labels")]
#[case("/$x/foo", "$x", "/metadata/labels/")]
fn test_skel_variable_substitution(#[case] input: &str, #[case] var_name: &str, #[case] var_pointer: &str) {
    assert_eq!(variable_substitution(input, var_name, var_pointer), "/metadata/labels/foo");
}
