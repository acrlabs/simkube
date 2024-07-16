use clockabilly::mock::MockUtcClock;
use clockabilly::{
    Clockable,
    DateTime,
};
// can't import prelude because that doesn't include "PATCH" for some reason
use httpmock::Method::*;
use k8s_openapi::api::coordination::v1 as coordinationv1;
use kube::error::ErrorResponse;
use serde_json::json;

use super::*;

const NOW: i64 = 15;
const TEST_LEASE_NS: &str = "simlease-ns";
const TEST_LEASE_DURATION: i64 = 10;

#[fixture]
fn lease_other_holder() -> coordinationv1::Lease {
    let holder = "some-other-sim";
    coordinationv1::Lease {
        spec: Some(coordinationv1::LeaseSpec {
            holder_identity: Some(holder.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_claim_lease_with_clock_already_owned_by_us(test_sim: Simulation, test_sim_root: SimulationRoot) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, clock.now());
    fake_apiserver
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let res = try_claim_lease_with_clock(client, &test_sim, &test_sim_root, TEST_LEASE_NS, clock)
        .await
        .unwrap();
    fake_apiserver.assert();
    assert_eq!(res, LeaseState::Claimed);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_claim_lease_with_clock_other_lease_unowned(test_sim: Simulation, test_sim_root: SimulationRoot) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let other_lease: coordinationv1::Lease = Default::default();
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, clock.now());
    fake_apiserver
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&other_lease);
        })
        .handle(move |when, then| {
            when.method(PUT)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let res = try_claim_lease_with_clock(client, &test_sim, &test_sim_root, TEST_LEASE_NS, clock)
        .await
        .unwrap();
    fake_apiserver.assert();
    assert_eq!(res, LeaseState::Claimed);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_claim_lease_with_clock_already_owned_by_other(
    test_sim: Simulation,
    test_sim_root: SimulationRoot,
    lease_other_holder: coordinationv1::Lease,
) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_other_holder);
        })
        .build();
    let res = try_claim_lease_with_clock(client, &test_sim, &test_sim_root, TEST_LEASE_NS, clock)
        .await
        .unwrap();
    fake_apiserver.assert();
    assert!(matches!(res, LeaseState::WaitingForClaim(..)));
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_claim_lease_with_clock(test_sim: Simulation, test_sim_root: SimulationRoot) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, clock.now());
    fake_apiserver
        .handle_not_found(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"))
        .handle(move |when, then| {
            when.method(POST)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases"));
            then.json_body_obj(&lease_obj);
        })
        .build();
    let res = try_claim_lease_with_clock(client, &test_sim, &test_sim_root, TEST_LEASE_NS, clock)
        .await
        .unwrap();
    fake_apiserver.assert();
    assert_eq!(res, LeaseState::Claimed);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_update_lease_with_clock_no_lease_found(test_sim: Simulation) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver
        .handle_not_found(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"))
        .build();
    let res = try_update_lease_with_clock(client, &test_sim, TEST_LEASE_NS, 10, clock)
        .await
        .unwrap_err();
    let err = res.downcast::<kube::Error>().unwrap();
    fake_apiserver.assert();
    assert!(matches!(err, kube::Error::Api(ErrorResponse { code: 404, .. })));
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_update_lease_with_clock_wrong_owner(test_sim: Simulation, lease_other_holder: coordinationv1::Lease) {
    let clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_other_holder);
        })
        .build();
    let res = try_update_lease_with_clock(client, &test_sim, TEST_LEASE_NS, 10, clock)
        .await
        .unwrap_err();
    let err = res.downcast::<KubernetesError>().unwrap();
    fake_apiserver.assert();
    assert!(matches!(err, KubernetesError::LeaseHeldByOther(..)));
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_try_update_lease_with_clock(test_sim: Simulation, test_sim_root: SimulationRoot) {
    let mut clock = MockUtcClock::new(NOW);
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, clock.now());
    let mut patched_lease_obj = build_lease(&test_sim, &test_sim_root, TEST_CTRL_NAMESPACE, clock.now());

    clock.advance(5);
    let renew_time = metav1::MicroTime(clock.now());
    patched_lease_obj.spec.as_mut().unwrap().lease_duration_seconds = Some(TEST_LEASE_DURATION as i32);
    patched_lease_obj.spec.as_mut().unwrap().renew_time = Some(renew_time.clone());

    fake_apiserver
        .handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"));
            then.json_body_obj(&lease_obj);
        })
        .handle(move |when, then| {
            when.method(PATCH)
                .path(format!("/apis/coordination.k8s.io/v1/namespaces/{TEST_LEASE_NS}/leases/{SK_LEASE_NAME}"))
                .json_body(json!({
                    "spec": {
                        "leaseDurationSeconds": TEST_LEASE_DURATION,
                        "renewTime": renew_time,
                    }
                }));
            then.json_body_obj(&patched_lease_obj);
        })
        .build();
    assert_eq!(
        (),
        try_update_lease_with_clock(client, &test_sim, TEST_LEASE_NS, 10, clock)
            .await
            .unwrap()
    );
    fake_apiserver.assert();
}

#[rstest]
#[case::no_data(None, None, RETRY_DELAY_SECONDS as i64)]
#[case::no_renew_time(Some(TEST_LEASE_DURATION), None, TEST_LEASE_DURATION + RETRY_DELAY_SECONDS as i64)]
#[case::no_duration_seconds(None, Some(13), 13 + RETRY_DELAY_SECONDS as i64 - NOW)]
#[case::valid(Some(TEST_LEASE_DURATION), Some(13), 23 + RETRY_DELAY_SECONDS as i64 - NOW)]
#[case::negative(Some(5), Some(2), RETRY_DELAY_SECONDS as i64)]
fn test_compute_remaining_lease_time_no_data(
    #[case] maybe_duration_seconds_64: Option<i64>,
    #[case] maybe_renew_ts: Option<i64>,
    #[case] expected: i64,
) {
    let maybe_renew_time = maybe_renew_ts.map(|ts| metav1::MicroTime(DateTime::from_timestamp(ts, 0).unwrap()));
    let maybe_duration_seconds = maybe_duration_seconds_64.map(|secs| secs as i32);
    assert_eq!(compute_remaining_lease_time(&maybe_duration_seconds, &maybe_renew_time, NOW), expected);
}
