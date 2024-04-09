use std::env;

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
        driver_image: "driver:latest".into(),
        driver_port: 1234,
        use_cert_manager: false,
        cert_manager_issuer: "".into(),
        verbosity: "info".into(),
    }
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_no_driver(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"))
        .build();
    assert_eq!(SimulationState::Initializing, fetch_driver_status(&ctx).await.unwrap().0);
    fake_apiserver.assert();
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_driver_no_status(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
            then.json_body(json!({}));
        })
        .build();
    assert_eq!(SimulationState::Running, fetch_driver_status(&ctx).await.unwrap().0);
    fake_apiserver.assert();
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_driver_running(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

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
        .build();
    assert_eq!(SimulationState::Running, fetch_driver_status(&ctx).await.unwrap().0);
    fake_apiserver.assert();
}

#[rstest]
#[case::complete(JOB_STATUS_CONDITION_COMPLETE)]
#[case::failed(JOB_STATUS_CONDITION_FAILED)]
#[tokio::test]
async fn test_fetch_driver_status_driver_finished(test_sim: Simulation, opts: Options, #[case] status: &'static str) {
    let expected_state = if status == JOB_STATUS_CONDITION_COMPLETE {
        SimulationState::Finished
    } else {
        SimulationState::Failed
    };

    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
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
    assert_eq!(expected_state, fetch_driver_status(&ctx).await.unwrap().0);
    fake_apiserver.assert();
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_setup_driver_no_ns(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    fake_apiserver
        .handle_not_found(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"))
        .build();

    assert!(matches!(
        setup_driver(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
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
async fn test_setup_driver_lease_claim_fails(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    fake_apiserver
        .handle(|when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
            then.json_body(json!({
                "kind": "Namespace",
            }));
        })
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

    let err = setup_driver(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
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
async fn test_setup_driver_create_prom(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());
    let driver_ns = ctx.driver_ns.clone();
    let prom_name = ctx.prometheus_name.clone();
    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let prom_obj = build_prometheus(&ctx.prometheus_name, &test_sim, &test_sim.spec.metrics_config.clone().unwrap());

    fake_apiserver
        .handle(|when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
            then.json_body(json!({
                "kind": "Namespace",
            }));
        })
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
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
        setup_driver(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
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
async fn test_setup_driver_wait_prom(
    mut test_sim: Simulation,
    test_sim_root: SimulationRoot,
    opts: Options,
    #[case] ready: bool,
    #[case] disabled: bool,
) {
    env::set_var("POD_SVC_ACCOUNT", "asdf");
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_ns = ctx.driver_ns.clone();
    let prom_name = ctx.prometheus_name.clone();
    let driver_svc_name = ctx.driver_svc.clone();
    let webhook_name = ctx.webhook_name.clone();
    let driver_name = ctx.driver_name.clone();

    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());
    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let driver_svc_obj = build_driver_service(&ctx, &test_sim_root);
    let webhook_obj = build_mutating_webhook(&ctx, &test_sim_root);
    let driver_obj = build_driver_job(&ctx, &test_sim, "".into(), TEST_CTRL_NAMESPACE).unwrap();

    fake_apiserver
        .handle(|when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
            then.json_body(json!({
                "kind": "Namespace",
            }));
        })
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .handle(move |when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{driver_ns}"));
            then.json_body_obj(&driver_ns_obj);
        });

    if disabled {
        test_sim.spec.metrics_config = None;
    } else {
        let prom_obj =
            build_prometheus(&ctx.prometheus_name, &test_sim, &test_sim.spec.metrics_config.clone().unwrap());
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
    let res = setup_driver(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
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
async fn test_cleanup(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let root = ctx.metaroot_name.clone();
    let prom = ctx.prometheus_name.clone();

    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{root}"));
            then.json_body(status_ok());
        })
        .handle(move |when, then| {
            when.path(format!("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses/{prom}"));
            then.json_body(status_ok());
        })
        .build();
    cleanup(&ctx, &test_sim).await;

    assert!(!logs_contain("ERROR"));
    fake_apiserver.assert();
}

// Copy-pasta-ing this because I can't get rstest cases and traced_test to play nicely with each
// other :facepalm: :eyeroll:
#[rstest]
#[traced_test]
#[tokio::test]
async fn test_cleanup_not_found(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let root = ctx.metaroot_name.clone();
    let prom = ctx.prometheus_name.clone();

    fake_apiserver
        .handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulationroots/{root}"));
            then.json_body(status_ok());
        })
        .handle_not_found(format!("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses/{prom}"))
        .build();
    cleanup(&ctx, &test_sim).await;

    assert!(logs_contain("WARN"));
    fake_apiserver.assert();
}
