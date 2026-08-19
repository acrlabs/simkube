use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::PodLifecycleData;

#[derive(Clone, Debug, Deserialize, Eq, Serialize, PartialEq)]
pub struct PodSimData {
    pub lifecycle: PodLifecycleData,
}

impl PodSimData {
    pub fn new(lifecycle: PodLifecycleData) -> Self {
        Self { lifecycle }
    }

    pub fn bound_start_ts(self, start_ts: i64) -> Self {
        Self { lifecycle: self.lifecycle.bound_start_ts(start_ts) }
    }

    pub fn overlaps(&self, start_ts: i64, end_ts: i64) -> bool {
        self.lifecycle.overlaps(start_ts, end_ts)
    }
}
