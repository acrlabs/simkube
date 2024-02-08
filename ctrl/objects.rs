use std::env;

use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use reqwest::Url;
use simkube::k8s::{
    build_global_object_meta,
    build_object_meta,
};
use simkube::macros::*;
use simkube::prelude::*;
use simkube::store::storage;

use super::cert_manager::DRIVER_CERT_NAME;
use super::trace::get_local_trace_volume;
use crate::SimulationContext;

const WEBHOOK_NAME: &str = "mutatepods.simkube.io";
const DRIVER_CERT_VOLUME: &str = "driver-cert";

pub(super) fn build_simulation_root(ctx: &SimulationContext, owner: &Simulation) -> anyhow::Result<SimulationRoot> {
    Ok(SimulationRoot {
        metadata: build_global_object_meta(&ctx.root, &ctx.name, owner)?,
        spec: SimulationRootSpec {},
    })
}

pub(super) fn build_driver_namespace(ctx: &SimulationContext, owner: &Simulation) -> anyhow::Result<corev1::Namespace> {
    Ok(corev1::Namespace {
        metadata: build_global_object_meta(&ctx.driver_ns, &ctx.name, owner)?,
        ..Default::default()
    })
}

pub(super) fn build_mutating_webhook(
    ctx: &SimulationContext,
    owner: &SimulationRoot,
) -> anyhow::Result<admissionv1::MutatingWebhookConfiguration> {
    let mut metadata = build_global_object_meta(&ctx.webhook_name, &ctx.name, owner)?;
    if ctx.opts.use_cert_manager {
        metadata
            .annotations
            .get_or_insert(BTreeMap::new())
            .insert("cert-manager.io/inject-ca-from".into(), format!("{}/{}", ctx.driver_ns, DRIVER_CERT_NAME));
    }

    Ok(admissionv1::MutatingWebhookConfiguration {
        metadata,
        webhooks: Some(vec![admissionv1::MutatingWebhook {
            admission_review_versions: vec!["v1".into()],
            client_config: admissionv1::WebhookClientConfig {
                service: Some(admissionv1::ServiceReference {
                    namespace: ctx.driver_ns.clone(),
                    name: ctx.driver_svc.clone(),
                    port: Some(ctx.opts.driver_port),
                    ..Default::default()
                }),
                ..Default::default()
            },
            failure_policy: Some("Ignore".into()),
            name: WEBHOOK_NAME.into(),
            side_effects: "None".into(),
            rules: Some(vec![admissionv1::RuleWithOperations {
                api_groups: Some(vec!["".into()]),
                api_versions: Some(vec!["v1".into()]),
                operations: Some(vec!["CREATE".into()]),
                resources: Some(vec!["pods".into()]),
                scope: Some("Namespaced".into()),
            }]),
            ..Default::default()
        }]),
    })
}

pub(super) fn build_driver_service(ctx: &SimulationContext, owner: &SimulationRoot) -> anyhow::Result<corev1::Service> {
    Ok(corev1::Service {
        metadata: build_object_meta(&ctx.driver_ns, &ctx.driver_svc, &ctx.name, owner)?,
        spec: Some(corev1::ServiceSpec {
            ports: Some(vec![corev1::ServicePort {
                port: ctx.opts.driver_port,
                target_port: Some(IntOrString::Int(ctx.opts.driver_port)),
                ..Default::default()
            }]),
            selector: klabel!("job-name" => ctx.driver_name),
            ..Default::default()
        }),
        ..Default::default()
    })
}

pub(super) fn build_driver_job(
    ctx: &SimulationContext,
    owner: &Simulation,
    cert_secret_name: &str,
    trace_path: &str,
) -> anyhow::Result<batchv1::Job> {
    let trace_url = Url::parse(trace_path)?;
    let (trace_vm, trace_volume, trace_mount_path) = match storage::get_scheme(&trace_url)? {
        storage::Scheme::AmazonS3 => todo!(),
        storage::Scheme::Local => get_local_trace_volume(&trace_url)?,
    };
    let (cert_vm, cert_volume, cert_mount_path) = build_certificate_volumes(cert_secret_name);

    let service_account = Some(env::var("POD_SVC_ACCOUNT")?);

    Ok(batchv1::Job {
        metadata: build_object_meta(&ctx.driver_ns, &ctx.driver_name, &ctx.name, owner)?,
        spec: Some(batchv1::JobSpec {
            backoff_limit: Some(0),
            template: corev1::PodTemplateSpec {
                spec: Some(corev1::PodSpec {
                    containers: vec![corev1::Container {
                        name: "driver".into(),
                        command: Some(vec!["/sk-driver".into()]),
                        args: Some(build_driver_args(ctx, cert_mount_path, trace_mount_path)),
                        image: Some(ctx.opts.driver_image.clone()),
                        env: Some(vec![corev1::EnvVar {
                            name: "RUST_BACKTRACE".into(),
                            value: Some("1".into()),
                            ..Default::default()
                        }]),
                        volume_mounts: Some(vec![trace_vm, cert_vm]),
                        ..Default::default()
                    }],
                    restart_policy: Some("Never".into()),
                    volumes: Some(vec![trace_volume, cert_volume]),
                    service_account,
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn build_driver_args(ctx: &SimulationContext, cert_mount_path: String, trace_mount_path: String) -> Vec<String> {
    vec![
        "--cert-path".into(),
        format!("{cert_mount_path}/tls.crt"),
        "--key-path".into(),
        format!("{cert_mount_path}/tls.key"),
        "--trace-path".into(),
        trace_mount_path,
        "--virtual-ns-prefix".into(),
        "virtual".into(),
        "--sim-root".into(),
        ctx.root.clone(),
        "--sim-name".into(),
        ctx.name.clone(),
        "--verbosity".into(),
        ctx.opts.verbosity.clone(),
    ]
}

fn build_certificate_volumes(cert_secret_name: &str) -> (corev1::VolumeMount, corev1::Volume, String) {
    (
        corev1::VolumeMount {
            name: DRIVER_CERT_VOLUME.into(),
            mount_path: "/etc/ssl/".into(),
            ..Default::default()
        },
        corev1::Volume {
            name: DRIVER_CERT_VOLUME.into(),
            secret: Some(corev1::SecretVolumeSource {
                secret_name: Some(cert_secret_name.into()),
                default_mode: Some(0o600),
                ..Default::default()
            }),
            ..Default::default()
        },
        "/etc/ssl/".into(),
    )
}
