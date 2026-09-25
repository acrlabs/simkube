use std::collections::HashMap;

use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::PodLifecycleData;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Serialize, PartialEq)]
pub enum MetricType {
    #[serde(rename = "cpu")]
    CPU,
    #[serde(rename = "memory")]
    Memory,
}

type MetricsData = HashMap<MetricType, Vec<f64>>;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct PodMetricsData {
    containers: HashMap<String, MetricsData>,
}

impl PodMetricsData {
    pub fn insert(&mut self, container: &str, metric_type: MetricType, value: f64) {
        self.containers
            .entry(container.into())
            .or_default()
            .entry(metric_type)
            .or_default()
            .push(value);
    }

    pub fn is_empty(&self) -> bool {
        self.containers.is_empty()
    }

    pub fn merge(&mut self, metrics_data: PodMetricsData) {
        for (container, data) in metrics_data.containers {
            let container_entry = self.containers.entry(container).or_default();
            for (metric_type, mut values) in data {
                container_entry.entry(metric_type).or_default().append(&mut values)
            }
        }
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

    pub fn merge_metrics(&mut self, metrics_data: PodMetricsData) {
        self.metrics.merge(metrics_data);
    }

    pub fn overlaps(&self, start_ts: i64, end_ts: i64) -> bool {
        self.lifecycle.overlaps(start_ts, end_ts)
    }
}
