use std::ops::Deref;
use std::sync::Arc;

use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::PostParams;
use kube::runtime::controller::Action;
use kube::ResourceExt;
use reqwest::Url;
use simkube::error::{
    SimKubeError,
    SimKubeResult,
};
use simkube::util::{
    add_common_fields,
    namespaced_name,
};
use simkube::{
    trace,
    Simulation,
    SimulationRoot,
    SimulationRootSpec,
};
use tokio::time::Duration;
use tracing::*;

use crate::trace::*;

pub struct SimulationContext {
    pub k8s_client: kube::Client,
}

fn create_simulation_root(simulation: &Simulation) -> SimKubeResult<SimulationRoot> {
    let mut root = SimulationRoot {
        metadata: metav1::ObjectMeta {
            name: Some(simulation.name_any()),
            ..metav1::ObjectMeta::default()
        },
        spec: SimulationRootSpec {},
    };
    add_common_fields(&simulation.name_any(), simulation, &mut root)?;

    return Ok(root);
}

fn create_driver_job(simulation: &Simulation) -> SimKubeResult<batchv1::Job> {
    let trace_path = Url::parse(&simulation.spec.trace)?;
    let (trace_vm, trace_volume, mount_path) = match trace::storage_type(&trace_path)? {
        trace::Scheme::AmazonS3 => todo!(),
        trace::Scheme::Local => get_local_trace_volume(&trace_path),
    };

    let mut job = batchv1::Job {
        metadata: metav1::ObjectMeta {
            namespace: Some(simulation.spec.driver_namespace.clone()),
            name: Some(format!("{}-driver", simulation.name_any())),
            ..metav1::ObjectMeta::default()
        },
        spec: Some(batchv1::JobSpec {
            backoff_limit: Some(1),
            template: corev1::PodTemplateSpec {
                spec: Some(corev1::PodSpec {
                    containers: vec![corev1::Container {
                        name: "driver".into(),
                        command: Some(vec!["/sk-driver".into()]),
                        args: Some(vec!["--trace-path".into(), mount_path]),
                        image: Some(simulation.spec.driver_image.clone()),
                        volume_mounts: Some(vec![trace_vm]),
                        ..corev1::Container::default()
                    }],
                    restart_policy: Some("Never".into()),
                    volumes: Some(vec![trace_volume]),
                    ..corev1::PodSpec::default()
                }),
                ..corev1::PodTemplateSpec::default()
            },
            ..batchv1::JobSpec::default()
        }),
        ..batchv1::Job::default()
    };
    add_common_fields(&simulation.name_any(), simulation, &mut job)?;

    return Ok(job);
}

pub async fn reconcile(simulation: Arc<Simulation>, ctx: Arc<SimulationContext>) -> SimKubeResult<Action> {
    let k8s_client = &ctx.k8s_client;
    info!("got simulation object: {:?}", simulation);

    let roots_api = kube::Api::<SimulationRoot>::all(k8s_client.clone());
    let jobs_api = kube::Api::<batchv1::Job>::namespaced(k8s_client.clone(), &simulation.spec.driver_namespace);
    match roots_api.get_opt(&simulation.name_any()).await? {
        None => {
            info!("creating SimulationRoot for {}", simulation.name_any());
            let root = create_simulation_root(simulation.deref())?;
            roots_api.create(&PostParams::default(), &root).await?;
        },
        Some(_) => {
            // TODO need to create the namespace

            // TODO should check if there are any other simulations running and block/wait until
            // they're done before proceeding
            info!("creating driver Job for {}", simulation.name_any());
            let job = create_driver_job(simulation.deref())?;
            jobs_api.create(&PostParams::default(), &job).await?;
        },
    }

    return Ok(Action::await_change());
}

pub fn error_policy(simulation: Arc<Simulation>, error: &SimKubeError, _: Arc<SimulationContext>) -> Action {
    warn!("reconcile failed on simulation {}: {:?}", namespaced_name(simulation.deref()), error);
    return Action::requeue(Duration::from_secs(5 * 60));
}
