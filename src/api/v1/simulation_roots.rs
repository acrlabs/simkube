use kube::CustomResource;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Clone, CustomResource, Debug, Deserialize, JsonSchema, Serialize)]
#[kube(group = "simkube.io", version = "v1", kind = "SimulationRoot")]
#[kube(shortname = "simroot", shortname = "simroots")]
pub struct SimulationRootSpec {}
