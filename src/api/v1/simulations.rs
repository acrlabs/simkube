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

pub fn default_monitoring_ns() -> String {
    DEFAULT_MONITORING_NS.into()
}

pub fn default_prom_svc_acct() -> String {
    DEFAULT_PROM_SVC_ACCOUNT.into()
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
    #[serde(default = "default_monitoring_ns")]
    pub monitoring_namespace: String,
    #[serde(default = "default_prom_svc_acct")]
    pub prometheus_service_account: String,
    pub trace: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationStatus {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub state: Option<String>,
}
