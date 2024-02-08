pub mod prometheus;
pub mod servicemonitor;

use std::collections::BTreeMap;

use kube::ResourceExt;
pub use prometheus::{
    Prometheus,
    PrometheusSpec,
    PrometheusStatus,
};
pub use servicemonitor::ServiceMonitor;
use servicemonitor::{
    ServiceMonitorEndpoints,
    ServiceMonitorEndpointsMetricRelabelings,
    ServiceMonitorEndpointsMetricRelabelingsAction,
    ServiceMonitorEndpointsRelabelings,
    ServiceMonitorEndpointsRelabelingsAction,
    ServiceMonitorEndpointsScheme,
    ServiceMonitorEndpointsTlsConfig,
    ServiceMonitorSpec,
};

use crate::k8s::build_object_meta;
use crate::prelude::*;

const PROM_VERSION: &str = "2.44.0";

pub fn build_ksm_service_monitor(name: &str, sim: &Simulation) -> anyhow::Result<ServiceMonitor> {
    // This object is just copy-pasta with minor modifications from the output of
    //
    // `kubectl describe servicemonitors kube-state-metrics`
    //
    // with the interval and scrape_timeout changed.  We may need to
    // adjust this more in the future.
    let mut metadata = build_object_meta(&sim.spec.monitoring_namespace, name, &sim.name_any(), sim)?;
    metadata
        .labels
        .get_or_insert(BTreeMap::new())
        .insert(APP_KUBERNETES_IO_NAME_KEY.into(), name.into());
    Ok(ServiceMonitor {
        metadata,
        spec: ServiceMonitorSpec {
            endpoints: vec![ServiceMonitorEndpoints {
                bearer_token_file: Some("/var/run/secrets/kubernetes.io/serviceaccount/token".into()),
                honor_labels: Some(true),
                interval: Some("1s".into()),
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
                scrape_timeout: Some("1s".into()),
                tls_config: Some(ServiceMonitorEndpointsTlsConfig {
                    insecure_skip_verify: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            job_label: Some(APP_KUBERNETES_IO_NAME_KEY.into()),
            selector: metav1::LabelSelector {
                match_labels: Some(BTreeMap::from([(APP_KUBERNETES_IO_NAME_KEY.into(), "kube-state-metrics".into())])),
                ..Default::default()
            },
            ..Default::default()
        },
    })
}

pub fn build_prometheus(name: &str, svc_mon_selector: &str, sim: &Simulation) -> anyhow::Result<Prometheus> {
    Ok(Prometheus {
        metadata: build_object_meta(&sim.spec.monitoring_namespace, name, &sim.name_any(), sim)?,
        spec: PrometheusSpec {
            image: Some(format!("quay.io/prometheus/prometheus:v{}", PROM_VERSION)),
            service_monitor_selector: Some(metav1::LabelSelector {
                match_labels: Some(BTreeMap::from([(APP_KUBERNETES_IO_NAME_KEY.into(), svc_mon_selector.into())])),
                ..Default::default()
            }),
            service_account_name: Some(sim.spec.prometheus_service_account.clone()),
            version: Some(PROM_VERSION.into()),
            ..Default::default()
        },
        status: Default::default(),
    })
}
