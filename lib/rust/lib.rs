#![allow(clippy::needless_return)]

mod config;
mod constants;
mod error;
pub mod trace;
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
    pub driver_namespace: String,
    pub trace: String,
}

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "simkube.io", version = "v1alpha1", kind = "SimulationRoot")]
pub struct SimulationRootSpec {}

pub mod prelude {
    pub use super::{
        Simulation,
        SimulationRoot,
        SimulationRootSpec,
        SimulationSpec,
    };
    pub use crate::config::*;
    pub use crate::constants::*;
    pub use crate::error::{
        SimKubeError,
        SimKubeResult,
    };
}
