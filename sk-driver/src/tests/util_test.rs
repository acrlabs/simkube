use clockabilly::mock::MockUtcClock;
use clockabilly::prelude::*;
use clockabilly::{
    DateTime,
    Utc,
};
use httpmock::Method::*;

use super::*;
use crate::util::{
    DRIVER_PAUSED_WAIT_SECONDS,
    compute_step_size,
};

#[rstest]
#[case(1.0, 0, 10, 10)]
#[case(2.0, 0, 10, 5)]
#[case(1.0, 10, 0, 0)]
fn test_compute_step_size(#[case] speed: f64, #[case] start_ts: i64, #[case] end_ts: i64, #[case] expected: i64) {
    let result = compute_step_size(speed, start_ts, end_ts);
    assert_eq!(expected, result);
}

#[rstest(tokio::test)]
async fn test_wait_if_paused_not_paused(test_sim: Simulation) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let clock = MockUtcClock::boxed(0);

    fake_apiserver.handle(move |when, then| {
        when.method(GET)
            .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
        then.json_body_obj(&test_sim);
    });
    wait_if_paused(client, TEST_SIM_NAME, clock.clone()).await.unwrap();
    fake_apiserver.assert();
    assert_eq!(0, clock.now_ts());
}

#[rstest(tokio::test)]
#[case::before_start(DateTime::from_timestamp(-1, 0))]
#[case::at_start(DateTime::from_timestamp(0, 0))]
async fn test_wait_if_paused_paused(test_sim: Simulation, #[case] paused_time: Option<DateTime<Utc>>) {
    // This whole test is really ugly for what it's actually testing; we need to be able to change
    // the return value from the fake apiserver after some time has passed, right now the only way
    // to do that is to set up a callback in the fake clock and delete the old mock + insert a new
    // one.  This could be simplified quite a bit if https://github.com/alexliesenfeld/httpmock/issues/132
    // is accepted and lands.
    let (mut fake_apiserver, client) = make_fake_apiserver();
    let mut clock = MockUtcClock::boxed(0);

    let mut test_sim_paused = test_sim.clone();
    test_sim_paused.spec.paused_time = paused_time;

    // Return the "paused" simulation state at first
    let sim_handle_id = fake_apiserver.handle_multiple(2, move |when, then| {
        when.method(GET)
            .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
        then.json_body_obj(&test_sim_paused);
    });

    // After some time has passed, unpause the simulation
    let mut fake_apiserver_clone = fake_apiserver.clone();
    clock.add_callback(DRIVER_PAUSED_WAIT_SECONDS + 1, move || {
        fake_apiserver_clone.drop(sim_handle_id);
        let test_sim_clone = test_sim.clone(); // Don't understand why this clone is needed
        fake_apiserver_clone.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim_clone);
        });
    });

    wait_if_paused(client, TEST_SIM_NAME, clock.clone()).await.unwrap();

    assert_eq!(DRIVER_PAUSED_WAIT_SECONDS * 2, clock.now_ts());
    fake_apiserver.assert();
}
