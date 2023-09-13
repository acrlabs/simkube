use std::collections::BTreeMap;

use chrono::Utc;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use rstest::*;
use serde_json::json;

use super::k8s::*;

#[rstest]
fn test_strip_obj() {
    let mut obj = DynamicObject {
        metadata: metav1::ObjectMeta {
            name: Some("test".into()),
            namespace: Some("test".into()),
            uid: Some("abcd".into()),
            resource_version: Some("1234".into()),
            managed_fields: Some(vec![Default::default()]),
            creation_timestamp: Some(metav1::Time(Utc::now())),
            deletion_timestamp: Some(metav1::Time(Utc::now())),
            owner_references: Some(vec![Default::default()]),
            ..Default::default()
        },
        types: None,
        data: json!({
            "foo": {
                "bars": [{
                    "spec": {
                        "nodeName": "foo",
                        "serviceAccountName": "bar",
                        "nodeSelector": {"buz": "biz"},
                    },
                },
                {
                    "spec": {},
                },
                {
                    "spec": {
                        "serviceAccount": "flumm",
                    },
                },
                ],
            },
        }),
    };

    strip_obj(&mut obj, "/foo/bars/*/spec");
    assert_eq!(obj.metadata.uid, None);
    assert_eq!(obj.metadata.resource_version, None);
    assert_eq!(obj.metadata.managed_fields, None);
    assert_eq!(obj.metadata.creation_timestamp, None);
    assert_eq!(obj.metadata.deletion_timestamp, None);
    assert_eq!(obj.metadata.owner_references, None);
    assert_eq!(
        obj.data,
        json!({
            "foo": {
                "bars": [
                {
                    "spec": {
                        "nodeSelector": {"buz": "biz"},
                    },
                },
                { "spec": {} },
                { "spec": {} },
                ],
            },
        })
    );
}

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
    let res = label_expr_match(&pod_labels, &label_expr).unwrap();
    assert_eq!(res, &op == OPERATOR_IN);
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
    let res = label_expr_match(&pod_labels, &label_expr).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
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
    let res = label_expr_match(&pod_labels, &label_expr).unwrap();
    assert_eq!(res, &op == OPERATOR_NOT_IN);
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
    let res = label_expr_match(&pod_labels, &label_expr).unwrap();
    assert_eq!(res, &op == OPERATOR_EXISTS);
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS.into())]
#[case::op_not_exists(OPERATOR_DOES_NOT_EXIST.into())]
fn test_label_expr_exists_values(pod_labels: BTreeMap<String, String>, #[case] op: String) {
    let label_expr = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: op.clone(),
        values: Some(vec!["bar".into()]),
    };
    let res = label_expr_match(&pod_labels, &label_expr).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
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
    let res = label_expr_match(&pod_labels, &label_expr).unwrap();
    assert_eq!(res, &op == OPERATOR_DOES_NOT_EXIST);
}

#[rstest]
#[case::label_match("foo".into())]
#[case::label_no_match("baz".into())]
fn test_label_match(pod: corev1::Pod, #[case] label_key: String) {
    let sel = metav1::LabelSelector {
        match_labels: Some(BTreeMap::from([(label_key.clone(), "bar".into())])),
        ..Default::default()
    };
    let res = obj_matches_selector(&pod, &sel).unwrap();
    assert_eq!(res, &label_key == "foo");
}
