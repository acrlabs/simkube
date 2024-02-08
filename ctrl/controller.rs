use std::ops::Deref;
use std::sync::Arc;

use anyhow::bail;
use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use kube::runtime::controller::Action;
use kube::ResourceExt;
use simkube::errors::*;
use simkube::k8s::label_selector;
use simkube::metrics::api::{
    build_ksm_service_monitor,
    build_prometheus,
    Prometheus,
    PrometheusStatus,
    ServiceMonitor,
};
use simkube::prelude::*;
use tokio::time::Duration;

use super::*;

const REQUEUE_DURATION: Duration = Duration::from_secs(5);
const REQUEUE_ERROR_DURATION: Duration = Duration::from_secs(300);
const KSM_SVC_MON_NAME: &str = "kube-state-metrics-fine-grained";

#[derive(Debug, Eq, PartialEq)]
pub enum DriverStatus {
    Waiting,
    Running,
    Finished,
}

async fn setup_sim_root(ctx: &SimulationContext, sim: &Simulation) -> anyhow::Result<SimulationRoot> {
    let roots_api = kube::Api::<SimulationRoot>::all(ctx.client.clone());
    match roots_api.get_opt(&ctx.root).await? {
        None => {
            info!("creating SimulationRoot");
            let root = build_simulation_root(ctx, sim)?;
            roots_api.create(&Default::default(), &root).await.map_err(|e| e.into())
        },
        Some(root) => Ok(root),
    }
}

pub(super) async fn fetch_driver_status(ctx: &SimulationContext) -> anyhow::Result<DriverStatus> {
    // TODO should check if there are any other simulations running and block/wait until
    // they're done before proceeding
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    if let Some(driver) = jobs_api.get_opt(&ctx.driver_name).await? {
        if driver.status.is_some_and(|s| {
            s.conditions
                .unwrap_or_default()
                .iter()
                .any(|cond| cond.type_ == "Completed" || cond.type_ == "Failed")
        }) {
            Ok(DriverStatus::Finished)
        } else {
            Ok(DriverStatus::Running)
        }
    } else {
        Ok(DriverStatus::Waiting)
    }
}

async fn setup_driver(ctx: &SimulationContext, sim: &Simulation, root: &SimulationRoot) -> anyhow::Result<Action> {
    info!("setting up simulation driver");

    // Create the namespaces
    let ns_api = kube::Api::<corev1::Namespace>::all(ctx.client.clone());
    if ns_api.get_opt(&ctx.driver_ns).await?.is_none() {
        info!("creating driver namespace {}", ctx.driver_ns);
        let obj = build_driver_namespace(ctx, sim)?;
        ns_api.create(&Default::default(), &obj).await?;
    };
    if ns_api.get_opt(&ctx.monitoring_ns).await?.is_none() {
        bail!("monitoring namespace not found: {}", ctx.monitoring_ns);
    };

    // Create the monitoring objects
    let svc_mon_api = kube::Api::<ServiceMonitor>::namespaced(ctx.client.clone(), &ctx.monitoring_ns);
    if svc_mon_api.get_opt(KSM_SVC_MON_NAME).await?.is_none() {
        info!("creating Prometheus ServiceMonitor object {}/{}", ctx.monitoring_ns, KSM_SVC_MON_NAME);
        let obj = build_ksm_service_monitor(KSM_SVC_MON_NAME, sim)?;
        svc_mon_api.create(&Default::default(), &obj).await?;
    }

    // if async closures ever become a thing, you could simplify this logic with .unwrap_or_else;
    // you might be able to hack something currently with futures.then(...), but I couldn't figure
    // out a good way to do so.
    let prom_api = kube::Api::<Prometheus>::namespaced(ctx.client.clone(), &ctx.monitoring_ns);
    let mut prom_ready = false;
    match prom_api.get_opt(&ctx.prometheus_name).await? {
        None => {
            info!("creating Prometheus object {}/{}", ctx.monitoring_ns, ctx.prometheus_name);
            let obj = build_prometheus(&ctx.prometheus_name, KSM_SVC_MON_NAME, sim)?;
            prom_api.create(&Default::default(), &obj).await?;
        },
        Some(prom) => {
            if let Some(PrometheusStatus { available_replicas: reps, .. }) = prom.status {
                prom_ready = reps > 0;
            }
        },
    }

    if !prom_ready {
        info!("waiting for prometheus to be ready");
        return Ok(Action::requeue(REQUEUE_DURATION));
    }

    // Set up the webhook
    let svc_api = kube::Api::<corev1::Service>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    if svc_api.get_opt(&ctx.driver_svc).await?.is_none() {
        info!("creating driver service {}", &ctx.driver_svc);
        let obj = build_driver_service(ctx, root)?;
        svc_api.create(&Default::default(), &obj).await?;
    }

    if ctx.opts.use_cert_manager {
        cert_manager::create_certificate_if_not_present(ctx, root).await?;
    }

    let secrets_api = kube::Api::<corev1::Secret>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    let secrets = secrets_api.list(&label_selector(SIMULATION_LABEL_KEY, &ctx.name)).await?;
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
        let obj = build_mutating_webhook(ctx, root)?;
        webhook_api.create(&Default::default(), &obj).await?;
    };

    // Create the actual driver
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    if jobs_api.get_opt(&ctx.driver_name).await?.is_none() {
        info!("creating simulation driver {}", ctx.driver_name);
        let obj = build_driver_job(ctx, sim, &driver_cert_secret_name, &sim.spec.trace)?;
        jobs_api.create(&Default::default(), &obj).await?;
    }

    Ok(Action::await_change())
}

async fn cleanup(ctx: &SimulationContext) {
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(ctx.client.clone());
    let svc_mon_api = kube::Api::<ServiceMonitor>::namespaced(ctx.client.clone(), &ctx.monitoring_ns);
    let prom_api = kube::Api::<Prometheus>::namespaced(ctx.client.clone(), &ctx.monitoring_ns);

    info!("cleaning up simulation {}", ctx.name);
    if let Err(e) = roots_api.delete(&ctx.root, &Default::default()).await {
        error!("Error cleaning up simulation: {e:?}");
    }

    info!("cleaning up prometheus resources");
    if let Err(e) = svc_mon_api.delete(KSM_SVC_MON_NAME, &Default::default()).await {
        error!("Error cleaning up Prometheus service monitor configuration: {e:?}");
    }

    if let Err(e) = prom_api.delete(&ctx.prometheus_name, &Default::default()).await {
        error!("Error cleaning up Prometheus: {e:?}");
    }
}

#[instrument(parent=None, skip_all, fields(simulation=sim.name_any()))]
pub(crate) async fn reconcile(sim: Arc<Simulation>, ctx: Arc<SimulationContext>) -> Result<Action, AnyhowError> {
    let sim = sim.deref();
    let ctx = ctx.with_sim(sim);

    let root = setup_sim_root(&ctx, sim).await?;
    match fetch_driver_status(&ctx).await? {
        DriverStatus::Waiting => setup_driver(&ctx, sim, &root).await.map_err(|e| e.into()),
        DriverStatus::Running => Ok(Action::await_change()),
        DriverStatus::Finished => {
            cleanup(&ctx).await;
            Ok(Action::await_change())
        },
    }
}

pub(crate) fn error_policy(sim: Arc<Simulation>, err: &AnyhowError, _: Arc<SimulationContext>) -> Action {
    skerr!(err, "reconcile failed on simulation {}", sim.namespaced_name());
    Action::requeue(REQUEUE_ERROR_DURATION)
}
