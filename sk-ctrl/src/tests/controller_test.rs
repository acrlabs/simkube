use std::env;

use clockabilly::prelude::*;
use httpmock::prelude::*;
use k8s_openapi::ByteString;
use kube::runtime::controller::Action;
use serde_json::json;
use sk_api::prometheus::*;
use sk_api::v1::SimulationState;
use sk_core::k8s::build_lease;
use tracing_test::traced_test;

use super::*;
use crate::controller::*;
use crate::errors::SkControllerError;
use crate::objects::*;

#[fixture]
fn opts() -> Options {
    Options {
        driver_secrets: None,
        use_cert_manager: false,
        cert_manager_issuer: "".into(),
        verbosity: "info".into(),
    }
}

enum WebhookState {
    NotCreated,
    CaBundleNone,
    CaBundleEmpty,
    ReadyWithCaBundle,
}

#[rstest(tokio::test)]
async fn test_fetch_driver_state_no_driver(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver.handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
    fake_apiserver.handle(move |when, then| {
        when.method(GET)
            .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
        then.json_body_obj(&lease_obj);
    });
    let sim_state = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap()
        .0;
    assert_eq!(SimulationState::Initializing, sim_state);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_fetch_driver_state_driver_no_status(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
        then.json_body(json!({}));
    });
    fake_apiserver.handle(move |when, then| {
        when.method(GET)
            .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
        then.json_body_obj(&lease_obj);
    });
    let sim_state = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap()
        .0;
    assert_eq!(SimulationState::Running, sim_state);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_fetch_driver_state_driver_running(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
        then.json_body(json!({
            "status": {
                "conditions": [{ "type": "Running" }],
            },
        }));
    });
    fake_apiserver.handle(move |when, then| {
        when.method(GET)
            .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
        then.json_body_obj(&lease_obj);
    });
    let sim_state = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap()
        .0;
    assert_eq!(SimulationState::Running, sim_state);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
#[case::complete(JOB_STATUS_CONDITION_COMPLETE)]
#[case::failed(JOB_STATUS_CONDITION_FAILED)]
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
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
        then.json_body(json!({
            "status": {
                "conditions": [{"type": "Running"}, { "type": status }],
            },
        }));
    });
    let sim_state = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap()
        .0;
    assert_eq!(expected_state, sim_state);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_fetch_driver_state_lease_waiting(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    let mut other_sim = test_sim.clone();
    other_sim.metadata.name = Some("blocking-sim".into());
    let other_lease_obj = build_lease(&other_sim, &test_sim_root, TEST_CTRL_NAMESPACE, UtcClock.now());

    let driver_name = ctx.driver_name.clone();
    fake_apiserver.handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
    fake_apiserver.handle(move |when, then| {
        when.method(GET)
            .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"));
        then.json_body_obj(&other_lease_obj);
    });

    let sim_state = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap()
        .0;
    assert_eq!(SimulationState::Blocked, sim_state);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_fetch_driver_state_lease_claim_fails(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let driver_name = ctx.driver_name.clone();
    fake_apiserver.handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
    fake_apiserver.handle_not_found(format!(
        "/apis/coordination.k8s.io/v1/namespaces/{TEST_CTRL_NAMESPACE}/leases/{SK_LEASE_NAME}"
    ));
    fake_apiserver.handle(move |when, then| {
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
    });

    let err = fetch_driver_state(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap_err()
        .downcast::<kube::api::entry::CommitError>()
        .unwrap();
    assert!(matches!(err, kube::api::entry::CommitError::Save(..)));
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_setup_simulation_no_ns(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);
    fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));

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

#[rstest(tokio::test)]
async fn test_setup_simulation_create_prom(test_sim: Simulation, test_sim_root: SimulationRoot, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let prom_name = ctx.prometheus_name.clone();
    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let prom_obj =
        build_prometheus(&ctx.prometheus_name, &test_sim, &test_sim_root, &test_sim.spec.metrics.clone().unwrap());

    fake_apiserver.handle(|when, then| {
        when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
        then.json_body(json!({
            "kind": "Namespace",
        }));
    });
    fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{TEST_NAMESPACE}"));
    fake_apiserver.handle(move |when, then| {
        when.method(POST).path("/api/v1/namespaces");
        then.json_body_obj(&driver_ns_obj);
    });
    fake_apiserver
        .handle_not_found(format!("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses/{prom_name}"));
    fake_apiserver.handle(move |when, then| {
        when.method(POST)
            .path("/apis/monitoring.coreos.com/v1/namespaces/monitoring/prometheuses");
        then.json_body_obj(&prom_obj);
    });
    assert_eq!(
        setup_simulation(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
            .await
            .unwrap(),
        Action::requeue(REQUEUE_DURATION)
    );
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
#[case::ready(true, false, WebhookState::ReadyWithCaBundle)]
#[case::not_ready(false, false, WebhookState::ReadyWithCaBundle)]
#[case::disabled(true, true, WebhookState::ReadyWithCaBundle)]
#[case::webhook_not_created(true, false, WebhookState::NotCreated)]
#[case::webhook_ca_bundle_none(true, false, WebhookState::CaBundleNone)]
#[case::webhook_ca_bundle_empty(true, false, WebhookState::CaBundleEmpty)]
async fn test_setup_simulation_wait_prom(
    mut test_sim: Simulation,
    test_sim_root: SimulationRoot,
    opts: Options,
    #[case] ready_to_create_webhook: bool,
    #[case] disabled: bool,
    #[case] initial_webhook_state: WebhookState,
) {
    // SAFETY: it's fine it's a test
    unsafe { env::set_var("POD_SVC_ACCOUNT", "asdf") };
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let prom_name = ctx.prometheus_name.clone();
    let driver_svc_name = ctx.driver_svc.clone();
    let webhook_name = ctx.webhook_name.clone();
    let driver_name = ctx.driver_name.clone();

    let driver_ns_obj = build_driver_namespace(&ctx, &test_sim);
    let driver_svc_obj = build_driver_service(&ctx, &test_sim, &test_sim_root);
    let webhook_obj = build_mutating_webhook(&ctx, &test_sim, &test_sim_root);
    let driver_obj = build_driver_job(&ctx, &test_sim, None, "".into(), TEST_CTRL_NAMESPACE).unwrap();

    fake_apiserver.handle(|when, then| {
        when.method(GET).path(format!("/api/v1/namespaces/{DEFAULT_METRICS_NS}"));
        then.json_body(json!({
            "kind": "Namespace",
        }));
    });
    fake_apiserver.handle(move |when, then| {
        when.method(GET).path(format!("/api/v1/namespaces/{TEST_NAMESPACE}"));
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
            if ready_to_create_webhook {
                prom_obj.status = Some(PrometheusStatus { available_replicas: 1, ..Default::default() });
            }
            then.json_body_obj(&prom_obj);
        });
    }

    if ready_to_create_webhook {
        fake_apiserver.handle_not_found(format!("/api/v1/namespaces/{TEST_NAMESPACE}/services/{driver_svc_name}"));
        fake_apiserver.handle(move |when, then| {
            when.method(POST).path(format!("/api/v1/namespaces/{TEST_NAMESPACE}/services"));
            then.json_body_obj(&driver_svc_obj);
        });
        fake_apiserver.handle(move |when, then| {
            when.method(GET).path(format!("/api/v1/namespaces/{TEST_NAMESPACE}/secrets"));
            then.json_body(json!({
                "kind": "SecretList",
                "metadata": {},
                "items": [{
                    "kind": "Secret"
                }],
            }));
        });

        match initial_webhook_state {
            WebhookState::NotCreated => {
                fake_apiserver.handle_not_found(format!(
                    "/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{webhook_name}"
                ));
                fake_apiserver.handle(move |when, then| {
                    when.method(POST)
                        .path("/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations");
                    then.json_body_obj(&webhook_obj);
                });
            },
            WebhookState::CaBundleNone => {
                let mut webhook_obj_no_ca = webhook_obj.clone();
                webhook_obj_no_ca.webhooks.as_mut().unwrap()[0].client_config.ca_bundle = None;
                fake_apiserver.handle(move |when, then| {
                    when.method(GET).path(format!(
                        "/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{webhook_name}"
                    ));
                    then.json_body_obj(&webhook_obj_no_ca);
                });
            },
            WebhookState::CaBundleEmpty => {
                let mut webhook_obj_empty_ca = webhook_obj.clone();
                webhook_obj_empty_ca.webhooks.as_mut().unwrap()[0].client_config.ca_bundle = Some(ByteString(vec![]));
                fake_apiserver.handle(move |when, then| {
                    when.method(GET).path(format!(
                        "/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{webhook_name}"
                    ));
                    then.json_body_obj(&webhook_obj_empty_ca);
                });
            },
            WebhookState::ReadyWithCaBundle => {
                let mut webhook_obj_with_ca = webhook_obj.clone();
                webhook_obj_with_ca.webhooks.as_mut().unwrap()[0].client_config.ca_bundle =
                    Some(ByteString(b"test-ca-bundle".to_vec()));
                fake_apiserver.handle(move |when, then| {
                    when.method(GET).path(format!(
                        "/apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{webhook_name}"
                    ));
                    then.json_body_obj(&webhook_obj_with_ca);
                });
            },
        }

        if matches!(initial_webhook_state, WebhookState::ReadyWithCaBundle) {
            fake_apiserver.handle_not_found(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/{driver_name}"));
            fake_apiserver.handle(move |when, then| {
                when.method(POST)
                    .path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs"));
                then.json_body_obj(&driver_obj);
            });
        }
    }
    let res = setup_simulation(&ctx, &test_sim, &test_sim_root, TEST_CTRL_NAMESPACE)
        .await
        .unwrap();
    if !ready_to_create_webhook || !matches!(initial_webhook_state, WebhookState::ReadyWithCaBundle) {
        assert_eq!(res, Action::requeue(REQUEUE_DURATION));
    } else {
        assert_eq!(res, Action::await_change());
    };

    fake_apiserver.assert();
}


#[rstest(tokio::test)]
#[traced_test]
async fn test_cleanup_simulation(test_sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&test_sim);

    let root = ctx.metaroot_name.clone();

    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/simkube.io/v1/simulationroots/{root}"));
        then.json_body(status_ok());
    });
    cleanup_simulation(&ctx, &test_sim).await;

    assert!(!logs_contain("ERROR"));
    fake_apiserver.assert();
}
