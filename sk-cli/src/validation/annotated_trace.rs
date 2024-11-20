use std::collections::{
    btree_map,
    BTreeMap,
};
use std::slice;

use kube::api::DynamicObject;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
};

use super::validator::{
    Validator,
    ValidatorCode,
};

#[derive(Clone, Default)]
pub struct AnnotatedTraceEvent {
    pub data: TraceEvent,
    pub annotations: Vec<Vec<ValidatorCode>>,
}

impl AnnotatedTraceEvent {
    pub fn new(data: TraceEvent) -> AnnotatedTraceEvent {
        let annotations = vec![vec![]; data.len()];

        AnnotatedTraceEvent { data, annotations }
    }
}

#[derive(Default)]
pub struct AnnotatedTrace {
    #[allow(dead_code)]
    base: TraceStore,
    path: String,
    events: Vec<AnnotatedTraceEvent>,
    summary: BTreeMap<ValidatorCode, usize>,
}

impl AnnotatedTrace {
    pub async fn new(trace_path: &str) -> anyhow::Result<AnnotatedTrace> {
        let object_store = SkObjectStore::new(trace_path)?;
        let trace_data = object_store.get().await?.to_vec();
        let base = TraceStore::import(trace_data, &None)?;
        let events = base.iter().map(|(event, _)| AnnotatedTraceEvent::new(event.clone())).collect();
        Ok(AnnotatedTrace {
            base,
            events,
            path: trace_path.into(),
            ..Default::default()
        })
    }

    pub fn validate(&mut self, validators: &mut BTreeMap<ValidatorCode, Validator>) {
        for event in self.events.iter_mut() {
            for (code, validator) in validators.iter_mut() {
                let affected_indices = validator.check_next_event(event);
                let count = affected_indices.len();
                self.summary.entry(*code).and_modify(|e| *e += count).or_insert(count);

                for i in affected_indices {
                    event.annotations[i].push(*code);
                }
            }
        }
    }

    pub fn get_event(&self, idx: usize) -> Option<&AnnotatedTraceEvent> {
        self.events.get(idx)
    }

    pub fn get_object(&self, event_idx: usize, obj_idx: usize) -> Option<&DynamicObject> {
        let event = self.events.get(event_idx)?;
        let applied_len = event.data.applied_objs.len();
        if obj_idx >= applied_len {
            event.data.deleted_objs.get(obj_idx - applied_len)
        } else {
            event.data.applied_objs.get(obj_idx)
        }
    }

    pub fn is_empty_at(&self, idx: usize) -> bool {
        self.events
            .get(idx)
            .map(|evt| evt.data.applied_objs.is_empty() && evt.data.deleted_objs.is_empty())
            .unwrap_or(true)
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

    pub fn summary_iter(&self) -> btree_map::Iter<'_, ValidatorCode, usize> {
        self.summary.iter()
    }
}

#[cfg(test)]
impl AnnotatedTrace {
    pub fn new_with_events(events: Vec<AnnotatedTraceEvent>) -> AnnotatedTrace {
        AnnotatedTrace { events, ..Default::default() }
    }

    pub fn summary_for(&self, code: &ValidatorCode) -> Option<usize> {
        self.summary.get(code).cloned()
    }
}
