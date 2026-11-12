use assertables::*;
use serde_json::json;
use sk_core::prelude::*;

use super::*;
use crate::validation::rules::deny_list::{
    simkube_io_pod_template_deny_validator,
    simkube_io_resource_deny_validator,
};

#[fixture]
fn depl_event(test_deployment: DynamicObject, #[default("serviceAccount")] sa_key: &str) -> TraceEvent {
    TraceEvent {
        ts: 2,
        applied_objs: vec![
            test_deployment.clone(),
            test_deployment.data(json!({
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
            })),
        ],
        deleted_objs: vec![],
    }
}

// pod template deny validator
#[rstest]
fn test_pass_on_pod_no_simkube_annotation(depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = simkube_io_pod_template_deny_validator();
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[]);
}
#[rstest]
fn test_pass_on_pod_simkube_annotation_not_in_pod_spec(mut depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = simkube_io_pod_template_deny_validator();
    depl_event.applied_objs[1].data["metadata"]["annotations"] = json!({"simkube.io/kwok-stage-complete-time": "2s"});
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[]);
}

#[rstest]
#[case("simkube.io/kwok-stage-complete-time")]
#[case("simkube.io/kwok-stage-complete-time")]
#[case("simkube.io/kwok-stage-error-time")]
#[case("simkube.io/kwok-stage-ready-delay")]
#[case("simkube.io/kwok-stage-create-delay")]
#[case("simkube.io/kwok-stage-ready-delay-jitter")]
#[case("simkube.io/kwok-stage-create-delay-jitter")]
fn test_fail_on_pod_simkube_annotation(
    mut depl_event: TraceEvent,
    test_trace_config: TracerConfig,
    #[case] deny_key: &str,
) {
    use crate::validation::rules::deny_list::simkube_io_pod_template_deny_validator;

    let mut v = simkube_io_pod_template_deny_validator();
    depl_event.applied_objs[1].data["spec"]["template"]["metadata"]["annotations"] = json!({deny_key: "2s"});
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}

#[rstest]
#[case("simkube.io/kwok-stage-complete")]
fn test_fail_on_pod_simkube_label(mut depl_event: TraceEvent, test_trace_config: TracerConfig, #[case] deny_key: &str) {
    let mut v = simkube_io_pod_template_deny_validator();
    depl_event.applied_objs[1].data["spec"]["template"]["metadata"]["labels"] = json!({deny_key: true});
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}

// resource deny validator
#[rstest]
#[case("simkube.io/kwok-stage-complete-time")]
#[case("simkube.io/kwok-stage-complete-time")]
#[case("simkube.io/kwok-stage-error-time")]
#[case("simkube.io/kwok-stage-ready-delay")]
#[case("simkube.io/kwok-stage-create-delay")]
#[case("simkube.io/kwok-stage-ready-delay-jitter")]
#[case("simkube.io/kwok-stage-create-delay-jitter")]
fn test_fail_on_resource_simkube_annotation(
    mut depl_event: TraceEvent,
    test_trace_config: TracerConfig,
    #[case] deny_key: &str,
) {
    let mut v = simkube_io_resource_deny_validator();
    depl_event.applied_objs[1].data["metadata"]["annotations"] = json!({deny_key: "2s"});
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}
#[rstest]
fn test_pass_on_resource_no_simkube_annotation(depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = simkube_io_resource_deny_validator();
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[]);
}

#[rstest]
#[case("simkube.io/kwok-stage-complete")]
fn test_fail_on_resource_simkube_label(
    mut depl_event: TraceEvent,
    test_trace_config: TracerConfig,
    #[case] deny_key: &str,
) {
    let mut v = simkube_io_resource_deny_validator();
    depl_event.applied_objs[1].data["metadata"]["labels"] = json!({deny_key: true});
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}
