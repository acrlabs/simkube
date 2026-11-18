use assertables::*;
use serde_json::json;
use sk_core::prelude::*;

use super::*;
use crate::validation::rules::deny_list::{job_uid_remove_validator, simkube_io_deny_validator};

#[fixture]
fn depl_event(test_deployment: DynamicObject) -> TraceEvent {
    TraceEvent {
        ts: 2,
        // its probably slightly inefficent but test against two in case there are weird errors there
        applied_objs: vec![test_deployment.clone(), test_deployment.clone()],
        deleted_objs: vec![],
    }
}

#[rstest]
/// validates the rule against the following using a deployment as base
/// if key in [/metadata/labels, /metadata/annotations,
/// pod_spec_template_path/metadata/labels, pod_spec_template_path/metadata/annotations,
/// pod_spec_template_path/spec/nodeSelector, pod_spec_template_path/spec/*/tolerations/key]
#[case(|obj: &mut DynamicObject, deny_key: &str| {
    obj.metadata.annotations = serde_json::from_value(json!({deny_key: "foo"})).unwrap();
},false )]
#[case(|obj: &mut DynamicObject, deny_key: &str| {
    obj.metadata.labels = serde_json::from_value(json!({deny_key: "foo"})).unwrap();
},false )]
#[case(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["metadata"]["labels"] = json!({deny_key: "foo"});
},false )]
#[case::template_metadata_annotation(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["metadata"]["annotations"] = json!({deny_key: "foo"});
},false )]
#[case::template_node_selector(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["spec"]["nodeSelector"] = json!({deny_key: "foo"});
},false )]
#[case::template_star_tolerations_key(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["spec"] = json!([ {"tolerations" : { "key":{deny_key: "foo"}}}]);
},false )]
// this is probably making tests a bit slower then we need since every value hits this case
// dispite it not even taking them
#[case::should_pass_on_wrong_key_in_right_spot(|obj: &mut DynamicObject, _: &str| {
    obj.data["spec"]["template"]["spec"]["nodeSelector"] =
        json!({"not-the-deny-key": "foo"});
},true )]
#[case::should_pass_on_right_key_in_wrong_spot(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["outofscope"] = json!({deny_key: "foo"});
},true )]
fn test_simkube_names_deny(
    mut depl_event: TraceEvent,
    test_trace_config: TracerConfig,
    #[values(
        "simkube.io/kwok-stage-complete-time",
        "simkube.io/kwok-stage-complete-time",
        "simkube.io/kwok-stage-error-time",
        "simkube.io/kwok-stage-ready-delay",
        "simkube.io/kwok-stage-create-delay",
        "simkube.io/kwok-stage-ready-delay-jitter",
        "simkube.io/kwok-stage-create-delay-jitter"
    )]
    deny_key: &str,
    #[case] mk_trace_event: fn(&mut DynamicObject, &str),
    #[case] should_pass: bool,
) {
    //logging of what goes wrong here would be great for debugging
    let mut v = simkube_io_deny_validator();
    mk_trace_event(&mut depl_event.applied_objs[1], deny_key);
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    if should_pass {
        assert_bag_eq!(failed_indices, &[]);
    } else {
        assert_bag_eq!(failed_indices, &[1]);
    }
}
#[fixture]
fn job_depl_event(test_deployment: DynamicObject, test_job: DynamicObject) -> TraceEvent {
    TraceEvent {
        ts: 2,
        // its probably slightly inefficent but test against two in case there are weird errors there
        applied_objs: vec![test_deployment.clone(), test_job.clone()],
        deleted_objs: vec![],
    }
}

#[rstest]
#[case::template_metadat_labels(|obj: &mut DynamicObject, deny_key: &str| {
    obj.data["spec"]["template"]["metadata"]["labels"] = json!({deny_key: "foo"});
})]
#[case::metadata_labels(|obj: &mut DynamicObject, deny_key: &str| {
    obj.metadata.labels = serde_json::from_value(json!({deny_key: "foo"})).unwrap();
})]
fn test_job_uid(
    mut job_depl_event: TraceEvent,
    test_trace_config: TracerConfig,
    #[values(
        "batch.kubernetes.io/controller-uid",
        "batch.kubernetes.io/job-name",
        "controller-uid",
        "job-name"
    )]
    deny_key: &str,
    #[case] mk_trace_event: fn(&mut DynamicObject, &str),
) {
    //logging of what goes wrong here would be great for debugging
    let mut v = job_uid_remove_validator();
    mk_trace_event(&mut job_depl_event.applied_objs[1], deny_key);
    let failed_indices = v.check_next_event(&job_depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}
#[rstest]
fn test_job_uid_validator_scoped_to_job_only(mut job_depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = job_uid_remove_validator();
    job_depl_event.applied_objs[0].data["spec"] = json!({"selector": "foo"});
    job_depl_event.applied_objs[1].data["spec"] = json!({"selector": "foo"});
    let failed_indices = v.check_next_event(&job_depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[1]);
}
#[rstest]
fn test_pass_on_no_annotation(depl_event: TraceEvent, test_trace_config: TracerConfig) {
    let mut v = simkube_io_deny_validator();
    let failed_indices = v.check_next_event(&depl_event, &test_trace_config).unwrap();
    assert_bag_eq!(failed_indices, &[]);
}
