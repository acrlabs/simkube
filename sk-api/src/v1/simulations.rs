use clockabilly::{
    DateTime,
    Utc,
};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::prometheus::PrometheusRemoteWrite;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub enum SimulationState {
    Blocked,
    Initializing,
    Finished,
    Failed,
    Paused,
    Retrying,
    Running,
}


#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationDriverConfig {
    pub args: Option<Vec<String>>,
    pub image: String,
    pub namespace: String,
    pub port: i32,
    pub secrets: Option<Vec<String>>,
    pub trace_path: String,
    #[serde(default = "default_ns_prefix")]
    pub virtual_ns_prefix: String,
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

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationHook {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub send_sim: Option<bool>,
    pub ignore_failure: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
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
    printcolumn = r#"{"name":"speed factor", "type":"number", "description":"multiplicative speed factor for the simulations", "jsonPath":".spec.speed"}"#,
    printcolumn = r#"{"name":"completed", "type":"integer", "description":"number of completed simulation runs", "jsonPath":".status.completedRuns"}"#,
    printcolumn = r#"{"name":"total", "type":"integer", "description":"total number of simulation runs", "jsonPath":".spec.repetitions"}"#,
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
    pub speed: Option<f64>,
    pub paused_time: Option<DateTime<Utc>>,
    pub hooks: Option<SimulationHooksConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationStatus {
    pub observed_generation: i64,

    pub state: Option<SimulationState>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub completed_runs: Option<u64>,
}

impl Simulation {
    pub fn speed(&self) -> f64 {
        self.spec.speed.unwrap_or(1.0)
    }
}

fn default_ns_prefix() -> String {
    "virtual".into()
}
