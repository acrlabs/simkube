use std::collections::HashMap;
use std::fs::File;
use std::ops::Not;

use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::GVK;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedObjectConfig {
    pub pod_spec_template_path: String,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub track_lifecycle: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerConfig {
    pub tracked_objects: HashMap<GVK, TrackedObjectConfig>,
}

impl TracerConfig {
    pub fn load(filename: &str) -> anyhow::Result<TracerConfig> {
        Ok(serde_yaml::from_reader(File::open(filename)?)?)
    }

    pub fn pod_spec_template_path(&self, gvk: &GVK) -> Option<&str> {
        Some(&self.tracked_objects.get(gvk)?.pod_spec_template_path)
    }

    pub fn track_lifecycle_for(&self, gvk: &GVK) -> bool {
        self.tracked_objects.get(gvk).is_some_and(|obj| obj.track_lifecycle)
    }
}
