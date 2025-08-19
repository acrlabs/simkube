use assertables::*;
use clockabilly::Utc;
use serde_json as json;

use super::*;

#[rstest]
fn test_sanitize_obj() {
    let mut obj = DynamicObject {
        metadata: metav1::ObjectMeta {
            name: Some("test-obj".into()),
            namespace: Some(TEST_NAMESPACE.into()),

            annotations: klabel!(
                "some_random_annotation" => "blah",
                LAST_APPLIED_CONFIG_LABEL_KEY => "foo",
                DEPL_REVISION_LABEL_KEY => "42.5",
            ),

            creation_timestamp: Some(metav1::Time(Utc::now())),
            deletion_timestamp: Some(metav1::Time(Utc::now())),
            deletion_grace_period_seconds: Some(123),
            generation: Some(456),
            managed_fields: Some(vec![Default::default()]),
            owner_references: Some(vec![Default::default()]),
            resource_version: Some("1234".into()),
            uid: Some("abcd".into()),

            ..Default::default()
        },
        types: None,
        data: json::Value::Null,
    };

    sanitize_obj(&mut obj, "bar.blah.sh/v2", "Stuff");

    assert_some!(obj.metadata.owner_references);

    assert_none!(obj.metadata.creation_timestamp);
    assert_none!(obj.metadata.deletion_timestamp);
    assert_none!(obj.metadata.deletion_grace_period_seconds);
    assert_none!(obj.metadata.generation);
    assert_none!(obj.metadata.managed_fields);
    assert_none!(obj.metadata.resource_version);
    assert_none!(obj.metadata.uid);

    assert_eq!(obj.metadata.annotations, klabel!("some_random_annotation" => "blah"));
    assert!(
        obj.types
            .is_some_and(|tm| tm.api_version == "bar.blah.sh/v2" && tm.kind == "Stuff")
    );
}

fn build_label_sel(key: &str, op: &str, value: Option<&str>) -> metav1::LabelSelector {
    metav1::LabelSelector {
        match_expressions: Some(vec![metav1::LabelSelectorRequirement {
            key: key.into(),
            operator: op.into(),
            values: match value {
                Some(s) => Some(vec![s.into()]),
                _ => None,
            },
        }]),
        ..Default::default()
    }
}

#[rstest]
#[case::op_in(OPERATOR_IN)]
#[case::op_not_in(OPERATOR_NOT_IN)]
fn test_label_expr_match(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("foo", op, Some("bar"));
    let res = test_pod.matches(&sel).unwrap();
    assert_eq!(res, op == OPERATOR_IN);
}

#[rstest]
#[case::op_in(OPERATOR_IN)]
#[case::op_not_in(OPERATOR_NOT_IN)]
fn test_label_expr_no_values(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("foo", op, None);
    let res = test_pod.matches(&sel).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
}

#[rstest]
#[case::op_in(OPERATOR_IN)]
#[case::op_not_in(OPERATOR_NOT_IN)]
fn test_label_expr_no_match(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("baz", op, Some("qux"));
    let res = test_pod.matches(&sel).unwrap();
    assert_eq!(res, op == OPERATOR_NOT_IN);
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS)]
#[case::op_exists(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_exists(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("foo", op, None);
    let res = test_pod.matches(&sel).unwrap();
    assert_eq!(res, op == OPERATOR_EXISTS);
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS)]
#[case::op_not_exists(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_exists_values(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("foo", op, Some("bar"));
    let res = test_pod.matches(&sel).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
}

#[rstest]
#[case::op_in(OPERATOR_EXISTS)]
#[case::op_not_in(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_not_exists(test_pod: corev1::Pod, #[case] op: &str) {
    let sel = build_label_sel("baz", op, None);
    let res = test_pod.matches(&sel).unwrap();
    assert_eq!(res, op == OPERATOR_DOES_NOT_EXIST);
}

#[rstest]
#[case::label_match("foo".into())]
#[case::label_no_match("baz".into())]
fn test_label_match(test_pod: corev1::Pod, #[case] label_key: String) {
    let sel = metav1::LabelSelector {
        match_labels: klabel!(label_key => "bar"),
        ..Default::default()
    };
    let res = test_pod.matches(&sel).unwrap();
    assert_eq!(res, &label_key == "foo");
}
