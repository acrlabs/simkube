use std::collections::HashMap;
use std::fs::File;

use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::GVK;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedObject {
    pub pod_spec_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerConfig {
    pub tracked_objects: HashMap<GVK, TrackedObject>,
}

impl TracerConfig {
    pub fn load(filename: &str) -> anyhow::Result<TracerConfig> {
        Ok(serde_yaml::from_reader(File::open(filename)?)?)
    }
}
