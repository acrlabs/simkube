use assertables::*;
use serde_json::json;
use sk_store::{
    TraceEvent,
    TracerConfig,
};

use super::missing_resources::service_account_validator;
use super::*;

#[fixture]
fn depl_event(test_deployment: DynamicObject, #[default("serviceAccount")] sa_key: &str) -> TraceEvent {
    TraceEvent {
        ts: 2,
        applied_objs: vec![test_deployment.data(json!({
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{
                            "env": [
                                {
                                    "name": "SECRET1",
                                    "valueFrom": {"secretKeyRef": {"key": "secretKey1"}},
                                },
                                {"name": "FOOENV", "value": "bar"},
                                {
                                    "name": "SECRET2",
                                    "valueFrom": {"secretKeyRef": {"key": "secretKey2"}},
                                },
                                {
                                    "name": "CONFIGMAP1",
                                    "valueFrom": {"configMapKeyRef": {"key": "configMap"}},
                                },
                            ],
                            "envFrom": [
                                {"configMapRef": {"name": "fooconfig"}},
                                {"secretRef": {"name": "secretRef"}},
                            ],
                        }],
                        sa_key: TEST_SERVICE_ACCOUNT,
                        "volumes": [
                            {"name": "volume1", "hostPath": {}},
                            {"name": "secretVolume", "secret": {"secretName": "secret"}},
                            {"name": "configVolume", "configMap": {"name": "configMap"}},
                        ],
                    }
                }
            }
        }))],
        deleted_objs: vec![],
    }
}

#[fixture]
fn sa_event(test_service_account: DynamicObject) -> TraceEvent {
    TraceEvent {
        ts: 0,
        applied_objs: vec![test_service_account],
        deleted_objs: vec![],
    }
}

#[rstest]
#[case("serviceAccount")]
#[case("serviceAccountName")]
fn test_service_account_missing(test_deployment: DynamicObject, test_trace_config: TracerConfig, #[case] sa_key: &str) {
    let mut v = service_account_validator();
    let mut evt = depl_event(test_deployment, sa_key);
    let failed_indices = v.check_next_event(&mut evt, &test_trace_config).unwrap();

    assert_bag_eq!(failed_indices, &[0]);
}

#[rstest]
fn test_service_account_missing_both_keys(mut depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = service_account_validator();
    depl_event.applied_objs[0].data["spec"]["template"]["spec"]["serviceAccountName"] = "foobar".into();
    let failed_indices = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_bag_eq!(failed_indices, &[0, 0]);
}

#[rstest]
fn test_service_account_missing_deleted(
    depl_event: TraceEvent,
    sa_event: TraceEvent,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let mut v = service_account_validator();
    let sa_event_del = TraceEvent {
        ts: 1,
        applied_objs: vec![],
        deleted_objs: vec![test_service_account],
    };
    v.check_next_event(&sa_event, &test_trace_config).unwrap();
    v.check_next_event(&sa_event_del, &test_trace_config).unwrap();
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();

    assert_bag_eq!(failed_indices, &[0]);
}

#[rstest]
fn test_service_account_not_missing(depl_event: TraceEvent, sa_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = service_account_validator();
    v.check_next_event(&sa_event, &test_trace_config).unwrap();
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();

    assert_is_empty!(failed_indices);
}

#[rstest]
fn test_service_account_not_missing_same_evt(
    test_deployment: DynamicObject,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let mut v = service_account_validator();
    let mut depl_evt = TraceEvent {
        ts: 1,
        applied_objs: vec![
            test_deployment.data(json!({"spec": {"template": {"spec": {"serviceAccountName": TEST_SERVICE_ACCOUNT}}}})),
            test_service_account,
        ],
        deleted_objs: vec![],
    };
    let failed_indices = v.check_next_event(&mut depl_evt, &test_trace_config).unwrap();

    assert_is_empty!(failed_indices);
}

// I'm using the service-account tests to check the various permutations in this code
// so I'm not going to re-duplicate them for secrets/etc.
