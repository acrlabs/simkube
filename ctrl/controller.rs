use std::env;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::{
    anyhow,
    bail,
};
use chrono::{
    DateTime,
    Utc,
};
use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use kube::api::{
    ListParams,
    Patch,
};
use kube::error::ErrorResponse;
use kube::runtime::controller::Action;
use kube::ResourceExt;
use serde_json::json;
use simkube::api::v1::build_simulation_root;
use simkube::errors::*;
use simkube::k8s::{
    try_claim_lease,
    LeaseState,
};
use simkube::metrics::api::*;
use simkube::prelude::*;
use simkube::sim::{
    hooks,
    metrics_ns,
};
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tokio::time::Duration;

use crate::objects::*;
use crate::*;

pub(super) const REQUEUE_DURATION: Duration = Duration::from_secs(RETRY_DELAY_SECONDS as u64);
const REQUEUE_ERROR_DURATION: Duration = Duration::from_secs(ERROR_RETRY_DELAY_SECONDS as u64);
pub(super) const JOB_STATUS_CONDITION_COMPLETE: &str = "Complete";
pub(super) const JOB_STATUS_CONDITION_FAILED: &str = "Failed";

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

pub(super) async fn fetch_driver_status(
    ctx: &SimulationContext,
) -> anyhow::Result<(SimulationState, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> {
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    let (mut state, mut start_time, mut end_time) = (SimulationState::Initializing, None, None);

    if let Some(driver) = jobs_api.get_opt(&ctx.driver_name).await? {
        state = SimulationState::Running;
        if let Some(status) = driver.status {
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

    Ok((state, start_time, end_time))
}

pub(super) async fn setup_driver(
    ctx: &SimulationContext,
    sim: &Simulation,
    metaroot: &SimulationRoot,
    ctrl_ns: &str,
) -> anyhow::Result<Action> {
    info!("setting up simulation driver");

    // Validate the input before doing anything
    let ns_api = kube::Api::<corev1::Namespace>::all(ctx.client.clone());
    let metrics_ns = metrics_ns(sim);
    if ns_api.get_opt(&metrics_ns).await?.is_none() {
        bail!(SkControllerError::namespace_not_found(&metrics_ns));
    };

    match try_claim_lease(ctx.client.clone(), sim, metaroot, ctrl_ns, Box::new(UtcClock)).await? {
        LeaseState::Claimed => (),
        LeaseState::WaitingForClaim(t) => {
            info!("sleeping for {t} seconds");
            return Ok(Action::requeue(Duration::from_secs(t as u64)));
        },
        LeaseState::Unknown => bail!("unknown lease state"),
    }

    // Create the namespaces
    if ns_api.get_opt(&ctx.driver_ns).await?.is_none() {
        info!("creating driver namespace {}", ctx.driver_ns);
        let obj = build_driver_namespace(ctx, sim);
        ns_api.create(&Default::default(), &obj).await?;
    };

    // Set up the metrics collector
    let mut prom_ready = false;
    match &sim.spec.metrics_config {
        Some(mc) => {
            // if async closures ever become a thing, you could simplify this logic with .unwrap_or_else;
            // you might be able to hack something currently with futures.then(...), but I couldn't figure
            // out a good way to do so.
            let prom_api = kube::Api::<Prometheus>::namespaced(ctx.client.clone(), &metrics_ns);
            match prom_api.get_opt(&ctx.prometheus_name).await? {
                None => {
                    info!("creating Prometheus object {}/{}", metrics_ns, ctx.prometheus_name);
                    let obj = build_prometheus(&ctx.prometheus_name, sim, mc);
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
    let driver_svc_api = kube::Api::<corev1::Service>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    if driver_svc_api.get_opt(&ctx.driver_svc).await?.is_none() {
        info!("creating driver service {}", &ctx.driver_svc);
        let obj = build_driver_service(ctx, metaroot);
        driver_svc_api.create(&Default::default(), &obj).await?;
    }

    if ctx.opts.use_cert_manager {
        cert_manager::create_certificate_if_not_present(ctx, metaroot).await?;
    }

    let secrets_api = kube::Api::<corev1::Secret>::namespaced(ctx.client.clone(), &ctx.driver_ns);
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
    if webhook_api.get_opt(&ctx.webhook_name).await?.is_none() {
        info!("creating mutating webhook configuration {}", ctx.webhook_name);
        let obj = build_mutating_webhook(ctx, metaroot);
        webhook_api.create(&Default::default(), &obj).await?;
    };

    // Create the actual driver
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    if jobs_api.get_opt(&ctx.driver_name).await?.is_none() {
        info!("creating simulation driver {}", ctx.driver_name);
        let obj = build_driver_job(ctx, sim, &driver_cert_secret_name, ctrl_ns)?;
        jobs_api.create(&Default::default(), &obj).await?;
    }

    Ok(Action::await_change())
}

pub(super) async fn cleanup(ctx: &SimulationContext, sim: &Simulation) {
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(ctx.client.clone());
    let prom_api = kube::Api::<Prometheus>::namespaced(ctx.client.clone(), &metrics_ns(sim));

    info!("cleaning up simulation {}", ctx.name);
    if let Err(e) = roots_api.delete(&ctx.metaroot_name, &Default::default()).await {
        error!("Error cleaning up simulation: {e:?}");
    }

    info!("cleaning up prometheus resources");
    if let Err(e) = prom_api.delete(&ctx.prometheus_name, &Default::default()).await {
        if matches!(e, kube::Error::Api(ErrorResponse { code: 404, .. })) {
            warn!("prometheus object not found; maybe already cleaned up?");
        } else {
            error!("Error cleaning up Prometheus: {e:?}");
        }
    }
}

#[instrument(parent=None, skip_all, fields(simulation=sim.name_any()))]
pub(crate) async fn reconcile(sim: Arc<Simulation>, ctx: Arc<SimulationContext>) -> Result<Action, AnyhowError> {
    let sim = sim.deref();
    let ctx = ctx.with_sim(sim);

    let root = setup_sim_metaroot(&ctx, sim).await?;
    let (driver_state, start_time, end_time) = fetch_driver_status(&ctx).await?;

    let sim_api: kube::Api<Simulation> = kube::Api::all(ctx.client.clone());
    sim_api
        .patch_status(
            &sim.name_any(),
            &Default::default(),
            &Patch::Merge(json!({
            "status": {
                "observedGeneration": sim.metadata.generation.unwrap_or(1),
                "startTime": start_time,
                "endTime": end_time,
                "state": driver_state,
            }})),
        )
        .await
        .map_err(|e| anyhow!(e))?;

    let ctrl_ns = env::var(CTRL_NS_ENV_VAR).map_err(|e| anyhow!(e))?;
    match driver_state {
        SimulationState::Initializing => {
            hooks::execute(sim, hooks::Type::PreStart).await?;
            setup_driver(&ctx, sim, &root, &ctrl_ns).await.map_err(|e| e.into())
        },
        SimulationState::Running => Ok(Action::await_change()),
        SimulationState::Finished | SimulationState::Failed => {
            cleanup(&ctx, sim).await;
            hooks::execute(sim, hooks::Type::PostStop).await?;
            Ok(Action::await_change())
        },
        _ => unimplemented!(),
    }
}

pub(crate) fn error_policy(sim: Arc<Simulation>, err: &AnyhowError, ctx: Arc<SimulationContext>) -> Action {
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
