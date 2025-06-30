use assertables::*;
use json_patch_ext::PatchOperation::Remove;
use json_patch_ext::prelude::*;
use k8s_openapi::api::apps::v1::Deployment;
use serde_json::json;
use sk_store::{
    TraceAction,
    TraceEvent,
    TracerConfig,
};

use super::missing_resources::{
    MissingResource,
    MissingResourceType,
    configmap_envvar_validator,
    configmap_volume_validator,
    secret_envvar_validator,
    secret_volume_validator,
    service_account_validator,
};
use super::*;
use crate::validation::PatchLocations;
use crate::validation::validator::CheckResult;

#[fixture]
fn depl_event(test_deployment: DynamicObject, #[default("serviceAccount")] sa_key: &str) -> AnnotatedTraceEvent {
    AnnotatedTraceEvent {
        data: TraceEvent {
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

fn check_sa_event_annotations(annotations_result: CheckResult, keys: &[&str]) {
    let annotations = annotations_result.unwrap();
    // There should be one patch for each key
    for (i, key) in keys.iter().enumerate() {
        let (index, patch) = &annotations[i];

        // Each patch should target the first object
        assert_eq!(*index, 0);
        // There should be two possible fixes for the element
        assert_len_eq_x!(patch, 2);

        // Check the first fix
        match &patch[0].locations {
            PatchLocations::ObjectReference(tm, ns_name) => {
                assert_eq!(tm, &Deployment::type_meta());
                assert_eq!(ns_name, &format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}"));
            },
            _ => panic!("unexpected location variant"),
        }
        assert_matches!(patch[0].ops[..], [Remove(..)]);
        assert_eq!(patch[0].ops[0].path().as_str(), format!("/spec/template/spec/{key}"));

        // Check the second fix
        match &patch[1].locations {
            PatchLocations::InsertAt(0, TraceAction::ObjectApplied, t, ..) => assert_eq!(t.kind, "ServiceAccount"),
            _ => panic!("bad insert location"),
        }
        assert_matches!(patch[1].ops[..], []);
    }
}

#[rstest]
#[case("serviceAccount")]
#[case("serviceAccountName")]
fn test_service_account_missing(test_deployment: DynamicObject, test_trace_config: TracerConfig, #[case] sa_key: &str) {
    let v = service_account_validator();
    let mut evt = depl_event(test_deployment, sa_key);
    let annotations = v.check_next_event(&mut evt, &test_trace_config);

    check_sa_event_annotations(annotations, &[&sa_key])
}

#[rstest]
fn test_service_account_missing_both_keys(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let v = service_account_validator();
    depl_event.data.applied_objs[0].data["spec"]["template"]["spec"]["serviceAccountName"] = "foobar".into();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config);

    check_sa_event_annotations(annotations, &["serviceAccount", "serviceAccountName"])
}

#[rstest]
fn test_service_account_missing_deleted(
    mut depl_event: AnnotatedTraceEvent,
    mut sa_event: AnnotatedTraceEvent,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let v = service_account_validator();
    let mut sa_event_del = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 1,
            applied_objs: vec![],
            deleted_objs: vec![test_service_account],
        },
        ..Default::default()
    };
    v.check_next_event(&mut sa_event, &test_trace_config).unwrap();
    v.check_next_event(&mut sa_event_del, &test_trace_config).unwrap();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config);

    check_sa_event_annotations(annotations, &["serviceAccount"])
}

#[rstest]
fn test_service_account_not_missing(
    mut depl_event: AnnotatedTraceEvent,
    mut sa_event: AnnotatedTraceEvent,
    test_trace_config: TracerConfig,
) {
    let v = service_account_validator();
    v.check_next_event(&mut sa_event, &test_trace_config).unwrap();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_none!(annotations.get(0));
}

#[rstest]
fn test_service_account_not_missing_same_evt(
    test_deployment: DynamicObject,
    test_service_account: DynamicObject,
    test_trace_config: TracerConfig,
) {
    let v = service_account_validator();
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

    assert_none!(annotations.get(0));
}

// I'm using the service-account tests to check the various permutations in this code
// so I'm not going to re-duplicate them for secrets/etc.  Here I'm just checking that
// the produced patches are correct.
#[rstest]
fn test_secret_envvar_missing(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let v = secret_envvar_validator();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_len_eq_x!(&annotations, 3); // three secrets should be patched out

    // all annotations should apply to the first object in the event
    assert_eq!(annotations[0].0, 0);
    assert_eq!(annotations[1].0, 0);
    assert_eq!(annotations[2].0, 0);

    // Check the secret paths -- this is a truly cursed set of indices
    assert_eq!(annotations[0].1[0].ops[0], remove_operation(format_ptr!("/spec/template/spec/containers/0/env/0")));
    assert_eq!(annotations[1].1[0].ops[0], remove_operation(format_ptr!("/spec/template/spec/containers/0/env/2")));
    assert_eq!(
        annotations[2].1[0].ops[0],
        remove_operation(format_ptr!("/spec/template/spec/containers/0/envFrom/1"))
    );
}

#[rstest]
fn test_configmap_envvar_missing(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let v = configmap_envvar_validator();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_len_eq_x!(&annotations, 2); // two configmaps should be patched out

    // both annotations should apply to the first object in the event
    assert_eq!(annotations[0].0, 0);
    assert_eq!(annotations[1].0, 0);

    // Check the configmap paths -- this is a truly cursed set of indices
    assert_eq!(annotations[0].1[0].ops[0], remove_operation(format_ptr!("/spec/template/spec/containers/0/env/3")));
    assert_eq!(
        annotations[1].1[0].ops[0],
        remove_operation(format_ptr!("/spec/template/spec/containers/0/envFrom/0"))
    );
}

#[rstest]
fn test_secret_volume_missing(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let v = secret_volume_validator();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_len_eq_x!(&annotations, 1); // one secret volume should be patched out

    // the annotation should apply to the first object in the event
    assert_eq!(annotations[0].0, 0);

    // Check the secret paths -- this is a truly cursed set of indices
    assert_eq!(annotations[0].1[0].ops[0], remove_operation(format_ptr!("/spec/template/spec/volumes/1/secret")));
    assert_eq!(
        annotations[0].1[0].ops[1],
        add_operation(format_ptr!("/spec/template/spec/volumes/1/emptyDir"), json!({}))
    );
}

#[rstest]
fn test_configmap_volume_missing(mut depl_event: AnnotatedTraceEvent, test_trace_config: TracerConfig) {
    let v = configmap_volume_validator();
    let annotations = v.check_next_event(&mut depl_event, &test_trace_config).unwrap();

    assert_len_eq_x!(&annotations, 1); // one configmap volume should be patched out

    // the annotation should apply to the first object in the event
    assert_eq!(annotations[0].0, 0);

    // Check the configmap paths -- this is a truly cursed set of indices
    assert_eq!(annotations[0].1[0].ops[0], remove_operation(format_ptr!("/spec/template/spec/volumes/2/configMap")));
    assert_eq!(
        annotations[0].1[0].ops[1],
        add_operation(format_ptr!("/spec/template/spec/volumes/2/emptyDir"), json!({}))
    );
}

fn add_service_account_to_pod_spec(obj: &mut DynamicObject, pod_spec_template_key: &str) {
    obj.data
        .as_object_mut()
        .unwrap()
        .get_mut("spec")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut(pod_spec_template_key)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("spec")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("serviceAccount".into(), json!(TEST_SERVICE_ACCOUNT));
}

#[rstest]
fn test_multiple_pod_spec_templates(test_trace_config_two_pods: TracerConfig, mut test_two_pods_obj: DynamicObject) {
    let v = service_account_validator();
    add_service_account_to_pod_spec(&mut test_two_pods_obj, "template1");
    add_service_account_to_pod_spec(&mut test_two_pods_obj, "template2");

    let mut evt = AnnotatedTraceEvent {
        data: TraceEvent {
            ts: 2,
            applied_objs: vec![test_two_pods_obj],
            deleted_objs: vec![],
        },
        ..Default::default()
    };

    let annotations = v.check_next_event(&mut evt, &test_trace_config_two_pods).unwrap();

    assert_len_eq_x!(&annotations, 2);
    assert_eq!(annotations[0].1[0].ops[0].path().as_str(), "/spec/template1/spec/serviceAccount");
    assert_eq!(annotations[1].1[0].ops[0].path().as_str(), "/spec/template2/spec/serviceAccount");
}

#[rstest]
fn test_missing_resources_reset() {
    let mut v = MissingResource::<corev1::ServiceAccount>::new(vec!["foo", "bar"], MissingResourceType::TopLevel);
    v.seen_resources.insert("asdf".into());
    v.reset();

    assert_is_empty!(v.seen_resources);
}
