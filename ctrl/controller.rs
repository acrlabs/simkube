use std::ops::Deref;
use std::sync::Arc;

use anyhow::bail;
use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::api::core::v1 as corev1;
use kube::runtime::controller::Action;
use kube::ResourceExt;
use simkube::k8s::label_selector;
use simkube::prelude::*;
use tokio::time::Duration;
use tracing::*;

use super::objects::*;
use super::*;

const REQUEUE_DURATION: Duration = Duration::from_secs(5);
const REQUEUE_ERROR_DURATION: Duration = Duration::from_secs(300);

async fn do_global_setup(ctx: &SimulationContext, sim: &Simulation) -> anyhow::Result<SimulationRoot> {
    info!("performing global setup");

    let roots_api = kube::Api::<SimulationRoot>::all(ctx.client.clone());
    let ns_api = kube::Api::<corev1::Namespace>::all(ctx.client.clone());
    let webhook_api = kube::Api::<admissionv1::MutatingWebhookConfiguration>::all(ctx.client.clone());

    let root = match roots_api.get_opt(&ctx.root).await? {
        None => {
            info!("creating SimulationRoot");
            let obj = build_simulation_root(ctx, sim)?;
            roots_api.create(&Default::default(), &obj).await?
        },
        Some(r) => r,
    };

    if ns_api.get_opt(&ctx.driver_ns).await?.is_none() {
        info!("creating driver namespace {}", ctx.driver_ns);
        let obj = build_driver_namespace(ctx, sim)?;
        ns_api.create(&Default::default(), &obj).await?;
    };

    if webhook_api.get_opt(&ctx.webhook_name).await?.is_none() {
        info!("creating mutating webhook configuration {} for {}", ctx.webhook_name, ctx.name);
        let obj = build_mutating_webhook(ctx, &root)?;
        webhook_api.create(&Default::default(), &obj).await?;
    };

    Ok(root)
}

async fn setup_driver(ctx: &SimulationContext, sim: &Simulation, root: &SimulationRoot) -> anyhow::Result<Action> {
    info!("setting up simulation driver");

    let svc_api = kube::Api::<corev1::Service>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    let secrets_api = kube::Api::<corev1::Secret>::namespaced(ctx.client.clone(), &ctx.driver_ns);
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.client.clone(), &ctx.driver_ns);

    if svc_api.get_opt(&ctx.driver_svc).await?.is_none() {
        info!("creating driver service {}", &ctx.driver_svc);
        let obj = build_driver_service(ctx, root)?;
        svc_api.create(&Default::default(), &obj).await?;
    }

    if ctx.opts.use_cert_manager {
        cert_manager::create_certificate_if_not_present(ctx, root).await?;
    }

    let secrets = secrets_api.list(&label_selector(SIMULATION_LABEL_KEY, &ctx.name)).await?;
    let driver_cert_secret_name = match secrets.items.len() {
        0 => {
            info!("waiting for secret to be created");
            return Ok(Action::requeue(REQUEUE_DURATION));
        },
        x if x > 1 => bail!("found multiple secrets for experiment"),
        _ => secrets.items[0].name_any(),
    };

    // TODO should check if there are any other simulations running and block/wait until
    // they're done before proceeding
    let driver = jobs_api.get_opt(&ctx.driver_name).await?;
    if driver.is_none() {
        info!("creating driver job {}", ctx.driver_name);
        let obj = build_driver_job(ctx, sim, &driver_cert_secret_name, &sim.spec.trace)?;
        jobs_api.create(&Default::default(), &obj).await?;
    }

    Ok(Action::await_change())
}

pub(crate) async fn reconcile(sim: Arc<Simulation>, ctx: Arc<SimulationContext>) -> Result<Action, ReconcileError> {
    info!("got simulation object");

    let sim = sim.deref();
    let ctx = ctx.new_with_sim(sim);

    let root = do_global_setup(&ctx, sim).await?;
    Ok(setup_driver(&ctx, sim, &root).await?)
}

pub(crate) fn error_policy(sim: Arc<Simulation>, error: &ReconcileError, _: Arc<SimulationContext>) -> Action {
    error!("reconcile failed on simulation {}: {:?}", sim.namespaced_name(), error);
    Action::requeue(REQUEUE_ERROR_DURATION)
}
