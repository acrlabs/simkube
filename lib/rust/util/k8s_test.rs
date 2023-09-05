use std::collections::BTreeMap;

use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use rstest::*;

use super::k8s::*;
use crate::prelude::*;

#[fixture]
fn pod_labels() -> BTreeMap<String, String> {
    return BTreeMap::from([("foo".into(), "bar".to_string())]);
}

#[fixture]
fn pod(pod_labels: BTreeMap<String, String>) -> corev1::Pod {
    corev1::Pod {
        metadata: metav1::ObjectMeta { labels: Some(pod_labels), ..Default::default() },
        ..Default::default()
    }
}

#[rstest]
#[case::op_in(OPERATOR_IN.into())]
#[case::op_not_in(OPERATOR_NOT_IN.into())]
fn test_label_expr_match(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: op.clone(),
        values: Some(vec!["bar".into()]),
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(&op == OPERATOR_IN, res.unwrap());
}

#[rstest]
#[case::op_in(OPERATOR_IN.into())]
#[case::op_not_in(OPERATOR_NOT_IN.into())]
fn test_label_expr_no_values(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: op.clone(),
        values: Some(vec![]),
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(Err(SimKubeError::MalformedLabelSelector), res);
}

#[rstest]
#[case::op_in(OPERATOR_IN.into())]
#[case::op_not_in(OPERATOR_NOT_IN.into())]
fn test_label_expr_no_match(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "baz".into(),
        operator: op.clone(),
        values: Some(vec!["qux".into()]),
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(&op == OPERATOR_NOT_IN, res.unwrap());
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS.into())]
#[case::op_exists(OPERATOR_DOES_NOT_EXIST.into())]
fn test_label_expr_exists(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: op.clone(),
        values: None,
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(&op == OPERATOR_EXISTS, res.unwrap());
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS.into())]
#[case::op_exists(OPERATOR_DOES_NOT_EXIST.into())]
fn test_label_expr_exists_values(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: op.clone(),
        values: Some(vec!["bar".into()]),
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(Err(SimKubeError::MalformedLabelSelector), res);
}

#[rstest]
#[case::op_in(OPERATOR_EXISTS.into())]
#[case::op_not_in(OPERATOR_DOES_NOT_EXIST.into())]
fn test_label_expr_not_exists(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "baz".into(),
        operator: op.clone(),
        values: None,
    };
    let res = label_expr_match(&pod_labels, &label_expr);
    assert_eq!(&op == OPERATOR_DOES_NOT_EXIST, res.unwrap());
}

#[rstest]
#[case::label_match("foo".into())]
#[case::label_no_match("baz".into())]
fn test_label_match(pod: corev1::Pod, #[case] label_key: String) {
    let sel = metav1::LabelSelector {
        match_labels: Some(BTreeMap::from([(label_key.clone(), "bar".into())])),
        ..Default::default()
    };
    let res = pod_matches_selector(&pod, &sel);
    assert_eq!(&label_key == "foo", res.unwrap());
}
