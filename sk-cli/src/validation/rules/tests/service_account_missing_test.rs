use assertables::*;
use serde_json::json;
use sk_store::{
    TraceEvent,
    TracerConfig,
};

use super::service_account_missing::ServiceAccountMissing;
use super::*;

#[fixture]
fn depl_event(test_deployment: DynamicObject, #[default("serviceAccount")] sa_key: &str) -> AnnotatedTraceEvent {
    AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 1,
            applied_objs: vec![test_deployment.data(json!({"spec": {"template": {"spec": {sa_key: "foobar"}}}}))],
            deleted_objs: vec![],
        },
        ..Default::default()
    }
}

#[fixture]
fn sa_event(test_service_account: DynamicObject) -> AnnotatedTraceEvent {
    AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 0,
            applied_objs: vec![test_service_account],
            deleted_objs: vec![],
        },
        ..Default::default()
    }
}

#[rstest]
#[case("serviceAccount")]
#[case("serviceAccountName")]
fn test_service_account_missing(test_deployment: DynamicObject, test_trace_config: TracerConfig, #[case] sa_key: &str) {
    let mut v = ServiceAccountMissing::default();
    let mut evt = depl_event(test_deployment, sa_key);
    let annotations = v.check_next_event(&mut evt, &test_trace_config).unwrap();

    assert_eq!(annotations.keys().collect::<Vec<_>>(), vec![&0]);
}

#[rstest]
fn test_service_account_missing_deleted(
    mut depl_event: AnnotatedTraceEvent,
    mut sa_event: AnnotatedTraceEvent,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let mut v = ServiceAccountMissing::default();
    let mut sa_event_del = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 0,
            applied_objs: vec![],
            deleted_objs: vec![test_service_account],
        },
        ..Default::default()
    };
    v.check_next_event(&mut sa_event, &test_trace_config).unwrap();
    v.check_next_event(&mut sa_event_del, &test_trace_config).unwrap();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_eq!(annotations.keys().collect::<Vec<_>>(), vec![&0]);
}

#[rstest]
fn test_service_account_not_missing(
    mut depl_event: AnnotatedTraceEvent,
    mut sa_event: AnnotatedTraceEvent,
    test_trace_config: TracerConfig,
) {
    let mut v = ServiceAccountMissing::default();
    v.check_next_event(&mut sa_event, &test_trace_config).unwrap();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_eq!(annotations.keys().collect::<Vec<_>>(), vec![&0]);
}

#[rstest]
fn test_service_account_not_missing_same_evt(
    test_deployment: DynamicObject,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let mut v = ServiceAccountMissing::default();
    let mut depl_evt = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 1,
            applied_objs: vec![
                test_deployment
                    .data(json!({"spec": {"template": {"spec": {"serviceAccountName": TEST_SERVICE_ACCOUNT}}}})),
                test_service_account,
            ],
            deleted_objs: vec![],
        },
        ..Default::default()
    };
    let annotations = v.check_next_event(&mut depl_evt, &test_trace_config).unwrap();

    assert_eq!(annotations.keys().collect::<Vec<_>>(), vec![&0]);
}

#[rstest]
fn test_service_account_reset(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let mut v = ServiceAccountMissing::default();
    v.check_next_event(&mut depl_event, &test_trace_config).unwrap();
    v.reset();

    assert_is_empty!(v.seen_service_accounts);
}
