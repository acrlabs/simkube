use std::env;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::{
    anyhow,
    bail,
};
use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use kube::api::{
    ListParams,
    Patch,
};
use kube::runtime::controller::Action;
use serde_json::json;
use sk_api::prometheus::*;
use sk_api::v1::{
    Simulation,
    SimulationRoot,
    SimulationState,
};
use sk_core::constants::*;
use sk_core::errors::*;
use sk_core::hooks;
use sk_core::k8s::{
    LeaseState,
    build_simulation_root,
    is_terminal,
    metrics_ns,
    try_claim_lease,
};
use sk_core::prelude::*;
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tokio::time::Duration;
use tracing::*;

use crate::cert_manager;
use crate::context::SimulationContext;
use crate::errors::*;
use crate::objects::*;

pub const REQUEUE_DURATION: Duration = Duration::from_secs(RETRY_DELAY_SECONDS);
pub const REQUEUE_ERROR_DURATION: Duration = Duration::from_secs(ERROR_RETRY_DELAY_SECONDS);
pub const JOB_STATUS_CONDITION_COMPLETE: &str = "Complete";
pub const JOB_STATUS_CONDITION_FAILED: &str = "Failed";

type SimulationStatusPatch = serde_json::Value;

async fn setup_sim_metaroot(ctx: &SimulationContext, sim: &Simulation) -> anyhow::Result<SimulationRoot> {
    let roots_api = kube::Api::<SimulationRoot>::all(ctx.client.clone());
    match roots_api.get_opt(&ctx.metaroot_name).await? {
        None => {
            info!("creating Simulation MetaRoot");
            let metaroot = build_simulation_root(&ctx.metaroot_name, sim);
            roots_api.create(&Default::default(), &metaroot).await.map_err(|e| e.into())
        },
        Some(metaroot) => Ok(metaroot),
    }
}

// The "left" driver state contains an actual status from the driver job, along with its start and
// end times (if they exist).  The "right" driver state contains an "inferred" state, such as
// "blocked" (i.e., the driver hasn't even been created yet because we couldn't claim the lease).
type DriverState = (SimulationState, SimulationStatusPatch, u64);

pub(crate) async fn fetch_driver_state(
    ctx: &SimulationContext,
    sim: &Simulation,
    metaroot: &SimulationRoot,
    ctrl_ns: &str,
) -> anyhow::Result<DriverState> {
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &sim.spec.driver.namespace);
    let (mut state, mut start_time, mut end_time, mut completed, mut blocked_duration) =
        (SimulationState::Initializing, None, None, None, 0);

    if let Some(driver) = jobs_api.get_opt(&ctx.driver_name).await? {
        state = SimulationState::Running;
        if let Some(status) = driver.status {
            completed = status.succeeded;
            start_time = status.start_time.map(|t| t.0);
            if let Some(cond) =
                status.conditions.unwrap_or_default().iter().find(|cond| {
                    cond.type_ == JOB_STATUS_CONDITION_COMPLETE || cond.type_ == JOB_STATUS_CONDITION_FAILED
                })
            {
                end_time = cond.last_transition_time.as_ref().map(|t| t.0);
                state = if cond.type_ == JOB_STATUS_CONDITION_COMPLETE {
                    SimulationState::Finished
                } else {
                    SimulationState::Failed
                };
            }
        }
    }

    // State processing if the simulation hasn't completed
    if !is_terminal(&state) {
        // If the simulation hasn't started yet, we don't want to set the state to paused; this way
        // we can still bring up the driver pod and do something with it before the simulation
        // proper starts.  Once the driver pod comes up, the reconciler will get re-triggered and
        // update the state.
        if state != SimulationState::Initializing && sim.spec.paused_time.is_some() {
            state = SimulationState::Paused;
        }

        // It's a little weird that we're trying to claim a lease inside the "fetch_driver_state" function,
        // but whatever, there are worse things.
        //
        // It would be cool to check the error and return Blocked if something else nabbed the lease first.
        // But it's not actually that important, because it will just requeue and on the next time through
        // it will correctly determine the Blocked status, so I'm not sure it's worth the increased
        // complexity.
        match try_claim_lease(ctx.client.clone(), sim, metaroot, ctrl_ns).await? {
            LeaseState::Claimed => (),
            LeaseState::WaitingForClaim(t) => {
                state = SimulationState::Blocked;
                blocked_duration = t;
            },
            LeaseState::Unknown => bail!("unknown lease state"),
        }
    }

    let patch = json!({
    "status": {
        "observedGeneration": sim.metadata.generation.unwrap_or(1),
        "startTime": start_time,
        "endTime": end_time,
        "completedRuns": completed,
        "state": state,
    }});

    Ok((state, patch, blocked_duration))
}

pub async fn setup_simulation(
    ctx: &SimulationContext,
    sim: &Simulation,
    metaroot: &SimulationRoot,
    ctrl_ns: &str,
) -> anyhow::Result<Action> {
    info!("setting up simulation");

    hooks::execute(sim, hooks::Type::PreStart).await?;

    // Validate the input before doing anything
    let ns_api = kube::Api::<corev1::Namespace>::all(ctx.client.clone());
    let metrics_ns = metrics_ns(sim);
    if ns_api.get_opt(&metrics_ns).await?.is_none() {
        bail!(SkControllerError::namespace_not_found(&metrics_ns));
    };

    // Create the namespaces
    if ns_api.get_opt(&sim.spec.driver.namespace).await?.is_none() {
        info!("creating driver namespace {}", sim.spec.driver.namespace);
        let obj = build_driver_namespace(ctx, sim);
        ns_api.create(&Default::default(), &obj).await?;
    };

    // Set up the metrics collector
    let mut prom_ready = false;
    match &sim.spec.metrics {
        Some(mc) => {
            // if async closures ever become a thing, you could simplify this logic with .unwrap_or_else;
            // you might be able to hack something currently with futures.then(...), but I couldn't figure
            // out a good way to do so.
            let prom_api = kube::Api::<Prometheus>::namespaced(ctx.client.clone(), &metrics_ns);
            match prom_api.get_opt(&ctx.prometheus_name).await? {
                None => {
                    info!("creating Prometheus object {}/{}", metrics_ns, ctx.prometheus_name);
                    let obj = build_prometheus(&ctx.prometheus_name, sim, metaroot, mc);
                    prom_api.create(&Default::default(), &obj).await?;
                },
                Some(prom) => {
                    if let Some(PrometheusStatus { available_replicas: reps, .. }) = prom.status {
                        prom_ready = reps > 0;
                    }
                },
            }
        },
        _ => prom_ready = true,
    }

    if !prom_ready {
        info!("waiting for prometheus to be ready");
        return Ok(Action::requeue(REQUEUE_DURATION));
    }

    // Set up the webhook
    let driver_svc_api = kube::Api::<corev1::Service>::namespaced(ctx.client.clone(), &sim.spec.driver.namespace);
    if driver_svc_api.get_opt(&ctx.driver_svc).await?.is_none() {
        info!("creating driver service {}", &ctx.driver_svc);
        let obj = build_driver_service(ctx, sim, metaroot);
        driver_svc_api.create(&Default::default(), &obj).await?;
    }

    if ctx.opts.use_cert_manager {
        cert_manager::create_certificate_if_not_present(ctx, sim, metaroot).await?;
    }

    let secrets_api = kube::Api::<corev1::Secret>::namespaced(ctx.client.clone(), &sim.spec.driver.namespace);
    let secrets = secrets_api
        .list(&ListParams {
            label_selector: Some(format!("{SIMULATION_LABEL_KEY}={}", ctx.name)),
            ..Default::default()
        })
        .await?;
    let driver_cert_secret_name = match secrets.items.len() {
        0 => {
            info!("waiting for secret to be created");
            return Ok(Action::requeue(REQUEUE_DURATION));
        },
        x if x > 1 => bail!("found multiple secrets for experiment"),
        _ => secrets.items[0].name_any(),
    };

    let webhook_api = kube::Api::<admissionv1::MutatingWebhookConfiguration>::all(ctx.client.clone());
    let mwc_opt = webhook_api.get_opt(&ctx.webhook_name).await?;
    if mwc_opt.is_none() {
        info!("creating mutating webhook configuration {}", ctx.webhook_name);
        let obj = build_mutating_webhook(ctx, sim, metaroot);
        webhook_api.create(&Default::default(), &obj).await?;
        return Ok(Action::requeue(REQUEUE_DURATION));
    }
    if let Some(mwc) = &mwc_opt
        && let Some(webhooks) = &mwc.webhooks
        // We create one webhook in this configuration but webhooks is a Vec<MutatingWebhooks>
        && let Some(webhook) = &webhooks.first()
        // ca_bundle is a ByteString, a tuple struct where .0 is the inner Vec<u8>.
        // If the ca_bundle is None or empty, it has not been populated by cert-manager yet.
        && webhook.client_config.ca_bundle.as_ref().is_none_or(|b| b.0.is_empty())
    {
        info!(
            "MutatingWebhookConfiguration {} exists but caBundle not yet populated, requeuing.",
            ctx.webhook_name
        );
        return Ok(Action::requeue(REQUEUE_DURATION));
    }

    // Create the actual driver
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &sim.spec.driver.namespace);
    if jobs_api.get_opt(&ctx.driver_name).await?.is_none() {
        info!("creating simulation driver {}", ctx.driver_name);
        let obj = build_driver_job(ctx, sim, ctx.opts.driver_secrets.as_ref(), &driver_cert_secret_name, ctrl_ns)?;
        jobs_api.create(&Default::default(), &obj).await?;
    }

    Ok(Action::await_change())
}

pub async fn cleanup_simulation(ctx: &SimulationContext, sim: &Simulation) {
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(ctx.client.clone());

    info!("cleaning up simulation {}", ctx.name);
    if let Err(e) = roots_api.delete(&ctx.metaroot_name, &Default::default()).await {
        error!("Error cleaning up simulation: {e:?}");
    }

    if let Err(e) = hooks::execute(sim, hooks::Type::PostStop).await {
        error!("Error running PostStop hooks: {e:?}");
    }
}

#[instrument(parent=None, skip_all, fields(simulation=sim.name_any()))]
pub async fn reconcile(sim: Arc<Simulation>, ctx: Arc<SimulationContext>) -> Result<Action, AnyhowError> {
    let sim = sim.deref();
    let ctx = ctx.with_sim(sim);
    let ctrl_ns = env::var(CTRL_NS_ENV_VAR).map_err(|e| anyhow!(e))?;

    let metaroot = setup_sim_metaroot(&ctx, sim).await?;
    let (simulation_state, status_patch, blocked_duration) = fetch_driver_state(&ctx, sim, &metaroot, &ctrl_ns).await?;

    debug!("sending patch status update: {status_patch}");
    let sim_api: kube::Api<Simulation> = kube::Api::all(ctx.client.clone());
    sim_api
        .patch_status(&sim.name_any(), &Default::default(), &Patch::Merge(status_patch))
        .await
        .map_err(|e| anyhow!(e))?;

    match simulation_state {
        SimulationState::Initializing => setup_simulation(&ctx, sim, &metaroot, &ctrl_ns).await.map_err(|e| e.into()),
        SimulationState::Blocked => {
            info!("simulation blocked; sleeping for {blocked_duration} seconds");
            Ok(Action::requeue(Duration::from_secs(blocked_duration)))
        },
        SimulationState::Running | SimulationState::Paused => Ok(Action::await_change()),
        SimulationState::Finished | SimulationState::Failed => {
            // This action should never return an error, we want to try cleaning up once and if it
            // doesn't work, just abort (may revisit this in the future)
            cleanup_simulation(&ctx, sim).await;
            Ok(Action::await_change())
        },

        // The driver itself can never return "Retrying", this is only set by the controller's
        // error_policy (see below).  If the driver were to return Retrying that would be very
        // weird indeed and we should panic.
        //
        // I have some qualms about having a simulation state that doesn't match 1-1 with the
        // driver state, but then also using the same enum for both... but I think in this specific
        // circumstance it's OK.
        SimulationState::Retrying => unimplemented!(),
    }
}

pub fn error_policy(sim: Arc<Simulation>, err: &AnyhowError, ctx: Arc<SimulationContext>) -> Action {
    skerr!(err, "reconcile failed on simulation {}", sim.namespaced_name());
    let (action, state) = if err.is::<SkControllerError>() {
        (Action::await_change(), SimulationState::Failed)
    } else {
        (Action::requeue(REQUEUE_ERROR_DURATION), SimulationState::Retrying)
    };

    let sim_api: kube::Api<Simulation> = kube::Api::all(ctx.client.clone());
    if let Err(e) = block_in_place(|| {
        Handle::current().block_on(sim_api.patch_status(
            &sim.name_any(),
            &Default::default(),
            &Patch::Merge(json!({
            "status": {
                "state": state,
            }})),
        ))
    }) {
        error!("failure updating simulation state for {}: {e:?}", sim.namespaced_name());
    }

    action
}
