use std::ops::Deref;
use std::sync::Arc;

use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::runtime::controller::Action;
use kube::ResourceExt;
use reqwest::Url;
use simkube::k8s::add_common_fields;
use simkube::prelude::*;
use simkube::store::storage;
use tokio::time::Duration;
use tracing::*;

use super::trace::get_local_trace_volume;
use super::ReconcileError;

pub(super) struct SimulationContext {
    pub(super) k8s_client: kube::Client,
    pub(super) driver_image: String,
}

fn create_simulation_root(simulation: &Simulation) -> anyhow::Result<SimulationRoot> {
    let mut root = SimulationRoot {
        metadata: metav1::ObjectMeta {
            name: Some(simulation.name_any()),
            ..Default::default()
        },
        spec: SimulationRootSpec {},
    };
    add_common_fields(&simulation.name_any(), simulation, &mut root)?;

    Ok(root)
}

fn create_driver_job(simulation: &Simulation, sim_root_name: &str, driver_image: &str) -> anyhow::Result<batchv1::Job> {
    let trace_path = Url::parse(&simulation.spec.trace)?;
    let (trace_vm, trace_volume, mount_path) = match storage::get_scheme(&trace_path)? {
        storage::Scheme::AmazonS3 => todo!(),
        storage::Scheme::Local => get_local_trace_volume(&trace_path)?,
    };

    let mut job = batchv1::Job {
        metadata: metav1::ObjectMeta {
            namespace: Some(simulation.spec.driver_namespace.clone()),
            name: Some(format!("{}-driver", simulation.name_any())),
            ..Default::default()
        },
        spec: Some(batchv1::JobSpec {
            backoff_limit: Some(1),
            template: corev1::PodTemplateSpec {
                spec: Some(corev1::PodSpec {
                    containers: vec![corev1::Container {
                        name: "driver".into(),
                        command: Some(vec!["/sk-driver".into()]),
                        args: Some(vec![
                            "--trace-path".into(),
                            mount_path,
                            "--sim-namespace-prefix".into(),
                            "virtual".into(),
                            "--sim-root".into(),
                            sim_root_name.into(),
                            "--sim-name".into(),
                            simulation.name_any(),
                        ]),
                        image: Some(driver_image.into()),
                        volume_mounts: Some(vec![trace_vm]),
                        ..Default::default()
                    }],
                    restart_policy: Some("Never".into()),
                    volumes: Some(vec![trace_volume]),
                    service_account: Some("sk-ctrl-service-account-c8688aad".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    add_common_fields(&simulation.name_any(), simulation, &mut job)?;

    Ok(job)
}

pub(crate) async fn reconcile(
    simulation: Arc<Simulation>,
    ctx: Arc<SimulationContext>,
) -> Result<Action, ReconcileError> {
    let k8s_client = &ctx.k8s_client;
    info!("got simulation object: {:?}", simulation);

    let roots_api = kube::Api::<SimulationRoot>::all(k8s_client.clone());
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(k8s_client.clone(), &simulation.spec.driver_namespace);
    match roots_api.get_opt(&simulation.name_any()).await? {
        None => {
            info!("creating SimulationRoot for {}", simulation.name_any());
            let root = create_simulation_root(simulation.deref())?;
            roots_api.create(&Default::default(), &root).await?;
        },
        Some(root) => {
            // TODO need to create the namespace

            // TODO should check if there are any other simulations running and block/wait until
            // they're done before proceeding
            info!("creating driver Job for {}", simulation.name_any());
            let job = create_driver_job(simulation.deref(), &root.name_any(), &ctx.driver_image)?;
            jobs_api.create(&Default::default(), &job).await?;
        },
    }

    Ok(Action::await_change())
}

pub(crate) fn error_policy(simulation: Arc<Simulation>, error: &ReconcileError, _: Arc<SimulationContext>) -> Action {
    warn!("reconcile failed on simulation {}: {:?}", simulation.namespaced_name(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}
