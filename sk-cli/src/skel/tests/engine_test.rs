use blackbox_metrics::{
    BlackboxRecorder,
    KeyExt,
    MetricsRead,
};
use json_patch_ext::PointerBuf;
use serde_json::{
    Value,
    json,
};

use super::*;
use crate::skel::ast::{
    Conditional,
    Rhs,
    TestOperation,
    TraceSelector,
    VarDef,
};
use crate::skel::context::*;
use crate::skel::engine::{
    process_modify_event_obj,
    process_remove_event_obj,
    reify_pointers,
    resource_conditional_matches,
    rhs_to_values,
    time_conditional_matches,
    trace_matches,
    variable_substitution,
};
use crate::skel::errors::SkelError;
use crate::skel::metric_names::*;

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
                    ],
                },
            },
        },
    })
}

#[fixture]
fn ctx(test_obj: Value) -> MatchContext {
    MatchContext::new(test_obj)
}

#[fixture]
fn ctx_with_x(mut ctx: MatchContext) -> MatchContext {
    ctx.insert(
        "$x".into(),
        MatchContextEntry::new_from_parts(
            vec!["/spec/template/spec/containers/0".to_string(), "/spec/template/spec/containers/2".to_string()],
            vec![json!({"name": "container1"}), json!({"name": "container2"})],
        ),
    );
    ctx
}

#[rstest]
fn test_process_modify_event_obj_multi_val(mut test_obj: Value, mut ctx: MatchContext) {
    ctx.insert(
        "$x".into(),
        MatchContextEntry::new_from_parts(vec!["/spec/foo".into(), "/spec/bar".into()], vec![/* don't care */]),
    );

    let res = process_modify_event_obj(
        &mut test_obj,
        "/spec/template/spec/containers/0/name",
        &Rhs::Path(PointerBuf::parse("/$x").unwrap()),
        &ctx,
    );

    let x = res.unwrap_err().downcast::<SkelError>().unwrap();
    assert_matches!(x, SkelError::MultipleMatchingValues(..));
}

#[rstest]
fn test_process_modify_event_obj_add(mut test_obj: Value, ctx_with_x: MatchContext) {
    let metrics_recorder = BlackboxRecorder::default();
    let res = metrics::with_local_recorder(&metrics_recorder, || {
        process_modify_event_obj(
            &mut test_obj,
            "/spec/template/spec/containers/1/name",
            &Rhs::Value(json!("foo")),
            &ctx_with_x,
        )
    });
    assert_ok!(res);
    assert_eq!(
        test_obj,
        json!({
            "metadata": {"labels": {"foo": "bar"}},
            "spec": {
                "template": {
                    "spec": {
                        "containers": [
                            {"name": "container1"},
                            {"name": "foo", "image": "asdf:latest"},
                            {"name": "container2"},
                        ],
                    },
                },
            },
        })
    );
    assert_some_eq_x!(metrics_recorder.get(&RESOURCE_MODIFIED_COUNTER.into_counter()), 1);
}

#[rstest]
fn test_process_modify_event_obj_replace(mut test_obj: Value, ctx_with_x: MatchContext) {
    let metrics_recorder = BlackboxRecorder::default();
    let res = metrics::with_local_recorder(&metrics_recorder, || {
        process_modify_event_obj(&mut test_obj, "/$x/name", &Rhs::Value(json!("foo")), &ctx_with_x)
    });
    assert_ok!(res);
    assert_eq!(
        test_obj,
        json!({
            "metadata": {"labels": {"foo": "bar"}},
            "spec": {
                "template": {
                    "spec": {
                        "containers": [
                            {"name": "foo"},
                            {"image": "asdf:latest"},
                            {"name": "foo"},
                        ],
                    },
                },
            },
        })
    );
    assert_some_eq_x!(metrics_recorder.get(&RESOURCE_MODIFIED_COUNTER.into_counter()), 2);
}

#[rstest]
fn test_process_remove_event_obj(mut test_obj: Value, ctx_with_x: MatchContext) {
    let metrics_recorder = BlackboxRecorder::default();
    let res =
        metrics::with_local_recorder(&metrics_recorder, || process_remove_event_obj(&mut test_obj, "/$x", &ctx_with_x));
    assert_ok!(res);
    assert_eq!(
        test_obj,
        json!({
            "metadata": {"labels": {"foo": "bar"}},
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{"image": "asdf:latest"}],
                    },
                },
            },
        })
    );
    assert_some_eq_x!(metrics_recorder.get(&RESOURCE_MODIFIED_COUNTER.into_counter()), 2);
}

#[rstest]
fn test_trace_matches_all(mut ctx: MatchContext) {
    assert_ok_eq_x!(trace_matches(&TraceSelector::All, 1234, &mut ctx), true);
}

#[rstest]
#[case(TestOperation::Eq, false)]
#[case(TestOperation::Ne, true)]
fn test_trace_matches_list(#[case] op: TestOperation, #[case] expected: bool, mut ctx: MatchContext) {
    assert_ok_eq_x!(
        trace_matches(
            &TraceSelector::List(vec![
                Conditional::Time { ts: 1235, op },
                Conditional::Resource {
                    ptr: "/metadata/labels/foo".into(),
                    op: TestOperation::Eq,
                    rhs: Some(Rhs::Value(json!("bar"))),
                    var: None
                }
            ]),
            1234,
            &mut ctx,
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
fn test_time_conditional_matches(#[case] op: TestOperation, #[case] n1: i64, #[case] n2: i64, #[case] expected: bool) {
    assert_eq!(time_conditional_matches(n1, op, n2), expected);
}

#[rstest]
#[case(TestOperation::Eq, "bar", true)]
#[case(TestOperation::Ne, "bar", false)]
#[case(TestOperation::Eq, "baz", false)]
#[case(TestOperation::Ne, "baz", true)]
fn test_resource_conditional_matches(
    #[case] op: TestOperation,
    #[case] condition_value: &str,
    #[case] expected: bool,
    mut ctx: MatchContext,
) {
    assert_eq!(
        resource_conditional_matches(
            "/metadata/labels/foo",
            op,
            &Some(Rhs::Value(json!(condition_value))),
            &None,
            &mut ctx,
        )
        .unwrap(),
        expected
    );
    assert_is_empty!(ctx);
}

#[rstest]
#[case(TestOperation::Exists, true)]
#[case(TestOperation::NotExists, false)]
fn test_resource_conditional_matches_existence(
    #[case] op: TestOperation,
    #[case] expected: bool,
    mut ctx: MatchContext,
) {
    assert_eq!(resource_conditional_matches("/metadata/labels/foo", op, &None, &None, &mut ctx).unwrap(), expected);
    assert_is_empty!(ctx);
}

#[rstest]
#[case(TestOperation::Eq, "container1", true, MatchContextEntry::new_from_parts(
    vec!["/spec/template/spec/containers/0".to_string()],
    vec![json!({"name": "container1"})],
))]
#[case(TestOperation::Ne, "container1", true, MatchContextEntry::new_from_parts(
    vec!["/spec/template/spec/containers/2".to_string()],
    vec![json!({"name": "container2"})],
))]
#[case(TestOperation::Eq, "container3", false, MatchContextEntry::new_from_parts(
    vec![],
    vec![],
))]
#[case(TestOperation::Ne, "container3", true, MatchContextEntry::new_from_parts(
    vec!["/spec/template/spec/containers/0".to_string(), "/spec/template/spec/containers/2".to_string()],
    vec![json!({"name": "container1"}), json!({"name": "container2"})],
))]
fn test_resource_conditional_matches_variable(
    #[case] op: TestOperation,
    #[case] condition_value: &str,
    #[case] expected: bool,
    #[case] expected_ctx_entry: MatchContextEntry,
    mut ctx: MatchContext,
) {
    assert_eq!(
        resource_conditional_matches(
            "/$x/name",
            op,
            &Some(Rhs::Value(json!(condition_value))),
            &Some(VarDef {
                name: "$x".into(),
                pointer: "/spec/template/spec/containers/*".into()
            }),
            &mut ctx,
        )
        .unwrap(),
        expected
    );
    if expected_ctx_entry.len() > 0 {
        assert_eq!(ctx["$x"], expected_ctx_entry);
    } else {
        assert_is_empty!(ctx);
    }
}

#[rstest]
#[case(TestOperation::Exists, true)]
#[case(TestOperation::NotExists, false)]
fn test_resource_conditional_matches_variable_existence(
    #[case] op: TestOperation,
    #[case] expected: bool,
    mut ctx: MatchContext,
) {
    assert_eq!(
        resource_conditional_matches(
            "/$x/name",
            op,
            &None,
            &Some(VarDef {
                name: "$x".into(),
                pointer: "/spec/template/spec/containers/*".into()
            }),
            &mut ctx,
        )
        .unwrap(),
        expected
    );
    if op == TestOperation::Exists {
        assert_eq!(
            ctx["$x"],
            MatchContextEntry::new_from_parts(
                vec!["/spec/template/spec/containers/0".to_string(), "/spec/template/spec/containers/2".to_string()],
                vec![json!({"name": "container1"}), json!({"name": "container2"})],
            )
        )
    } else {
        assert_is_empty!(ctx);
    }
}

#[rstest]
#[case("asdf:latest", TestOperation::Eq, true)]
#[case("asdf:latest", TestOperation::Ne, false)]
#[case("asdf:latest2", TestOperation::Eq, false)]
#[case("asdf:latest2", TestOperation::Ne, true)]
fn test_resource_conditional_matches_other_variable(
    #[case] var_value: &str,
    #[case] op: TestOperation,
    #[case] expected: bool,
    mut ctx: MatchContext,
) {
    ctx.insert(
        "$x".into(),
        MatchContextEntry::new_from_parts(
            vec!["/foo/bar".into(), "/baz/buzz".into()],
            vec![json!({"qwerty": "whatever"}), json!({"value": var_value})],
        ),
    );
    assert_eq!(
        resource_conditional_matches(
            "/$y/image",
            op,
            &Some(Rhs::Path(PointerBuf::parse("/$x/value").unwrap())),
            &Some(VarDef {
                name: "$y".into(),
                pointer: "/spec/template/spec/containers/*".into(),
            }),
            &mut ctx,
        )
        .unwrap(),
        expected
    );
}

#[rstest]
#[case("/$x/foo", "$x", "/metadata/labels")]
#[case("/$x/foo", "$x", "/metadata/labels/")]
fn test_variable_substitution(#[case] input: &str, #[case] var_name: &str, #[case] var_pointer: &str) {
    assert_some_eq_x!(variable_substitution(input, var_name, var_pointer), "/metadata/labels/foo");
}

#[rstest]
fn test_variable_substition_none() {
    assert_none!(variable_substitution("/foo/bar", "$x", "/metadata/labels"));
}

#[rstest]
fn test_reify_pointers_no_var(ctx: MatchContext) {
    let ptrs = reify_pointers("/foo/bar", &ctx);
    assert_ok_eq_x!(&ptrs, &[PointerBuf::parse("/foo/bar").unwrap()]);
}

#[rstest]
fn test_reify_pointers(mut ctx: MatchContext) {
    ctx.insert(
        "$x".into(),
        MatchContextEntry::new_from_parts(vec!["/foo/bar".into(), "/baz/buzz".into()], vec![/* don't care */]),
    );
    ctx.insert("$y".into(), MatchContextEntry::new_from_parts(vec!["/fizz/buzz".into()], vec![/* don't care */]));
    let ptrs = reify_pointers("/$y", &ctx);
    assert_ok_eq_x!(&ptrs, &[PointerBuf::parse("/fizz/buzz").unwrap()]);
}

#[rstest]
#[case(Rhs::Value(json!(42)), vec![json!(42)])]
#[case(Rhs::Path(PointerBuf::parse("/metadata/labels/foo").unwrap()), vec![json!("bar")])]
#[case(Rhs::Path(PointerBuf::parse("/$x/name").unwrap()), vec![json!("container1"), json!("container2")])]
fn test_rhs_to_values(#[case] rhs: Rhs, #[case] expected: Vec<Value>, ctx_with_x: MatchContext) {
    let values = rhs_to_values(&rhs, &ctx_with_x).unwrap();
    assert_all!(values.iter().enumerate(), |(i, v): (usize, &&Value)| **v == expected[i]);
}
