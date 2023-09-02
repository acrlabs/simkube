use std::collections::BTreeMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use rstest::*;

use super::k8s::*;

#[rstest]
fn test_label_expr_match() {
    let pod_labels = BTreeMap::from([("foo".to_string(), "bar".to_string())]);
    let label_selector = metav1::LabelSelectorRequirement {
        key: "foo".into(),
        operator: OPERATOR_IN.into(),
        values: Some(vec!["bar".into()]),
    };
    let res = label_expr_match(&pod_labels, &label_selector);
    assert_eq!(true, res.unwrap());
}
