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
    pub metric_query_configmap: String,
    pub monitoring_namespace: Option<String>,
    pub prometheus_service_account: Option<String>,
    pub trace: String,
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
    pub fn monitoring_ns(&self) -> String {
        self.spec.monitoring_namespace.clone().unwrap_or(DEFAULT_MONITORING_NS.into())
    }

    pub fn prom_svc_account(&self) -> String {
        self.spec
            .prometheus_service_account
            .clone()
            .unwrap_or(DEFAULT_PROM_SVC_ACCOUNT.into())
    }
}
