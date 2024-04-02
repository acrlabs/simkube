use kube::{
    CustomResource,
    ResourceExt,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use super::Simulation;
use crate::k8s::build_global_object_meta;

#[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, Serialize)]
#[kube(group = "simkube.io", version = "v1", kind = "SimulationRoot")]
#[kube(shortname = "simroot", shortname = "simroots")]
pub struct SimulationRootSpec {}

pub fn build_simulation_root(name: &str, sim: &Simulation) -> SimulationRoot {
    let owner = sim;
    SimulationRoot {
        metadata: build_global_object_meta(name, &sim.name_any(), owner),
        spec: SimulationRootSpec {},
    }
}
