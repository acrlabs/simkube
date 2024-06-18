use std::env;

use either::for_both;
use httpmock::prelude::*;
use kube::runtime::controller::Action;
use serde_json::json;
use simkube::k8s::build_lease;
use simkube::metrics::api::*;

use super::controller::*;
use super::*;
use crate::objects::*;

#[fixture]
fn opts() -> Options {
    Options {
        use_cert_manager: false,
        cert_manager_issuer: "".into(),
        verbosity: "info".into(),
    }
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_state_no_driver(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"))
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let sim_state =
        for_both!(fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE).await.unwrap(), s => s.0);
    assert_eq!(SimulationState::Initializing, sim_state);
    fake_apiserver.assert();
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_state_driver_no_status(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
            then.json_body(json!({}));
        })
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let sim_state =
        for_both!(fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE).await.unwrap(), s => s.0);
    assert_eq!(SimulationState::Running, sim_state);
    fake_apiserver.assert();
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_state_driver_running(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
            then.json_body(json!({
                "status": {
                    "conditions": [{ "type": "Running" }],
                },
            }));
        })
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let sim_state =
        for_both!(fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE).await.unwrap(), s => s.0);
    assert_eq!(SimulationState::Running, sim_state);
    fake_apiserver.assert();
}

#[rstest]
#[case::complete(JOB_STATUS_CONDITION_COMPLETE)]
#[case::failed(JOB_STATUS_CONDITION_FAILED)]
#[tokio::test]
async fn test_fetch_driver_state_driver_finished(
    test_sim: Simulation,
    test_sim_root: SimulationRoot,
    opts: Options,
    #[case] status: &'static str,
) {
    let expected_state = if status == JOB_STATUS_CONDITION_COMPLETE {
        SimulationState::Finished
    } else {
        SimulationState::Failed
    };

    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
    // No lease handler because we don't claim the lease if we're done
    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
            then.json_body(json!({
                "status": {
                    "conditions": [{"type": "Running"}, { "type": status }],
                },
            }));
        })
        .build();
    let sim_state =
        for_both!(fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE).await.unwrap(), s => s.0);
    assert_eq!(expected_state, sim_state);
    fake_apiserver.assert();
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_fetch_driver_state_lease_waiting(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let mut other_sim = test_sim.clone();
    other_sim.metadata.name = Some("blocking-sim".into());
    let other_lease_obj = build_lease(&other_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"))
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&other_lease_obj);
        })
        .build();

    let sim_state =
        for_both!(fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE).await.unwrap(), s => s.0);
    assert_eq!(SimulationState::Blocked, sim_state);
    fake_apiserver.assert();
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_fetch_driver_state_lease_claim_fails(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"))
        .handle_not_found(format!(
            "/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"
        ))
        .handle(move |when, then| {
            when.method(POST)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases"));
            then.status(409).json_body(json!({
              "kind": "Status",
              "apiVersion": "v1",
              "metadata": {},
              "message": "the object has been modified; please apply your changes to the latest version and try again",
              "status": "Failure",
              "reason": "Conflict",
              "code": 409
            }));
        })
        .build();

    let err = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap_err()
        .downcast::<kube::api::entry::CommitError>()
        .unwrap();
    assert!(matches!(err, kube::api::entry::CommitError::Save(..)));
    fake_apiserver.assert();
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_setup_simulation_no_ns(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    fake_apiserver
        .handle_not_found(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"))
        .build();

    assert!(matches!(
        setup_simulation(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
            .await
            .unwrap_err()
            .downcast::<SkControllerError>()
            .unwrap(),
        SkControllerError::NamespaceNotFound(_)
    ));
    fake_apiserver.assert();
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_setup_simulation_create_prom(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_ns = test_sim.spec.driver.namespace.clone();
    let prom_name = ctx.prometheus_name.clone();
    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let prom_obj =
        build_prometheus(&ctx.prometheus_name, &test_sim, &test_sim_root, &test_sim.spec.metrics.clone().unwrap());

    fake_apiserver
        .handle(|when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
            then.json_body(json!({
                "kind": "Namespace",
            }));
        })
        .handle_not_found(format!("/api/v1/namespaces/{driver_ns}"))
        .handle(move |when, then| {
            when.method(POST).path("/api/v1/namespaces");
            then.json_body_obj(&driver_ns_obj);
        })
        .handle_not_found(format!("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses/{prom_name}"))
        .handle(move |when, then| {
            when.method(POST)
                .path("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses");
            then.json_body_obj(&prom_obj);
        })
        .build();
    assert_eq!(
        setup_simulation(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
            .await
            .unwrap(),
        Action::requeue(REQUEUE_DURATION)
    );
    fake_apiserver.assert();
}

#[rstest]
#[case::ready(true, false)]
#[case::not_ready(false, false)]
#[case::disabled(true, true)]
#[traced_test]
#[tokio::test]
async fn test_setup_simulation_wait_prom(
    mut test_sim: Simulation,
    test_sim_root: SimulationRoot,
    opts: Options,
    #[case] ready: bool,
    #[case] disabled: bool,
) {
    env::set_var("POD_SVC_ACCOUNT", "asdf");
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_ns = test_sim.spec.driver.namespace.clone();
    let prom_name = ctx.prometheus_name.clone();
    let driver_svc_name = ctx.driver_svc.clone();
    let webhook_name = ctx.webhook_name.clone();
    let driver_name = ctx.driver_name.clone();

    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let driver_svc_obj = build_driver_service(&ctx, &test_sim, &test_sim_root);
    let webhook_obj = build_mutating_webhook(&ctx, &test_sim, &test_sim_root);
    let driver_obj = build_driver_job(&ctx, &test_sim, "".into(), TEST_CTRL_NAMESPACE).unwrap();

    fake_apiserver
        .handle(|when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
            then.json_body(json!({
                "kind": "Namespace",
            }));
        })
        .handle(move |when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{driver_ns}"));
            then.json_body_obj(&driver_ns_obj);
        });

    if disabled {
        test_sim.spec.metrics = None;
    } else {
        let prom_obj =
            build_prometheus(&ctx.prometheus_name, &test_sim, &test_sim_root, &test_sim.spec.metrics.clone().unwrap());
        fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses/{prom_name}"));
            let mut prom_obj = prom_obj.clone();
            if ready {
                prom_obj.status = Some(PrometheusStatus { available_replicas: 1, ..Default::default() });
            }
            then.json_body_obj(&prom_obj);
        });
    }

    if ready {
        fake_apiserver
            .handle_not_found(format!("/api/v1/namespaces/test/services/{driver_svc_name}"))
            .handle(move |when, then| {
                when.method(POST).path("/api/v1/namespaces/test/services");
                then.json_body_obj(&driver_svc_obj);
            })
            .handle(move |when, then| {
                when.method(GET).path("/api/v1/namespaces/test/secrets");
                then.json_body(json!({
                    "kind": "SecretList",
                    "metadata": {},
                    "items": [{
                        "kind": "Secret"
                    }],
                }));
            })
            .handle_not_found(format!(
                "/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{webhook_name}",
            ))
            .handle(move |when, then| {
                when.method(POST)
                    .path("/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations");
                then.json_body_obj(&webhook_obj);
            })
            .handle_not_found(format!("/apis/batch/v1/namespaces/test/jobs/{driver_name}"))
            .handle(move |when, then| {
                when.method(POST).path("/apis/batch/v1/namespaces/test/jobs");
                then.json_body_obj(&driver_obj);
            });
    }
    fake_apiserver.build();
    let res = setup_simulation(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap();
    if ready {
        assert_eq!(res, Action::await_change());
    } else {
        assert_eq!(res, Action::requeue(REQUEUE_DURATION));
    }
    fake_apiserver.assert();
}


#[rstest]
#[traced_test]
#[tokio::test]
async fn test_cleanup_simulation(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let root = ctx.metaroot_name.clone();

    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{root}"));
            then.json_body(status_ok());
        })
        .build();
    cleanup_simulation(&ctx, &test_sim).await;

    assert!(!logs_contain("ERROR"));
    fake_apiserver.assert();
}
