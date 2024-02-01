use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Clone, CustomResource, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[kube(group = "simkube.io", version = "v1", kind = "Simulation")]
#[kube(shortname = "sim", shortname = "sims")]
#[kube(status = "SimulationStatus")]
#[serde(rename_all = "camelCase")]
pub struct SimulationSpec {
    pub driver_namespace: String,
    pub trace: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct SimulationStatus {}
