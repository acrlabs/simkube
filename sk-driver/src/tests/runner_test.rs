use clockabilly::mock::MockUtcClock;
use clockabilly::prelude::*;
use clockabilly::DateTime;
use httpmock::Method::*;
use serde_json::json;
use sk_api::v1::SimulationRootSpec;
use sk_core::k8s::build_lease;

use super::helpers::{
    build_driver_context,
    build_trace_data,
    TRACE_START,
};
use super::*;
use crate::runner::{
    build_virtual_ns,
    build_virtual_obj,
    cleanup_trace,
};
use crate::util::DRIVER_PAUSED_WAIT_SECONDS;

// Must match the namespace in tests/data/trace.json
const TEST_NS_NAME: &str = "default";

#[rstest(tokio::test)]
async fn test_build_virtual_object_multiple_pod_specs(test_sim_root: SimulationRoot, test_two_pods_obj: DynamicObject) {
    let (_, client) = make_fake_apiserver();
    let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

    let store = Arc::new(TraceStore::new(Default::default()));
    let ctx = build_driver_context(cache, store);

    let pod_spec_template_paths = Some(vec!["/spec/template1".into(), "/spec/template2".into()]);

    let virtual_ns = format!("virtual-{TEST_NAMESPACE}");
    let vobj = build_virtual_obj(
        &ctx,
        &test_sim_root,
        TEST_NAMESPACE,
        &virtual_ns,
        &test_two_pods_obj,
        pod_spec_template_paths.as_deref(),
    )
    .unwrap();

    assert_eq!(vobj.metadata.namespace.unwrap(), virtual_ns);
    assert_eq!(
        vobj.data,
        json!({
            "spec": {
                "template1": {
                    "metadata": {
                        "annotations": {
                            ORIG_NAMESPACE_ANNOTATION_KEY: TEST_NAMESPACE,
                        },
                    },
                    "spec": {"containers": [{}]},
                },
                "template2": {
                    "metadata": {
                        "annotations": {
                            ORIG_NAMESPACE_ANNOTATION_KEY: TEST_NAMESPACE,
                        },
                    },
                    "spec": {"containers": [{}]},
                },
            }
        })
    );
}

#[rstest(tokio::test)]
async fn test_cleanup_trace_error() {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
    let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

    let store = Arc::new(TraceStore::new(Default::default()));
    let ctx = build_driver_context(cache, store);

    let clock = MockUtcClock::boxed(0);

    fake_apiserver.handle(|when, then| {
        when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
            .method(DELETE);
        then.status(500);
    });

    let res = cleanup_trace(&ctx, roots_api, clock, DRIVER_CLEANUP_TIMEOUT_SECONDS)
        .await
        .unwrap_err()
        .downcast::<SkDriverError>()
        .unwrap();
    assert!(matches!(res, SkDriverError::CleanupFailed(..)));
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_cleanup_trace_timeout() {
    let (fake_apiserver, client) = make_fake_apiserver();
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
    let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

    let store = Arc::new(TraceStore::new(Default::default()));
    let ctx = build_driver_context(cache, store);

    let clock = MockUtcClock::boxed(DRIVER_CLEANUP_TIMEOUT_SECONDS + 10);

    let res = cleanup_trace(&ctx, roots_api, clock, DRIVER_CLEANUP_TIMEOUT_SECONDS)
        .await
        .unwrap_err()
        .downcast::<SkDriverError>()
        .unwrap();
    assert!(matches!(res, SkDriverError::CleanupTimeout(..)));
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_cleanup_trace() {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(client.clone());
    let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

    let store = Arc::new(TraceStore::new(Default::default()));
    let ctx = build_driver_context(cache, store);

    let clock = MockUtcClock::boxed(0);

    fake_apiserver.handle(|when, then| {
        when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
            .method(DELETE);
        then.json_body(status_ok());
    });
    cleanup_trace(&ctx, roots_api, clock, DRIVER_CLEANUP_TIMEOUT_SECONDS)
        .await
        .unwrap();
    fake_apiserver.assert();
}

mod itest {
    use super::*;

    #[rstest(tokio::test)]
    #[case::has_start_marker(true)]
    #[case::no_start_marker(false)]
    async fn test_driver_run(test_sim: Simulation, #[case] has_start_marker: bool) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

        let trace_data = build_trace_data(has_start_marker, None);
        let store = Arc::new(TraceStore::import(trace_data, None).unwrap());
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
        let lease_obj = build_lease(&test_sim, &root, TEST_CTRL_NAMESPACE, UtcClock.now());
        let patched_lease_obj = build_lease(&test_sim, &root, TEST_CTRL_NAMESPACE, UtcClock.now());
        let test_sim_clone = test_sim.clone();

        fake_apiserver.handle(move |when, then| {
            // In theory the driver needs to create the driver root first, but here we return
            // that it's already been created so we can build the right virtual_ns object above
            when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
                .method(GET);
            then.json_body_obj(&root);
        });
        fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        });
        fake_apiserver.handle(move |when, then| {
            when.method(PATCH)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&patched_lease_obj);
        });
        fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}"));
        fake_apiserver.handle(move |when, then| {
            when.path("/api/v1/namespaces".to_string());
            then.json_body_obj(&virt_ns);
        });
        fake_apiserver.handle(|when, then| {
            when.path("/apis/apps/v1".to_string());
            then.json_body(apps_v1_discovery());
        });
        fake_apiserver.handle(|when, then| {
            when.method(PATCH).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment"
            ));
            then.json_body(status_ok());
        });
        fake_apiserver.handle_multiple(
            move |when, then| {
                when.method(GET)
                    .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
                then.json_body_obj(&test_sim_clone);
            },
            if has_start_marker { 2 } else { 1 },
        );
        fake_apiserver.handle(|when, then| {
            when.method(DELETE).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment-2"
            ));
            then.json_body(status_ok());
        });
        fake_apiserver.handle(|when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{TEST_DRIVER_ROOT_NAME}"))
                .method(DELETE);
            then.json_body(status_ok());
        });
        run_trace(ctx, client, test_sim).await.unwrap();
        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    #[case::not_paused(false, 10)]
    #[case::paused(true, 20)]
    async fn test_driver_run_internal_paused(
        mut test_sim: Simulation,
        #[case] paused: bool,
        #[case] expected_end_ts: i64,
    ) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

        let trace_data = build_trace_data(false, Some(10));
        let store = Arc::new(TraceStore::import(trace_data, None).unwrap());
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

        let test_sim_clone = test_sim.clone();
        let mut clock = MockUtcClock::boxed(0);
        if paused {
            test_sim.spec.paused_time = Some(clock.now());
        }

        fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}"));
        fake_apiserver.handle(move |when, then| {
            when.path("/api/v1/namespaces".to_string());
            then.json_body_obj(&virt_ns);
        });
        fake_apiserver.handle(|when, then| {
            when.path("/apis/apps/v1".to_string());
            then.json_body(apps_v1_discovery());
        });
        fake_apiserver.handle(|when, then| {
            when.method(PATCH).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment"
            ));
            then.json_body(status_ok());
        });
        fake_apiserver.handle(|when, then| {
            when.method(DELETE).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment-2"
            ));
            then.json_body(status_ok());
        });
        let sim_handle_id = fake_apiserver.handle_multiple(
            move |when, then| {
                when.method(GET)
                    .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
                then.json_body_obj(&test_sim);
            },
            2,
        );

        if paused {
            let mut fake_apiserver_clone = fake_apiserver.clone();
            clock.add_callback(DRIVER_PAUSED_WAIT_SECONDS + 1, move || {
                fake_apiserver_clone.drop(sim_handle_id);
                let test_sim_clone = test_sim_clone.clone(); // Don't understand why this clone is needed
                fake_apiserver_clone.handle_multiple(
                    move |when, then| {
                        when.method(GET)
                            .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
                        then.json_body_obj(&test_sim_clone);
                    },
                    2,
                );
            });
        }

        run_trace_internal(&ctx, client, 1.0, root, TRACE_START, clock.clone())
            .await
            .unwrap();
        fake_apiserver.assert();
        assert_eq!(expected_end_ts, clock.now_ts());
    }

    #[rstest(tokio::test)]
    async fn test_driver_run_internal_paused_in_middle(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let cache = Arc::new(Mutex::new(OwnersCache::new(DynamicApiSet::new(client.clone()))));

        let trace_data = build_trace_data(false, Some(10));
        let store = Arc::new(TraceStore::import(trace_data, None).unwrap());
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

        let test_sim_clone_1 = test_sim.clone();
        let test_sim_clone_2 = test_sim.clone();
        let mut clock = MockUtcClock::boxed(0);

        fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}"));
        fake_apiserver.handle(move |when, then| {
            when.path("/api/v1/namespaces".to_string());
            then.json_body_obj(&virt_ns);
        });
        fake_apiserver.handle(|when, then| {
            when.path("/apis/apps/v1".to_string());
            then.json_body(apps_v1_discovery());
        });
        fake_apiserver.handle(|when, then| {
            when.method(PATCH).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment"
            ));
            then.json_body(status_ok());
        });
        fake_apiserver.handle(|when, then| {
            when.method(DELETE).path(format!(
                "/apis/apps/v1/namespaces/{TEST_VIRT_NS_PREFIX}-{TEST_NS_NAME}/deployments/nginx-deployment-2"
            ));
            then.json_body(status_ok());
        });
        let sim_handle_id = fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim);
        });

        let mut fake_apiserver_clone_1 = fake_apiserver.clone();
        let paused_ts = 3;
        clock.add_callback(paused_ts, move || {
            fake_apiserver_clone_1.drop(sim_handle_id);
            let mut test_sim_clone = test_sim_clone_1.clone(); // Don't understand why this clone is needed
            test_sim_clone.spec.paused_time = DateTime::from_timestamp(paused_ts, 0);
            fake_apiserver_clone_1.handle_multiple(
                move |when, then| {
                    when.method(GET)
                        .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
                    then.json_body_obj(&test_sim_clone);
                },
                2,
            );
        });

        // The simulation was "supposed" to process its next event at time 10, so that's when
        // it wakes up; at that point, it realizes that the simulation was paused at time 3,
        // so it then sleeps until time 20.  Then it wakes up, realizes it's still paused, so
        // sleeps until time 30.  Then it wakes up, it is unpaused, sleeps for another 7 seconds,
        // and completes the simulation at time 37.
        let mut fake_apiserver_clone_2 = fake_apiserver.clone();
        clock.add_callback(paused_ts + 2 * DRIVER_PAUSED_WAIT_SECONDS, move || {
            fake_apiserver_clone_2.drop(sim_handle_id + 1);
            let test_sim_clone = test_sim_clone_2.clone(); // Don't understand why this clone is needed
            fake_apiserver_clone_2.handle(move |when, then| {
                when.method(GET)
                    .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
                then.json_body_obj(&test_sim_clone);
            });
        });

        run_trace_internal(&ctx, client, 1.0, root, TRACE_START, clock.clone())
            .await
            .unwrap();
        fake_apiserver.assert();
        assert_eq!(37, clock.now_ts());
    }
}
