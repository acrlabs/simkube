use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::constants::*;

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
pub struct SimulationStatus {}
