use httpmock::Method::*;
use simkube::k8s::build_lease;

use super::helpers::{
    build_driver_context,
    build_trace_data,
};
use super::runner::build_virtual_ns;
use super::*;

const TEST_NS_NAME: &str = "default";

#[rstest]
#[case::has_start_marker(true)]
#[case::no_start_marker(false)]
#[traced_test]
#[tokio::test]
async fn itest_run(#[case] has_start_marker: bool) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let cache = Arc::new(Mutex::new(OwnersCache::new(ApiSet::new(client.clone()))));

    let trace_data = build_trace_data(has_start_marker);
    let store = Arc::new(TraceStore::import(trace_data, &None).unwrap());
    let ctx = build_driver_context(cache, store);

    let root = SimulationRoot {
        metadata: metav1::ObjectMeta {
            name: Some(TEST_DRIVER_ROOT_NAME.into()),
            uid: Some("puwern5t".into()),
            ..Default::default()
        },
        spec: SimulationRootSpec {},
    };
    let virt_ns = build_virtual_ns(&ctx, &root.clone(), TEST_NS_NAME);
    let lease_obj = build_lease(&ctx.sim, &root, TEST_CTRL_NAMESPACE, UtcClock.now());
    let patched_lease_obj = build_lease(&ctx.sim, &root, TEST_CTRL_NAMESPACE, UtcClock.now());

    fake_apiserver
        .handle(move |when, then| {
            // In theory the driver needs to create the driver root first, but here we return
            // that it's already been created so we can build the right virtual_ns object above
            when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
                .method(GET);
            then.json_body_obj(&root);
        })
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .handle(move |when, then| {
            when.method(PATCH)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&patched_lease_obj);
        })
        .handle_not_found(format!("/api/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}"))
        .handle(move |when, then| {
            when.path("/api/v1/namespaces".to_string());
            then.json_body_obj(&virt_ns);
        })
        .handle(|when, then| {
            when.path("/apis/apps/v1".to_string());
            then.json_body(apps_v1_discovery());
        })
        .handle(|when, then| {
            when.method(PATCH).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment"
            ));
            then.json_body(status_ok());
        })
        .handle(|when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
                .method(DELETE);
            then.json_body(status_ok());
        })
        .build();
    run_trace(ctx, client).await.unwrap();
    fake_apiserver.assert();
}
