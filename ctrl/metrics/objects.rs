use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::ResourceExt;
use simkube::k8s::build_object_meta;
use simkube::macros::*;
use simkube::metrics::api::prometheus::{
    PrometheusPodMetadata,
    PrometheusRemoteWriteWriteRelabelConfigs as WriteRelabelConfigs,
};
use simkube::metrics::api::servicemonitor::{
    ServiceMonitorEndpoints,
    ServiceMonitorEndpointsMetricRelabelings,
    ServiceMonitorEndpointsMetricRelabelingsAction,
    ServiceMonitorEndpointsRelabelings,
    ServiceMonitorEndpointsRelabelingsAction,
    ServiceMonitorEndpointsScheme,
    ServiceMonitorEndpointsTlsConfig,
    ServiceMonitorSpec,
};
use simkube::metrics::api::*;
use simkube::prelude::*;

const METRICS_NAME_LABEL: &str = "__name__";
const SIMKUBE_META_LABEL: &str = "simkube_meta";
const PROM_COMPONENT_LABEL: &str = "prometheus";
const PROM_VERSION: &str = "2.44.0";
pub(super) const SCRAPE_INTERVAL_SECS: f64 = 1.0;
pub(super) const PROM_PORT: i32 = 9090;

pub fn build_ksm_service_monitor(name: &str, sim: &Simulation) -> anyhow::Result<ServiceMonitor> {
    // This object is just copy-pasta with minor modifications from the output of
    //
    // `kubectl describe servicemonitors kube-state-metrics`
    //
    // with the interval and scrape_timeout changed.  We may need to
    // adjust this more in the future.
    let scrape_interval_str = format!("{SCRAPE_INTERVAL_SECS:.0}s");
    Ok(ServiceMonitor {
        metadata: build_object_meta(&sim.metrics_ns(), name, &sim.name_any(), sim)?,
        spec: ServiceMonitorSpec {
            endpoints: vec![ServiceMonitorEndpoints {
                bearer_token_file: Some("/var/run/secrets/kubernetes.io/serviceaccount/token".into()),
                honor_labels: Some(true),
                interval: Some(scrape_interval_str.clone()),
                metric_relabelings: Some(vec![ServiceMonitorEndpointsMetricRelabelings {
                    action: Some(ServiceMonitorEndpointsMetricRelabelingsAction::Drop),
                    regex: Some("kube_endpoint_address_not_ready|kube_endpoint_address_available".into()),
                    source_labels: Some(vec!["__name__".into()]),
                    ..Default::default()
                }]),
                port: Some("https-main".into()),
                relabelings: Some(vec![ServiceMonitorEndpointsRelabelings {
                    action: Some(ServiceMonitorEndpointsRelabelingsAction::LabelDrop),
                    regex: Some("(pod|service|endpoint|namespace)".into()),
                    ..Default::default()
                }]),
                scheme: Some(ServiceMonitorEndpointsScheme::Https),
                scrape_timeout: Some(scrape_interval_str),
                tls_config: Some(ServiceMonitorEndpointsTlsConfig {
                    insecure_skip_verify: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            job_label: Some(APP_KUBERNETES_IO_NAME_KEY.into()),
            selector: metav1::LabelSelector {
                match_labels: klabel!(APP_KUBERNETES_IO_NAME_KEY => "kube-state-metrics"),
                ..Default::default()
            },
            ..Default::default()
        },
    })
}

pub fn build_prometheus(name: &str, svc_mon_selector: &str, sim: &Simulation) -> anyhow::Result<Prometheus> {
    // Configure the remote write endpoints; these _can_ be overridden by the user but set up some
    // sane defaults so they don't have to.
    let remote_write = sim.spec.metrics_config.as_ref().map(|cfg| {
        let mut rw_cfgs = cfg.remote_write_configs.clone();
        for rw in rw_cfgs.iter_mut() {
            rw.send_exemplars.get_or_insert(false);
            rw.send_native_histograms.get_or_insert(false);
            rw.remote_timeout.get_or_insert("30s".into());

            // Every metric we write should have the simkube_meta label on it for easy filtering
            rw.write_relabel_configs.get_or_insert(vec![]).push(WriteRelabelConfigs {
                source_labels: Some(vec![METRICS_NAME_LABEL.into()]), // match every metric
                target_label: Some(SIMKUBE_META_LABEL.into()),
                replacement: Some(sim.name_any()),
                ..Default::default()
            });
        }
        rw_cfgs
    });

    Ok(Prometheus {
        metadata: build_object_meta(&sim.metrics_ns(), name, &sim.name_any(), sim)?,
        spec: PrometheusSpec {
            image: Some(format!("quay.io/prometheus/prometheus:v{}", PROM_VERSION)),
            pod_metadata: Some(PrometheusPodMetadata {
                labels: klabel!(
                    SIMULATION_LABEL_KEY => sim.name_any(),
                    APP_KUBERNETES_IO_COMPONENT_KEY => PROM_COMPONENT_LABEL,
                ),
                ..Default::default()
            }),
            remote_write,
            service_monitor_selector: Some(metav1::LabelSelector {
                match_labels: klabel!(APP_KUBERNETES_IO_NAME_KEY => svc_mon_selector),
                ..Default::default()
            }),
            service_account_name: Some(sim.metrics_svc_account()),
            version: Some(PROM_VERSION.into()),
            ..Default::default()
        },
        status: Default::default(),
    })
}

pub fn build_prometheus_service(name: &str, sim: &Simulation) -> anyhow::Result<corev1::Service> {
    Ok(corev1::Service {
        metadata: build_object_meta(&sim.metrics_ns(), name, &sim.name_any(), sim)?,
        spec: Some(corev1::ServiceSpec {
            ports: Some(vec![corev1::ServicePort {
                port: PROM_PORT,
                target_port: Some(IntOrString::Int(PROM_PORT)),
                ..Default::default()
            }]),
            selector: klabel!(
                SIMULATION_LABEL_KEY => sim.name_any(),
                APP_KUBERNETES_IO_COMPONENT_KEY => PROM_COMPONENT_LABEL,
            ),
            ..Default::default()
        }),
        ..Default::default()
    })
}
