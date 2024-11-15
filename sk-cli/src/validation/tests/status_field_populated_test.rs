use assertables::*;
use kube::api::DynamicObject;
use serde_json::json;
use sk_store::TraceEvent;

use super::*;

#[rstest]
fn test_status_field_populated(test_deployment: DynamicObject) {
    let mut v = status_field_populated::validator();
    let mut evt = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 1,
            applied_objs: vec![test_deployment.data(json!({"status": {"availableReplicas": 3}}))],
            deleted_objs: vec![],
        },
        ..Default::default()
    };
    let indices = v.check_next_event(&mut evt);
    assert_bag_eq!(indices, &[0]);
}

#[rstest]
fn test_status_field_not_populated(test_deployment: DynamicObject) {
    let mut v = status_field_populated::validator();
    let mut evt = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 1,
            applied_objs: vec![test_deployment.data(json!({}))],
            deleted_objs: vec![],
        },
        ..Default::default()
    };
    let indices = v.check_next_event(&mut evt);
    assert_is_empty!(indices);
}
