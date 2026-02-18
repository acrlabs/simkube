use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::anyhow;
use metrics::{
    Counter,
    Gauge,
    Histogram,
    Key,
    KeyName,
    Metadata,
    Recorder,
    SharedString,
    Unit,
    set_global_recorder,
};
use metrics_util::registry::{
    AtomicStorage,
    Registry,
};

#[derive(Clone)]
pub struct MemoryRecorder {
    registry: Arc<Registry<Key, AtomicStorage>>,
}

impl MemoryRecorder {
    pub fn new() -> anyhow::Result<Self> {
        let recorder = MemoryRecorder { registry: Arc::new(Registry::atomic()) };
        set_global_recorder(recorder.clone())?;
        Ok(recorder)
    }

    pub fn get_counter(&self, key: &Key) -> anyhow::Result<u64> {
        self.registry
            .get_counter(key)
            .map(|v| v.load(Ordering::Relaxed))
            .ok_or(anyhow!("no counter with key {key}"))
    }

    pub fn get_gauge(&self, key: &Key) -> anyhow::Result<f64> {
        self.registry
            .get_gauge(key)
            .map(|v| f64::from_bits(v.load(Ordering::Relaxed)))
            .ok_or(anyhow!("no gauge with key {key}"))
    }

    pub fn get_histogram(&self, _key: &Key) -> anyhow::Result<Histogram> {
        unimplemented!();
    }
}

impl Recorder for MemoryRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
        self.registry.get_or_create_counter(key, |c| Counter::from_arc(c.clone()))
    }

    fn register_gauge(&self, key: &Key, _metadata: &Metadata<'_>) -> Gauge {
        self.registry.get_or_create_gauge(key, |g| Gauge::from_arc(g.clone()))
    }

    fn register_histogram(&self, key: &Key, _metadata: &Metadata<'_>) -> Histogram {
        self.registry.get_or_create_histogram(key, |h| Histogram::from_arc(h.clone()))
    }
}
