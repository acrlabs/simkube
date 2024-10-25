use std::collections::{
    btree_map,
    BTreeMap,
};
use std::ops::Index;
use std::slice;

use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
};

use super::validator::Validator;

#[derive(Clone, Default)]
pub struct AnnotatedTraceEvent {
    pub data: TraceEvent,
    pub annotations: Vec<(usize, String)>,
}

#[derive(Default)]
pub struct AnnotatedTrace {
    #[allow(dead_code)]
    base: TraceStore,
    path: String,
    events: Vec<AnnotatedTraceEvent>,
    summary: BTreeMap<String, usize>,
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

    pub fn validate(&mut self, validators: &mut BTreeMap<String, Validator>) {
        for evt in self.events.iter_mut() {
            for (code, validator) in validators.iter_mut() {
                let mut affected_indices: Vec<_> =
                    validator.check_next_event(evt).into_iter().map(|i| (i, code.clone())).collect();
                let count = affected_indices.len();
                self.summary.entry(code.into()).and_modify(|e| *e += count).or_insert(count);

                // This needs to happen at the ends, since `append` consumes affected_indices' contents
                evt.annotations.append(&mut affected_indices);
            }
        }
    }

    pub fn iter(&self) -> slice::Iter<'_, AnnotatedTraceEvent> {
        self.events.iter()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn path(&self) -> String {
        self.path.clone()
    }

    pub fn start_ts(&self) -> Option<i64> {
        self.events.first().map(|evt| evt.data.ts)
    }

    pub fn summary_iter(&self) -> btree_map::Iter<'_, String, usize> {
        self.summary.iter()
    }
}

impl Index<usize> for AnnotatedTrace {
    type Output = AnnotatedTraceEvent;

    fn index(&self, index: usize) -> &Self::Output {
        &self.events[index]
    }
}

#[cfg(test)]
impl AnnotatedTrace {
    pub fn new_with_events(events: Vec<AnnotatedTraceEvent>) -> AnnotatedTrace {
        AnnotatedTrace { events, ..Default::default() }
    }

    pub fn summary_for(&self, code: &str) -> Option<usize> {
        self.summary.get(code).cloned()
    }
}
