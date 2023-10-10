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

async fn do_global_setup(
    sim_name: &str,
    simulation: &Simulation,
    driver_ns_name: &str,
    driver_svc_name: &str,
    sim_root_name: &str,
    ctx: &SimulationContext,
) -> anyhow::Result<SimulationRoot> {
    info!("performing global setup");

    let roots_api = kube::Api::<SimulationRoot>::all(ctx.k8s_client.clone());
    let ns_api = kube::Api::<corev1::Namespace>::all(ctx.k8s_client.clone());
    let webhook_api = kube::Api::<admissionv1::MutatingWebhookConfiguration>::all(ctx.k8s_client.clone());

    let root = match roots_api.get_opt(sim_root_name).await? {
        None => {
            info!("creating SimulationRoot for {}", sim_name);
            let obj = build_simulation_root(sim_root_name, sim_name, simulation)?;
            roots_api.create(&Default::default(), &obj).await?
        },
        Some(r) => r,
    };

    if ns_api.get_opt(driver_ns_name).await?.is_none() {
        info!("creating driver namespace {} for {}", driver_ns_name, sim_name);
        let obj = build_driver_namespace(driver_ns_name, sim_name, simulation)?;
        ns_api.create(&Default::default(), &obj).await?;
    };

    let webhook_config_name = &mutating_webhook_config_name(sim_name);
    if webhook_api.get_opt(webhook_config_name).await?.is_none() {
        info!("creating mutating webhook configuration {} for {}", webhook_config_name, sim_name);
        let obj = build_mutating_webhook(
            webhook_config_name,
            driver_ns_name,
            driver_svc_name,
            ctx.opts.driver_port,
            ctx.opts.use_cert_manager,
            sim_name,
            &root,
        )?;
        webhook_api.create(&Default::default(), &obj).await?;
    };

    Ok(root)
}

async fn setup_driver(
    sim_name: &str,
    simulation: &Simulation,
    driver_ns_name: &str,
    driver_svc_name: &str,
    root: &SimulationRoot,
    ctx: &SimulationContext,
) -> anyhow::Result<Action> {
    info!("setting up simulation driver");

    let svc_api = kube::Api::<corev1::Service>::namespaced(ctx.k8s_client.clone(), driver_ns_name);
    let secrets_api = kube::Api::<corev1::Secret>::namespaced(ctx.k8s_client.clone(), driver_ns_name);
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(ctx.k8s_client.clone(), driver_ns_name);

    let driver_name = &sim_driver_name(sim_name);

    if svc_api.get_opt(driver_svc_name).await?.is_none() {
        info!("creating driver service {} for {}", driver_svc_name, sim_name);
        let obj =
            build_driver_service(driver_ns_name, driver_svc_name, driver_name, ctx.opts.driver_port, sim_name, root)?;
        svc_api.create(&Default::default(), &obj).await?;
    }

    if ctx.opts.use_cert_manager {
        cert_manager::create_certificate_if_not_present(
            ctx.k8s_client.clone(),
            driver_ns_name,
            driver_svc_name,
            &ctx.opts.cert_manager_issuer,
            sim_name,
            root,
        )
        .await?;
    }

    let secrets = secrets_api.list(&label_selector(SIMULATION_LABEL_KEY, sim_name)).await?;
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
    let driver = jobs_api.get_opt(driver_name).await?;
    if driver.is_none() {
        info!("creating driver job {} for {}", driver_name, simulation.name_any());
        let obj = build_driver_job(
            driver_ns_name,
            driver_name,
            &driver_cert_secret_name,
            &ctx.opts.driver_image,
            &simulation.spec.trace,
            &ctx.sim_svc_account,
            sim_name,
            root,
            simulation,
        )?;
        jobs_api.create(&Default::default(), &obj).await?;
    }

    Ok(Action::await_change())
}

pub(crate) async fn reconcile(
    simulation: Arc<Simulation>,
    ctx: Arc<SimulationContext>,
) -> Result<Action, ReconcileError> {
    info!("got simulation object: {:?}", simulation);

    let simulation = simulation.deref();
    let ctx = ctx.deref();

    let sim_name = &simulation.name_any();
    let root_name = &sim_root_name(sim_name);
    let driver_ns_name = &simulation.spec.driver_namespace;
    let driver_svc_name = &driver_service_name(sim_name);

    let root = do_global_setup(sim_name, simulation, driver_ns_name, driver_svc_name, root_name, ctx).await?;
    Ok(setup_driver(sim_name, simulation, driver_ns_name, driver_svc_name, &root, ctx).await?)
}

pub(crate) fn error_policy(simulation: Arc<Simulation>, error: &ReconcileError, _: Arc<SimulationContext>) -> Action {
    warn!("reconcile failed on simulation {}: {:?}", simulation.namespaced_name(), error);
    Action::requeue(REQUEUE_ERROR_DURATION)
}
