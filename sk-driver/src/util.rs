use std::cmp::max;

use clockabilly::prelude::*;
use sk_core::prelude::*;
use tracing::*;

pub(crate) const DRIVER_PAUSED_WAIT_SECONDS: i64 = 10;

pub fn compute_step_size(speed: f64, start_ts: i64, end_ts: i64) -> i64 {
    let normal_step_duration = max(0, end_ts - start_ts) as f64;
    (normal_step_duration / speed) as i64
}

// This could be made more efficient and intuitive by using a watcher and message-passing so that we
// only get notified and pause/unpause if changes are actually made to the simulation object.  But
// that requires more rearchitecting than I really want to do right now, so for the time being we're
// just going to poll the apiserver every 10 seconds for changes.
pub(crate) async fn wait_if_paused(
    client: kube::Client,
    sim_name: &str,
    clock: Box<dyn Clockable + Send>,
) -> anyhow::Result<i64> {
    let sim_api: kube::Api<Simulation> = kube::Api::all(client.clone());
    let mut sim = sim_api.get(sim_name).await?;
    let mut total_paused_seconds = 0;

    if let Some(mut pause_time) = sim.spec.paused_time {
        // If the simulation was paused at start, then the pause time will be several seconds
        // before the current timestamp, which means we end up waiting another few seconds once the
        // simulation is resumed.  Not a huge deal, but slightly annoying, so we special-case it
        if sim
            .status
            .as_ref()
            .is_some_and(|st| pause_time < st.start_time.unwrap_or_default())
        {
            pause_time = clock.now();
        }

        debug!("simulation pause time = {pause_time}");
        let pause_duration = compute_step_size(sim.speed(), pause_time.timestamp(), clock.now_ts());
        while sim.spec.paused_time.is_some() {
            info!("simulation is paused, waiting for {DRIVER_PAUSED_WAIT_SECONDS} seconds...");
            clock.sleep(DRIVER_PAUSED_WAIT_SECONDS).await;
            total_paused_seconds += DRIVER_PAUSED_WAIT_SECONDS;
            sim = sim_api.get(sim_name).await?;
        }
        info!("simulation resumed; next event happens in {pause_duration} seconds");
        total_paused_seconds += pause_duration;
        clock.sleep(pause_duration).await;
    }
    Ok(total_paused_seconds)
}
