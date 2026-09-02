use std::collections::HashMap;

use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::PodLifecycleData;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Serialize, PartialEq)]
pub enum MetricType {
    CPU,
    Memory,
}

type MetricsData = HashMap<MetricType, Vec<f64>>;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct PodMetricsData {
    containers: HashMap<String, MetricsData>,
}

impl PodMetricsData {
    pub fn is_empty(&self) -> bool {
        self.containers.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PodSimData {
    pub lifecycle: PodLifecycleData,

    #[serde(default, skip_serializing_if = "PodMetricsData::is_empty")]
    pub metrics: PodMetricsData,
}

impl PodSimData {
    pub fn new(lifecycle: PodLifecycleData) -> Self {
        Self { lifecycle, metrics: Default::default() }
    }

    pub fn bound_start_ts(self, start_ts: i64) -> Self {
        Self {
            lifecycle: self.lifecycle.bound_start_ts(start_ts),
            metrics: Default::default(),
        }
    }

    pub fn overlaps(&self, start_ts: i64, end_ts: i64) -> bool {
        self.lifecycle.overlaps(start_ts, end_ts)
    }
}
