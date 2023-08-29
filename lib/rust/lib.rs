#![allow(clippy::needless_return)]

mod constants;
pub mod error;
pub mod util;
pub mod watchertracer;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

// Our custom resources
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "simkube.io", version = "v1alpha1", kind = "Simulation")]
#[serde(rename_all = "camelCase")]
pub struct SimulationSpec {
    pub driver_image: String,
    pub driver_namespace: String,
    pub trace: String,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "simkube.io", version = "v1alpha1", kind = "SimulationRoot")]
pub struct SimulationRootSpec {}
