use clockabilly::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::prometheus::PrometheusRemoteWrite;

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
pub struct SimulationDriverConfig {
    pub namespace: String,
    pub image: String,
    pub trace_path: String,
    pub port: i32,
    pub speed: f64,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationMetricsConfig {
    pub namespace: Option<String>,
    pub service_account: Option<String>,
    pub prometheus_shards: Option<i32>,
    pub pod_monitor_names: Option<Vec<String>>,
    pub pod_monitor_namespaces: Option<Vec<String>>,
    pub service_monitor_names: Option<Vec<String>>,
    pub service_monitor_namespaces: Option<Vec<String>>,
    pub remote_write_configs: Vec<PrometheusRemoteWrite>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationHook {
    pub cmd: String,
    pub args: Vec<String>,
    pub send_sim: Option<bool>,
    pub ignore_failure: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationHooksConfig {
    pub pre_start_hooks: Option<Vec<SimulationHook>>,
    pub pre_run_hooks: Option<Vec<SimulationHook>>,
    pub post_run_hooks: Option<Vec<SimulationHook>>,
    pub post_stop_hooks: Option<Vec<SimulationHook>>,
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
    // Required fields
    pub driver: SimulationDriverConfig,

    // Optional fields
    pub metrics: Option<SimulationMetricsConfig>,
    pub duration: Option<String>,
    pub repetitions: Option<i32>,
    pub hooks: Option<SimulationHooksConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationStatus {
    pub observed_generation: i64,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub state: Option<SimulationState>,
}
