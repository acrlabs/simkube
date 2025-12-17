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
    Operation,
};
use kube::core::{
    GroupVersionKind,
    GroupVersionResource,
};
use rocket::serde::json::Json;
use sk_core::k8s::PodLifecycleData;
use sk_store::{
    ExportedTrace,
    PodLifecyclesMap,
};

use super::helpers::build_driver_context;
use super::*;

#[fixture]
fn ctx(
    test_pod: corev1::Pod,
    #[default(vec![])] pod_owners: Vec<metav1::OwnerReference>,
    #[default(ExportedTrace::default())] trace: ExportedTrace,
) -> DriverContext {
    let (_, client) = make_fake_apiserver();
    let mut owners = HashMap::new();
    owners.insert((corev1::Pod::gvk(), test_pod.namespaced_name()), pod_owners);
    let cache = OwnersCache::new_from_parts(DynamicApiSet::new(client), owners);
    build_driver_context(cache, trace)
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
        operation: Operation::Create,
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
    mut test_pod: corev1::Pod,
    mut adm_rev: AdmissionReview<corev1::Pod>,
) {
    let owner = metav1::OwnerReference {
        name: TEST_DRIVER_ROOT_NAME.into(),
        ..Default::default()
    };
    let ctx = ctx(test_pod.clone(), vec![owner.clone()], ExportedTrace::default());
    test_pod.owner_references_mut().push(owner);
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
async fn test_mutate_pod_not_owned_by_sim(
    test_sim: Simulation,
    mut test_pod: corev1::Pod,
    mut adm_resp: AdmissionResponse,
) {
    let owner = metav1::OwnerReference { name: "foo".into(), ..Default::default() };
    let ctx = ctx(test_pod.clone(), vec![owner.clone()], ExportedTrace::default());
    test_pod.owner_references_mut().push(owner);
    adm_resp = mutate_pod(&ctx, &test_sim, adm_resp, &test_pod, &MutationData::new(), MockUtcClock::boxed(0))
        .await
        .unwrap();
    assert_eq!(adm_resp.patch, None);
}

mod itest {
    use super::*;

    #[rstest(tokio::test)]
    // don't need the cross-product of these cases
    #[case(true)]
    #[case(false)]
    async fn test_mutate_pod(
        mut test_sim: Simulation,
        mut test_pod: corev1::Pod,
        mut adm_resp: AdmissionResponse,
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
        let root = metav1::OwnerReference {
            name: TEST_DRIVER_ROOT_NAME.into(),
            ..Default::default()
        };
        let depl = metav1::OwnerReference {
            name: TEST_DEPLOYMENT.into(),
            api_version: "apps/v1".into(),
            kind: "Deployment".into(),
            ..Default::default()
        };

        let owner_ns_name = format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}");
        let mut trace = ExportedTrace::default();
        if running_and_has_node_selector {
            let pod_spec_hash = 18161541283955474812;
            trace.pod_lifecycles.insert(
                (DEPL_GVK.clone(), owner_ns_name.clone()),
                PodLifecyclesMap::from([(pod_spec_hash, vec![PodLifecycleData::Finished(0, 42)])]),
            );
            trace.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), 1234);
        }

        let ctx = ctx(test_pod.clone(), vec![root.clone(), depl.clone()], trace);

        adm_resp = mutate_pod(&ctx, &test_sim, adm_resp, &test_pod, &MutationData::new(), MockUtcClock::boxed(0))
            .await
            .unwrap();
        let mut json_pod = serde_json::to_value(&test_pod).unwrap();
        let pod_patch: Patch = serde_json::from_slice(&adm_resp.patch.unwrap()).unwrap();
        for p in pod_patch.0 {
            patch_ext(&mut json_pod, p).unwrap();
        }
        assert_debug_snapshot!(json_pod);
    }
}
