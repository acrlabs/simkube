use std::collections::{
    BTreeMap,
    HashMap,
};

use clockabilly::mock::MockUtcClock;
use insta::assert_debug_snapshot;
use json_patch_ext::prelude::*;
use kube::core::admission::{
    AdmissionRequest,
    AdmissionResponse,
    AdmissionReview,
    Operation as AdmissionOperation,
};
use kube::core::{
    GroupVersionKind,
    GroupVersionResource,
};
use rocket::serde::json::Json;
use serde_json::json;
use sk_core::k8s::PodLifecycleData;
use sk_core::prelude::*;
use tracing_test::traced_test;

use super::helpers::build_driver_context;
use super::*;

#[fixture]
fn ctx(
    test_pod: corev1::Pod,
    #[default(vec![])] pod_owners: Vec<metav1::OwnerReference>,
    #[default(Trace::default())] trace: Trace,
) -> DriverContext {
    let (_, client) = make_fake_apiserver();
    ctx_with_client(test_pod, client, pod_owners, trace)
}

fn ctx_with_client(
    test_pod: corev1::Pod,
    client: kube::Client,
    pod_owners: Vec<metav1::OwnerReference>,
    trace: Trace,
) -> DriverContext {
    let mut owners = HashMap::new();
    owners.insert((corev1::Pod::gvk(), test_pod.namespaced_name()), pod_owners);
    let cache = OwnersCache::new_from_parts(DynamicApiSet::new(client.clone()), owners);
    build_driver_context(cache, trace, client)
}

#[fixture]
fn adm_req(test_pod: corev1::Pod) -> AdmissionRequest<corev1::Pod> {
    let gvr = GroupVersionResource::gvr("".into(), "v1".into(), "pods".into());
    let gvk = GroupVersionKind::gvk("".into(), "v1".into(), "Pod".into());
    AdmissionRequest {
        types: TypeMeta { api_version: "v1".into(), kind: "Pod".into() },
        uid: "12345-12345".into(),
        kind: gvk,
        resource: gvr,
        sub_resource: None,
        request_kind: None,
        request_resource: None,
        request_sub_resource: None,
        name: test_pod.name_any(),
        namespace: Some(test_pod.namespace().unwrap()),
        operation: AdmissionOperation::Create,
        user_info: Default::default(),
        object: Some(test_pod.clone()),
        old_object: None,
        dry_run: false,
        options: None,
    }
}

#[fixture]
fn adm_rev(adm_req: AdmissionRequest<corev1::Pod>) -> AdmissionReview<corev1::Pod> {
    AdmissionReview {
        types: Default::default(),
        request: Some(adm_req),
        response: None,
    }
}

#[fixture]
fn adm_resp(adm_req: AdmissionRequest<corev1::Pod>) -> AdmissionResponse {
    AdmissionResponse::from(&adm_req)
}

#[rstest(tokio::test)]
async fn test_handler_invalid_review(ctx: DriverContext, test_sim: Simulation) {
    let adm_rev = AdmissionReview {
        types: Default::default(),
        request: None,
        response: None,
    };
    let resp = handler(
        rocket::State::from(&ctx),
        rocket::State::from(&test_sim),
        Json(adm_rev),
        rocket::State::from(&MutationData::new()),
    )
    .await;
    assert!(!resp.0.response.unwrap().allowed);
}

#[rstest(tokio::test)]
async fn test_handler_bad_response(
    test_sim: Simulation,
    root_owner_ref: metav1::OwnerReference,
    mut test_pod: corev1::Pod,
    mut adm_rev: AdmissionReview<corev1::Pod>,
) {
    let ctx = ctx(test_pod.clone(), vec![root_owner_ref.clone()], Trace::default());
    test_pod.owner_references_mut().push(root_owner_ref);
    test_pod.spec = None;

    *adm_rev.request.as_mut().unwrap().object.as_mut().unwrap() = test_pod;
    let resp = handler(
        rocket::State::from(&ctx),
        rocket::State::from(&test_sim),
        Json(adm_rev),
        rocket::State::from(&MutationData::new()),
    )
    .await;
    assert!(!resp.0.response.unwrap().allowed);
}

#[rstest(tokio::test)]
async fn test_handler_pod_not_owned_by_sim(
    test_sim: Simulation,
    mut test_pod: corev1::Pod,
    adm_rev: AdmissionReview<corev1::Pod>,
) {
    let owner = metav1::OwnerReference { name: "foo".into(), ..Default::default() };
    let ctx = ctx(test_pod.clone(), vec![owner.clone()], Trace::default());
    test_pod.owner_references_mut().push(owner.clone());
    let resp = handler(
        rocket::State::from(&ctx),
        rocket::State::from(&test_sim),
        Json(adm_rev),
        rocket::State::from(&MutationData::new()),
    )
    .await;
    assert_none!(resp.0.response.unwrap().patch);
}

#[rstest(tokio::test)]
async fn test_reschedule_interrupted_pod_no_action_two_owners(
    mut test_pod: corev1::Pod,
    root_owner_ref: metav1::OwnerReference,
    rs_owner_ref: metav1::OwnerReference,
) {
    let owners = vec![root_owner_ref, rs_owner_ref];
    test_pod.status.get_or_insert_default().phase = Some("Running".into());
    let ctx = ctx(test_pod.clone(), owners.clone(), Trace::default());
    reschedule_interrupted_pod(&ctx, &owners, &test_pod).await.unwrap();
}

#[rstest(tokio::test)]
async fn test_reschedule_interrupted_pod_no_action_not_running(
    mut test_pod: corev1::Pod,
    root_owner_ref: metav1::OwnerReference,
) {
    let owners = vec![root_owner_ref];
    test_pod.status.get_or_insert_default().phase = Some("Succeeded".into());
    let ctx = ctx(test_pod.clone(), owners.clone(), Trace::default());
    reschedule_interrupted_pod(&ctx, &owners, &test_pod).await.unwrap();
}

#[rstest(tokio::test)]
#[case::first_reschedule(None)]
#[case::subsequent_reschedule(Some(42))]
async fn test_reschedule_interrupted_pod(mut test_pod: corev1::Pod, #[case] last_reschedule_count: Option<usize>) {
    let (mut fake_apiserver, client) = make_fake_apiserver();

    test_pod.metadata.owner_references = None;
    let mut expected_pod = test_pod.clone();

    let mut annotations =
        BTreeMap::from([("simkube.io/foo".into(), "bar".into()), ("some.kubernetes.io/thing".into(), "baz".into())]);
    if let Some(i) = last_reschedule_count {
        test_pod.metadata.name = Some(format!("{TEST_POD}-clone-{i}"));
        annotations.insert(ORIG_OWNER_ANNOTATION_KEY.into(), TEST_POD.into());
    }
    test_pod.metadata.uid = Some("asdf1234".into());
    test_pod.metadata.annotations = Some(annotations);
    test_pod.metadata.labels = Some(BTreeMap::from([
        ("simkube.io/kwok-whatever".into(), "1234".into()),
        ("some.kubernetes.io/stuff".into(), "baz".into()),
    ]));
    test_pod.spec.get_or_insert_default().node_name = Some("1-2-3-4.internal".into());
    test_pod.status.get_or_insert_default().phase = Some("Running".into());
    let ctx = ctx_with_client(test_pod.clone(), client, vec![], Trace::default());

    let next_reschedule_index = last_reschedule_count.unwrap_or_default() + 1;
    expected_pod.metadata.name = Some(format!("{TEST_POD}-clone-{next_reschedule_index}"));
    expected_pod.metadata.annotations = Some(BTreeMap::from([
        ("some.kubernetes.io/thing".into(), "baz".into()),
        (ORIG_OWNER_ANNOTATION_KEY.into(), TEST_POD.into()),
    ]));
    expected_pod.metadata.labels = Some(BTreeMap::from([("some.kubernetes.io/stuff".into(), "baz".into())]));
    expected_pod.status = None;

    fake_apiserver.handle(move |when, then| {
        when.path(format!("/api/v1/namespaces/{TEST_NAMESPACE}/pods"))
            .body(serde_json::to_string(&expected_pod).unwrap());
        then.json_body_obj(&expected_pod);
    });

    reschedule_interrupted_pod(&ctx, &[], &test_pod).await.unwrap();
    fake_apiserver.assert();
}

#[rstest]
fn test_add_node_selector_tolerations(test_pod: corev1::Pod) {
    let mut patches = vec![];
    add_node_selector_tolerations(&test_pod, &mut patches).unwrap();
    assert_contains!(
        patches,
        &add_operation(
            format_ptr!("/spec/tolerations/-"),
            json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
        )
    );
    assert_contains!(patches, &add_operation(format_ptr!("/spec/nodeSelector/type"), json!("virtual"),));
}

#[rstest]
fn test_add_node_selector_tolerations_already_exists(mut test_pod: corev1::Pod) {
    test_pod.spec.get_or_insert_default().tolerations = Some(vec![corev1::Toleration {
        key: Some(VIRTUAL_NODE_TOLERATION_KEY.to_string()),
        ..Default::default()
    }]);
    let mut patches = vec![];
    add_node_selector_tolerations(&test_pod, &mut patches).unwrap();
    assert_not_contains!(
        patches,
        &add_operation(
            format_ptr!("/spec/tolerations/-"),
            json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
        )
    );
}

#[rstest]
fn test_add_node_selector_tolerations_other_tols_exist(mut test_pod: corev1::Pod) {
    test_pod.spec.get_or_insert_default().tolerations = Some(vec![corev1::Toleration {
        key: Some("asdf".to_string()),
        ..Default::default()
    }]);
    let mut patches = vec![];
    add_node_selector_tolerations(&test_pod, &mut patches).unwrap();
    assert_contains!(
        patches,
        &add_operation(
            format_ptr!("/spec/tolerations/-"),
            json!({"key": VIRTUAL_NODE_TOLERATION_KEY, "operator": "Exists", "effect": "NoSchedule"}),
        )
    );
}

mod itest {
    use httpmock::{
        HttpMockRequest,
        HttpMockResponse,
    };

    use super::*;

    #[rstest(tokio::test)]
    // don't need the cross-product of these cases
    #[case::running_with_node_selector(true)]
    #[case::not_running_or_no_node_selector(false)]
    async fn test_mutation_handler_create(
        mut test_sim: Simulation,
        mut test_pod: corev1::Pod,
        mut adm_req: AdmissionRequest<corev1::Pod>,
        root_owner_ref: metav1::OwnerReference,
        depl_owner_ref: metav1::OwnerReference,
        #[case] running_and_has_node_selector: bool,
    ) {
        set_snapshot_suffix!("{running_and_has_node_selector}");
        test_sim.spec.speed = Some(2.0);
        test_pod
            .annotations_mut()
            .insert(ORIG_NAMESPACE_ANNOTATION_KEY.into(), TEST_NAMESPACE.into());

        if running_and_has_node_selector {
            test_pod.status.get_or_insert_default().phase = Some("Running".into());
            test_pod.spec.get_or_insert_default().node_selector = Some(BTreeMap::from([("boo".into(), "far".into())]));
        }
        let owner_ns_name = format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}");
        let mut trace = Trace::default();
        if running_and_has_node_selector {
            let pod_spec_hash = 18161541283955474812;
            trace.pod_lifecycles.insert(
                (DEPLOYMENT_GVK.clone(), owner_ns_name.clone()),
                PodLifecyclesMap::from([(pod_spec_hash, vec![PodLifecycleData::Finished(0, 42)])]),
            );
            trace
                .tracked_objects
                .entry(DEPLOYMENT_GVK.clone())
                .or_insert(HashMap::new())
                .insert(owner_ns_name.clone(), ResourceMetadata {});
        }

        let owners = vec![root_owner_ref, depl_owner_ref];
        adm_req.object = Some(test_pod.clone());
        let rev = adm_rev(adm_req);
        let ctx = ctx(test_pod.clone(), owners.clone(), trace);

        let resp = handler(
            rocket::State::from(&ctx),
            rocket::State::from(&test_sim),
            Json(rev),
            rocket::State::from(&MutationData::new_from_parts(
                std::sync::Mutex::new(HashMap::new()),
                MockUtcClock::boxed(0),
            )),
        )
        .await;

        let mut json_pod = serde_json::to_value(&test_pod).unwrap();
        let pod_patch: Patch = serde_json::from_slice(&resp.0.response.unwrap().patch.unwrap()).unwrap();
        for p in pod_patch.0 {
            patch_ext(&mut json_pod, p).unwrap();
        }
        assert_debug_snapshot!(json_pod);
    }

    #[rstest(tokio::test)]
    #[case::no_reschedule(false)]
    #[case::reschedule(true)]
    async fn test_mutation_handler_delete(
        test_sim: Simulation,
        mut test_pod: corev1::Pod,
        mut adm_req: AdmissionRequest<corev1::Pod>,
        root_owner_ref: metav1::OwnerReference,
        #[case] should_reschedule_pod: bool,
    ) {
        // All the snapshots should be the same so we don't create separate ones for each case

        let (mut fake_apiserver, client) = make_fake_apiserver();
        if should_reschedule_pod {
            fake_apiserver.handle(move |when, then| {
                when.path(format!("/api/v1/namespaces/{TEST_NAMESPACE}/pods"));

                // I don't care about exactly constructing the right pod in this test, there's a
                // different test for that; here we'll just echo back whatever we got
                then.respond_with(|req: &HttpMockRequest| {
                    let echoed_body = req.body().to_string();
                    HttpMockResponse::builder().status(200).body(echoed_body).build()
                });
            });
            test_pod.status.get_or_insert_default().phase = Some("Running".into());
        }

        adm_req.object = None;
        adm_req.old_object = Some(test_pod.clone());
        adm_req.operation = AdmissionOperation::Delete;
        let rev = adm_rev(adm_req);

        let owners = vec![root_owner_ref];
        let ctx = ctx_with_client(test_pod.clone(), client, owners.clone(), Trace::default());
        let resp = handler(
            rocket::State::from(&ctx),
            rocket::State::from(&test_sim),
            Json(rev),
            rocket::State::from(&MutationData::new()),
        )
        .await;
        assert_debug_snapshot!(resp);
    }

    #[rstest(tokio::test)]
    #[traced_test]
    async fn test_mutation_handler_delete_client_error(
        test_sim: Simulation,
        mut test_pod: corev1::Pod,
        mut adm_req: AdmissionRequest<corev1::Pod>,
        root_owner_ref: metav1::OwnerReference,
    ) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        fake_apiserver.handle(move |when, then| {
            when.path(format!("/api/v1/namespaces/{TEST_NAMESPACE}/pods"));
            then.status(503);
        });
        test_pod.status.get_or_insert_default().phase = Some("Running".into());

        adm_req.object = None;
        adm_req.old_object = Some(test_pod.clone());
        adm_req.operation = AdmissionOperation::Delete;
        let rev = adm_rev(adm_req);

        let owners = vec![root_owner_ref];
        let ctx = ctx_with_client(test_pod.clone(), client, owners.clone(), Trace::default());
        let resp = handler(
            rocket::State::from(&ctx),
            rocket::State::from(&test_sim),
            Json(rev),
            rocket::State::from(&MutationData::new()),
        )
        .await;
        assert_debug_snapshot!(resp);
        assert!(logs_contain("could not reschedule pod"));
    }
}
