use serde_json::json;
use simkube::testutils::fake::make_fake_apiserver;
use simkube::testutils::*;

use super::controller::*;
use super::*;


#[fixture]
fn sim() -> Simulation {
    Simulation::new(
        "testing",
        SimulationSpec {
            driver_namespace: "test".into(),
            ..Default::default()
        },
    )
}

#[fixture]
fn opts() -> Options {
    Options {
        driver_image: "driver:latest".into(),
        driver_port: 1234,
        use_cert_manager: true,
        cert_manager_issuer: "nobody".into(),
        verbosity: "info".into(),
    }
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_no_driver(sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/sk-testing-driver"));
        then.status(404).json_body(json!({
            "status": "Failure",
            "reason": "NotFound",
            "code": 404,
        }));
    });
    fake_apiserver.build();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&sim);
    assert_eq!(DriverStatus::Waiting, fetch_driver_status(&ctx).await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_driver_no_status(sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/sk-testing-driver"));
        then.json_body(json!({}));
    });
    fake_apiserver.build();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&sim);
    assert_eq!(DriverStatus::Running, fetch_driver_status(&ctx).await.unwrap());
}

#[rstest]
#[tokio::test]
async fn test_fetch_driver_status_driver_running(sim: Simulation, opts: Options) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/sk-testing-driver"));
        then.json_body(json!({
            "status": {
                "conditions": [{ "type": "Running" }],
            },
        }));
    });
    fake_apiserver.build();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&sim);
    assert_eq!(DriverStatus::Running, fetch_driver_status(&ctx).await.unwrap());
}

#[rstest]
#[case("Completed")]
#[case("Failed")]
#[tokio::test]
async fn test_fetch_driver_status_driver_finished(sim: Simulation, opts: Options, #[case] status: &'static str) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(move |when, then| {
        when.path(format!("/apis/batch/v1/namespaces/{TEST_NAMESPACE}/jobs/sk-testing-driver"));
        then.json_body(json!({
            "status": {
                "conditions": [{"type": "Running"}, { "type": status }],
            },
        }));
    });
    fake_apiserver.build();
    let ctx = Arc::new(SimulationContext::new(client, opts)).with_sim(&sim);
    assert_eq!(DriverStatus::Finished, fetch_driver_status(&ctx).await.unwrap());
}
