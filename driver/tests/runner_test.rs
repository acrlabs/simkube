use std::collections::{
    HashMap,
    VecDeque,
};
use std::fs::File;
use std::io::BufReader;

use httpmock::Method::PATCH;
use simkube::store::{
    PodLifecyclesMap,
    TraceEvent,
};

use super::runner::build_virtual_ns;
use super::*;

const TEST_VIRT_NS_PREFIX: &str = "virt-test";

fn build_trace_data(has_start_marker: bool) -> Vec<u8> {
    // I want the trace data to be easily editable, so we load it from a plain-text JSON file and
    // then re-encode it into msgpack so we can pass the data to import
    let trace_data_file = File::open("./driver/tests/data/trace.json").unwrap();
    let reader = BufReader::new(trace_data_file);
    let (config, mut events, index, lifecycle_data): (
        TracerConfig,
        VecDeque<TraceEvent>,
        HashMap<String, u64>,
        HashMap<String, PodLifecyclesMap>,
    ) = serde_json::from_reader(reader).unwrap();

    if has_start_marker {
        events.push_front(TraceEvent {
            ts: 1709241485,
            applied_objs: vec![],
            deleted_objs: vec![],
        });
    }

    rmp_serde::to_vec_named(&(&config, &events, &index, &lifecycle_data)).unwrap()
}

#[rstest]
#[case::has_start_marker(true)]
#[case::no_start_marker(false)]
#[traced_test]
#[tokio::test]
async fn itest_run(#[case] has_start_marker: bool) {
    let trace_data = build_trace_data(has_start_marker);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let apiset = ApiSet::new(client.clone());
    let owners_cache = Arc::new(Mutex::new(OwnersCache::new(apiset)));
    let store = Arc::new(TraceStore::import(trace_data, &None).unwrap());
    let ctx = DriverContext {
        name: TEST_SIM_NAME.into(),
        sim_root: TEST_SIM_ROOT_NAME.into(),
        virtual_ns_prefix: TEST_VIRT_NS_PREFIX.into(),
        owners_cache,
        store,
    };

    let root = SimulationRoot {
        metadata: metav1::ObjectMeta {
            name: Some(TEST_SIM_ROOT_NAME.into()),
            uid: Some("puwern5t".into()),
            ..Default::default()
        },
        spec: SimulationRootSpec {},
    };
    let virt_ns = build_virtual_ns(&ctx, &root.clone(), "default").unwrap();

    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_SIM_ROOT_NAME}"));
            then.json_body_obj(&SimulationRoot {
                metadata: metav1::ObjectMeta {
                    name: Some(TEST_SIM_ROOT_NAME.into()),
                    uid: Some("puwern5t".into()),
                    ..Default::default()
                },
                spec: SimulationRootSpec {},
            });
        })
        .handle_not_found(format!("/api/v1/namespaces/{TEST_VIRT_NS_PREFIX}-default"))
        .handle(move |when, then| {
            when.path("/api/v1/namespaces".to_string());
            then.json_body_obj(&virt_ns);
        })
        .handle(|when, then| {
            when.path("/apis/apps/v1".to_string());
            then.json_body(apps_v1_discovery());
        })
        .handle(|when, then| {
            when.method(PATCH)
                .path(format!("/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-default/deployments/nginx-deployment"));
            then.json_body(status_ok());
        })
        .build();
    let runner = TraceRunner::new(client, ctx).await.unwrap();
    runner.run().await.unwrap();
    fake_apiserver.assert();
}
