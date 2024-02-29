use chrono::{
    DateTime,
    Utc,
};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::metrics::api::prometheus::PrometheusRemoteWrite;
use crate::prelude::*;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum SimulationState {
    Blocked,
    Initializing,
    Finished,
    Failed,
    Retrying,
    Running,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationMetricsConfig {
    pub namespace: Option<String>,
    pub service_account: Option<String>,
    pub remote_write_configs: Vec<PrometheusRemoteWrite>,
}

#[derive(Clone, CustomResource, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[kube(group = "simkube.io", version = "v1", kind = "Simulation")]
#[kube(shortname = "sim", shortname = "sims")]
#[kube(status = "SimulationStatus")]
#[kube(
    printcolumn = r#"{"name":"start time", "type":"string", "description":"simulation driver start time", "jsonPath":".status.startTime"}"#,
    printcolumn = r#"{"name":"end time", "type":"string", "description":"simulation driver end time", "jsonPath":".status.endTime"}"#,
    printcolumn = r#"{"name":"state", "type":"string", "description":"simulation state", "jsonPath":".status.state"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SimulationSpec {
    pub driver_namespace: String,
    pub metrics_config: Option<SimulationMetricsConfig>,
    pub duration: Option<String>,
    pub trace_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationStatus {
    pub observed_generation: i64,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub state: Option<SimulationState>,
}

impl Simulation {
    pub fn metrics_ns(&self) -> String {
        match &self.spec.metrics_config {
            Some(SimulationMetricsConfig { namespace: Some(ns), .. }) => ns.clone(),
            _ => DEFAULT_METRICS_NS.into(),
        }
    }

    pub fn metrics_svc_account(&self) -> String {
        match &self.spec.metrics_config {
            Some(SimulationMetricsConfig { service_account: Some(sa), .. }) => sa.clone(),
            _ => DEFAULT_METRICS_SVC_ACCOUNT.into(),
        }
    }
}
