use std::collections::BTreeMap;

use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
};

#[derive(Clone, Default)]
pub struct AnnotatedTraceEvent {
    pub data: TraceEvent,
    pub annotations: Vec<(usize, String)>,
}

#[derive(Default)]
pub struct AnnotatedTrace {
    pub base: TraceStore,
    pub path: String,
    pub events: Vec<AnnotatedTraceEvent>,
    pub summary: BTreeMap<String, usize>,
}

impl AnnotatedTrace {
    pub async fn new(trace_path: &str) -> anyhow::Result<AnnotatedTrace> {
        let object_store = SkObjectStore::new(trace_path)?;
        let trace_data = object_store.get().await?.to_vec();
        let base = TraceStore::import(trace_data, &None)?;
        let events = base
            .iter()
            .map(|(event, _)| AnnotatedTraceEvent { data: event.clone(), ..Default::default() })
            .collect();
        Ok(AnnotatedTrace {
            base,
            events,
            path: trace_path.into(),
            ..Default::default()
        })
    }
}
