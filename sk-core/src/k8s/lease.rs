use clockabilly::{
    Clockable,
    DateTime,
    Utc,
    UtcClock,
};
use k8s_openapi::api::coordination::v1 as coordinationv1;
use kube::api::Patch;
use kube::ResourceExt;
use serde_json::json;

use crate::k8s::{
    build_object_meta,
    KubernetesError,
};
use crate::prelude::*;

#[derive(Debug, Eq, PartialEq)]
pub enum LeaseState {
    Unknown,
    Claimed,
    WaitingForClaim(u64),
}

pub fn build_lease(sim: &Simulation, metaroot: &SimulationRoot, ns: &str, now: DateTime<Utc>) -> coordinationv1::Lease {
    let owner = metaroot;
    let sim_name = sim.name_any();
    coordinationv1::Lease {
        metadata: build_object_meta(ns, SK_LEASE_NAME, &sim_name, owner),
        spec: Some(coordinationv1::LeaseSpec {
            holder_identity: Some(sim_name),
            acquire_time: Some(metav1::MicroTime(now)),
            renew_time: Some(metav1::MicroTime(now)),
            ..Default::default()
        }),
    }
}

pub async fn try_claim_lease(
    client: kube::Client,
    sim: &Simulation,
    metaroot: &SimulationRoot,
    lease_ns: &str,
) -> anyhow::Result<LeaseState> {
    try_claim_lease_with_clock(client, sim, metaroot, lease_ns, UtcClock::new()).await
}

pub(super) async fn try_claim_lease_with_clock(
    client: kube::Client,
    sim: &Simulation,
    metaroot: &SimulationRoot,
    lease_ns: &str,
    clock: Box<dyn Clockable + Send>,
) -> anyhow::Result<LeaseState> {
    // Try to claim the lease -- leases are namespaced, so we create the lease in the same
    // namespace as the controller.  You could hypothetically work around this by running two
    // controllers in two different namespaces, but at that point you're deliberately trying to
    // subvert supported behaviour so you're on your own, kid.
    let lease_api = kube::Api::<coordinationv1::Lease>::namespaced(client.clone(), lease_ns);
    let lease_obj = build_lease(sim, metaroot, lease_ns, clock.now());
    let mut lease_state = LeaseState::Unknown;
    let mut lease_entry = lease_api
        .entry(SK_LEASE_NAME)
        .await?
        .and_modify(|lease| match &lease.spec {
            // Case 1: Some other named entity has the lease -- wait until the lease duration is
            // up (plus some margin) and then try to claim it again.
            Some(coordinationv1::LeaseSpec {
                holder_identity: Some(holder),
                lease_duration_seconds: maybe_duration_seconds,
                renew_time: maybe_renew_time,
                ..
            }) => {
                // If we already own the lease, do nothing; mark it as claimed and move on
                if sim.name_any() == *holder {
                    lease_state = LeaseState::Claimed;
                    return;
                }
                info!("another simulation is currently running: {holder}");
                let sleep_time = compute_remaining_lease_time(maybe_duration_seconds, maybe_renew_time, clock.now_ts());
                lease_state = LeaseState::WaitingForClaim(sleep_time as u64);
            },

            // Case 2: There is no lease or the lease is unowned -- then we just take it
            _ => *lease = lease_obj.clone(),
        })
        .or_insert(|| lease_obj);


    Ok(match lease_state {
        LeaseState::Unknown => {
            info!("trying to acquire lease");
            lease_entry.commit(&Default::default()).await?;
            LeaseState::Claimed
        },
        l => l,
    })
}

pub async fn try_update_lease(
    client: kube::Client,
    sim: &Simulation,
    lease_ns: &str,
    lease_duration: i64,
) -> EmptyResult {
    try_update_lease_with_clock(client, sim, lease_ns, lease_duration, UtcClock::new()).await
}

pub(super) async fn try_update_lease_with_clock(
    client: kube::Client,
    sim: &Simulation,
    lease_ns: &str,
    lease_duration: i64,
    clock: Box<dyn Clockable + Send>,
) -> EmptyResult {
    let lease_api = kube::Api::<coordinationv1::Lease>::namespaced(client.clone(), lease_ns);
    match lease_api.get(SK_LEASE_NAME).await?.spec {
        Some(coordinationv1::LeaseSpec { holder_identity: Some(holder), .. }) if holder != sim.name_any() => {
            return Err(KubernetesError::lease_held_by_other(&holder));
        },
        _ => (),
    }

    lease_api
        .patch(
            SK_LEASE_NAME,
            &Default::default(),
            &Patch::Merge(json!({
                "spec": {
                    "leaseDurationSeconds": lease_duration,
                    "renewTime": metav1::MicroTime(clock.now()),
                },
            })),
        )
        .await?;
    Ok(())
}

pub(super) fn compute_remaining_lease_time(
    maybe_duration_seconds: &Option<i32>,
    maybe_renew_time: &Option<metav1::MicroTime>,
    now_ts: i64,
) -> i64 {
    let duration_seconds = maybe_duration_seconds.map_or(0, |secs| secs as i64) + RETRY_DELAY_SECONDS as i64;
    let renew_time = maybe_renew_time
        .clone()
        .map(|microtime| microtime.0.timestamp())
        .unwrap_or(now_ts);
    let sleep_time = renew_time + duration_seconds - now_ts;
    if sleep_time <= 0 {
        warn!("exceeded the lease time but something hasn't released it; trying again");
        return RETRY_DELAY_SECONDS as i64;
    }
    sleep_time
}
