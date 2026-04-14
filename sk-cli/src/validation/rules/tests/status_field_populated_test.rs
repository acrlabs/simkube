use assertables::*;
use serde_json::json;
use sk_store::TraceEvent;

use super::*;

#[rstest]
fn test_status_field_populated(test_deployment: DynamicObject) {
    let mut v = status_field_populated::validator();
    let evt = TraceEvent {
        ts: 1,
        applied_objs: vec![test_deployment.data(json!({"status": {"availableReplicas": 3}}))],
        deleted_objs: vec![],
    };
    let failed_indices = v.check_next_event(&evt, &Default::default()).unwrap();
    assert_bag_eq!(failed_indices, &[0]);
}

#[rstest]
fn test_status_field_not_populated(test_deployment: DynamicObject) {
    let mut v = status_field_populated::validator();
    let evt = TraceEvent {
        ts: 1,
        applied_objs: vec![test_deployment.data(json!({}))],
        deleted_objs: vec![],
    };
    let annotations = v.check_next_event(&evt, &Default::default()).unwrap();
    assert_is_empty!(annotations);
}
