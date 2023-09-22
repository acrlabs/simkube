use std::collections::BTreeMap;

use chrono::Utc;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use rstest::*;
use serde_json::json;

use super::util::*;

#[rstest]
fn test_sanitize_obj() {
    let mut obj = DynamicObject {
        metadata: metav1::ObjectMeta {
            name: Some("test".into()),
            namespace: Some("test".into()),

            annotations: Some(BTreeMap::from([
                ("some_random_annotation".into(), "blah".into()),
                (LAST_APPLIED_CONFIG_LABEL_KEY.into(), "foo".into()),
                (DEPL_REVISION_LABEL_KEY.into(), "42.5".into()),
            ])),

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

    sanitize_obj(&mut obj, "/foo/bars/*/spec", "bar.blah.sh/v2", "Stuff");

    assert_eq!(obj.metadata.creation_timestamp, None);
    assert_eq!(obj.metadata.deletion_timestamp, None);
    assert_eq!(obj.metadata.deletion_grace_period_seconds, None);
    assert_eq!(obj.metadata.generation, None);
    assert_eq!(obj.metadata.managed_fields, None);
    assert_eq!(obj.metadata.owner_references, None);
    assert_eq!(obj.metadata.resource_version, None);
    assert_eq!(obj.metadata.uid, None);

    assert_eq!(obj.metadata.annotations, Some(BTreeMap::from([("some_random_annotation".into(), "blah".into())])));
    assert!(obj
        .types
        .is_some_and(|tm| tm.api_version == "bar.blah.sh/v2" && tm.kind == "Stuff"));

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
fn pod() -> corev1::Pod {
    let labels = Some(BTreeMap::from([("foo".into(), "bar".to_string())]));
    corev1::Pod {
        metadata: metav1::ObjectMeta { labels, ..Default::default() },
        ..Default::default()
    }
}

fn make_label_sel(key: &str, op: &str, value: Option<&str>) -> metav1::LabelSelector {
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
fn test_label_expr_match(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("foo", op, Some("bar"));
    let res = obj_matches_selector(&pod, &sel).unwrap();
    assert_eq!(res, op == OPERATOR_IN);
}

#[rstest]
#[case::op_in(OPERATOR_IN)]
#[case::op_not_in(OPERATOR_NOT_IN)]
fn test_label_expr_no_values(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("foo", op, None);
    let res = obj_matches_selector(&pod, &sel).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
}

#[rstest]
#[case::op_in(OPERATOR_IN)]
#[case::op_not_in(OPERATOR_NOT_IN)]
fn test_label_expr_no_match(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("baz", op, Some("qux"));
    let res = obj_matches_selector(&pod, &sel).unwrap();
    assert_eq!(res, op == OPERATOR_NOT_IN);
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS)]
#[case::op_exists(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_exists(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("foo", op, None);
    let res = obj_matches_selector(&pod, &sel).unwrap();
    assert_eq!(res, op == OPERATOR_EXISTS);
}

#[rstest]
#[case::op_exists(OPERATOR_EXISTS)]
#[case::op_not_exists(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_exists_values(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("foo", op, Some("bar"));
    let res = obj_matches_selector(&pod, &sel).unwrap_err().downcast().unwrap();
    assert!(matches!(res, KubernetesError::MalformedLabelSelector(_)));
}

#[rstest]
#[case::op_in(OPERATOR_EXISTS)]
#[case::op_not_in(OPERATOR_DOES_NOT_EXIST)]
fn test_label_expr_not_exists(pod: corev1::Pod, #[case] op: &str) {
    let sel = make_label_sel("baz", op, None);
    let res = obj_matches_selector(&pod, &sel).unwrap();
    assert_eq!(res, op == OPERATOR_DOES_NOT_EXIST);
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
