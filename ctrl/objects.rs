use std::env;

use k8s_openapi::api::admissionregistration::v1 as admissionv1;
use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::ResourceExt;
use reqwest::Url;
use simkube::k8s::{
    build_containment_label_selector,
    build_global_object_meta,
    build_object_meta,
};
use simkube::macros::*;
use simkube::metrics::api::prometheus::{
    Prometheus,
    PrometheusPodMetadata,
    PrometheusRemoteWriteWriteRelabelConfigs as WriteRelabelConfigs,
    PrometheusSpec,
};
use simkube::prelude::*;
use simkube::sim::*;
use simkube::store::storage;

use super::cert_manager::DRIVER_CERT_NAME;
use super::trace::get_local_trace_volume;
use crate::SimulationContext;

const METRICS_NAME_LABEL: &str = "__name__";
const SIMKUBE_META_LABEL: &str = "simkube_meta";
const PROM_VERSION: &str = "2.44.0";
const PROM_COMPONENT_LABEL: &str = "prometheus";
const WEBHOOK_NAME: &str = "mutatepods.simkube.io";
const DRIVER_CERT_VOLUME: &str = "driver-cert";

pub(super) fn build_driver_namespace(ctx: &SimulationContext, sim: &Simulation) -> corev1::Namespace {
    let owner = sim;
    corev1::Namespace {
        metadata: build_global_object_meta(&ctx.driver_ns, &ctx.name, owner),
        ..Default::default()
    }
}

pub(super) fn build_prometheus(name: &str, sim: &Simulation, mc: &SimulationMetricsConfig) -> Prometheus {
    // Configure the remote write endpoints; these _can_ be overridden by the user but set up some
    // sane defaults so they don't have to.
    let mut rw_cfgs = mc.remote_write_configs.clone();
    for cfg in rw_cfgs.iter_mut() {
        cfg.send_exemplars.get_or_insert(false);
        cfg.send_native_histograms.get_or_insert(false);
        cfg.remote_timeout.get_or_insert("30s".into());

        // Every metric we write should have the simkube_meta label on it for easy filtering
        cfg.write_relabel_configs.get_or_insert(vec![]).push(WriteRelabelConfigs {
            source_labels: Some(vec![METRICS_NAME_LABEL.into()]), // match every metric
            target_label: Some(SIMKUBE_META_LABEL.into()),
            replacement: Some(sim.name_any()),
            ..Default::default()
        });
    }


    let shards = mc.prometheus_shards.or(Some(1));
    let pod_monitor_namespace_selector =
        Some(mc.pod_monitor_namespaces.clone().map_or(Default::default(), |name| {
            build_containment_label_selector(KUBERNETES_IO_METADATA_NAME_KEY, name)
        }));
    let pod_monitor_selector = Some(
        mc.pod_monitor_names
            .clone()
            .map_or(Default::default(), |name| build_containment_label_selector(APP_KUBERNETES_IO_NAME_KEY, name)),
    );
    let service_monitor_namespace_selector =
        Some(mc.service_monitor_namespaces.clone().map_or(Default::default(), |name| {
            build_containment_label_selector(KUBERNETES_IO_METADATA_NAME_KEY, name)
        }));
    let service_monitor_selector = Some(
        mc.service_monitor_names
            .clone()
            .map_or(Default::default(), |name| build_containment_label_selector(APP_KUBERNETES_IO_NAME_KEY, name)),
    );

    let owner = sim;
    Prometheus {
        metadata: build_object_meta(&metrics_ns(sim), name, &sim.name_any(), owner),
        spec: PrometheusSpec {
            image: Some(format!("quay.io/prometheus/prometheus:v{}", PROM_VERSION)),
            pod_metadata: Some(PrometheusPodMetadata {
                labels: klabel!(
                    SIMULATION_LABEL_KEY => sim.name_any(),
                    APP_KUBERNETES_IO_COMPONENT_KEY => PROM_COMPONENT_LABEL,
                ),
                ..Default::default()
            }),
            external_labels: klabel!(PROM2PARQUET_PREFIX_KEY => sim.name_any()),
            shards,
            pod_monitor_namespace_selector,
            pod_monitor_selector,
            service_monitor_namespace_selector,
            service_monitor_selector,
            remote_write: Some(rw_cfgs),
            service_account_name: Some(metrics_svc_account(sim)),
            version: Some(PROM_VERSION.into()),
            ..Default::default()
        },
        status: Default::default(),
    }
}

pub(super) fn build_mutating_webhook(
    ctx: &SimulationContext,
    metaroot: &SimulationRoot,
) -> admissionv1::MutatingWebhookConfiguration {
    let owner = metaroot;
    let mut metadata = build_global_object_meta(&ctx.webhook_name, &ctx.name, owner);
    if ctx.opts.use_cert_manager {
        metadata
            .annotations
            .get_or_insert(BTreeMap::new())
            .insert("cert-manager.io/inject-ca-from".into(), format!("{}/{}", ctx.driver_ns, DRIVER_CERT_NAME));
    }

    admissionv1::MutatingWebhookConfiguration {
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
    }
}

pub(super) fn build_driver_service(ctx: &SimulationContext, metaroot: &SimulationRoot) -> corev1::Service {
    let owner = metaroot;
    corev1::Service {
        metadata: build_object_meta(&ctx.driver_ns, &ctx.driver_svc, &ctx.name, owner),
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
    }
}

pub(super) fn build_driver_job(
    ctx: &SimulationContext,
    sim: &Simulation,
    cert_secret_name: &str,
    ctrl_ns: &str,
) -> anyhow::Result<batchv1::Job> {
    let trace_url = Url::parse(&sim.spec.trace_path)?;
    let (trace_vm, trace_volume, trace_mount_path) = match storage::get_scheme(&trace_url)? {
        storage::Scheme::AmazonS3 => todo!(),
        storage::Scheme::Local => get_local_trace_volume(&trace_url)?,
    };
    let (cert_vm, cert_volume, cert_mount_path) = build_certificate_volumes(cert_secret_name);

    let service_account = Some(env::var(POD_SVC_ACCOUNT_ENV_VAR)?);

    Ok(batchv1::Job {
        metadata: build_object_meta(&ctx.driver_ns, &ctx.driver_name, &ctx.name, sim),
        spec: Some(batchv1::JobSpec {
            backoff_limit: Some(0),
            template: corev1::PodTemplateSpec {
                spec: Some(corev1::PodSpec {
                    containers: vec![corev1::Container {
                        name: "driver".into(),
                        command: Some(vec!["/sk-driver".into()]),
                        args: Some(build_driver_args(ctx, cert_mount_path, trace_mount_path, ctrl_ns.into())),
                        image: Some(ctx.opts.driver_image.clone()),
                        env: Some(vec![
                            corev1::EnvVar {
                                name: "RUST_BACKTRACE".into(),
                                value: Some("1".into()),
                                ..Default::default()
                            },
                            corev1::EnvVar {
                                name: DRIVER_NAME_ENV_VAR.into(),
                                value_from: Some(corev1::EnvVarSource {
                                    field_ref: Some(corev1::ObjectFieldSelector {
                                        field_path: "metadata.name".into(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
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
            parallelism: Some(1),
            completions: sim.spec.repetitions,
            ..Default::default()
        }),
        ..Default::default()
    })
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

fn build_driver_args(
    ctx: &SimulationContext,
    cert_mount_path: String,
    trace_mount_path: String,
    ctrl_ns: String,
) -> Vec<String> {
    vec![
        "--cert-path".into(),
        format!("{cert_mount_path}/tls.crt"),
        "--key-path".into(),
        format!("{cert_mount_path}/tls.key"),
        "--trace-mount-path".into(),
        trace_mount_path,
        "--virtual-ns-prefix".into(),
        "virtual".into(),
        "--sim-name".into(),
        ctx.name.clone(),
        "--verbosity".into(),
        ctx.opts.verbosity.clone(),
        "--controller-ns".into(),
        ctrl_ns,
    ]
}
