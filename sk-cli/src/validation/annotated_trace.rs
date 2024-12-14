use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't
use std::slice;

use json_patch_ext::prelude::*;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
};

use super::validator::{
    Validator,
    ValidatorCode,
};

#[derive(Clone, Debug)]
pub enum PatchLocations {
    Everywhere,
    #[allow(dead_code)]
    ObjectReference(TypeMeta, String),
}

#[derive(Clone, Debug)]
pub struct AnnotatedTracePatch {
    pub locations: PatchLocations,
    pub ops: Vec<PatchOperation>,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub code: ValidatorCode,
    // The annotation applies to a particular object within an event; the patches
    // vector is a list of _possible_ (and probably mutually-exclusive) patches
    // that we can apply to the object that will fix the validation issue.  The first
    // patch in this list is the "recommended" fix, in that this is the one that will
    // be applied by `skctl validate check --fix`
    pub patches: Vec<AnnotatedTracePatch>,
}

#[derive(Clone, Debug, Default)]
pub struct AnnotatedTraceEvent {
    pub data: TraceEvent,
    // The annotations map is from "object index" to a list of problems/annotations
    // that apply at that specific index (remember that the "object index" is interpreted
    // as an applied object if it is less than data.applied_objs.len(), and as a deleted
    // object otherwise.
    pub annotations: BTreeMap<usize, Vec<Annotation>>,
}

impl AnnotatedTraceEvent {
    pub fn new(data: TraceEvent) -> AnnotatedTraceEvent {
        AnnotatedTraceEvent { data, ..Default::default() }
    }

    pub fn clear_annotations(&mut self) {
        self.annotations.clear();
    }
}

type AnnotationSummary = BTreeMap<ValidatorCode, usize>;

#[derive(Default)]
pub struct AnnotatedTrace {
    path: String,
    base: TraceStore,
    patches: Vec<AnnotatedTracePatch>,

    events: Vec<AnnotatedTraceEvent>,
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

    pub fn apply_patch(&mut self, patch: AnnotatedTracePatch) -> anyhow::Result<usize> {
        let mut count = 0;
        for event in self.events.iter_mut() {
            for obj in event.data.applied_objs.iter_mut().chain(event.data.deleted_objs.iter_mut()) {
                let should_apply_here = match patch.locations {
                    PatchLocations::Everywhere => true,
                    PatchLocations::ObjectReference(ref type_, ref ns_name) => {
                        obj.types.as_ref().is_some_and(|t| t == type_) && &obj.namespaced_name() == ns_name
                    },
                };

                if should_apply_here {
                    count += 1;
                    for op in &patch.ops {
                        patch_ext(&mut obj.data, op.clone())?;
                    }
                }
            }
        }
        self.patches.push(patch);

        Ok(count)
    }

    pub fn export(&self) -> anyhow::Result<Vec<u8>> {
        let trace = self
            .base
            .clone_with_events(self.events.iter().map(|a_event| a_event.data.clone()).collect());
        trace.export_all()
    }

    pub fn validate(&mut self, validators: &BTreeMap<ValidatorCode, Validator>) -> anyhow::Result<AnnotationSummary> {
        let mut summary = BTreeMap::new();
        for event in self.events.iter_mut() {
            event.clear_annotations();
            for (code, validator) in validators.iter() {
                let event_patches = validator.check_next_event(event)?;
                let count = event_patches.len();
                summary.entry(*code).and_modify(|e| *e += count).or_insert(count);

                for (i, obj_patches) in event_patches {
                    event
                        .annotations
                        .entry(i)
                        .or_insert(vec![])
                        .push(Annotation { code: *code, patches: obj_patches });
                }
            }
        }
        Ok(summary)
    }

    pub fn get_event(&self, idx: usize) -> Option<&AnnotatedTraceEvent> {
        self.events.get(idx)
    }

    pub fn get_next_annotation(&self) -> Option<&Annotation> {
        for event in &self.events {
            for annotation_list in event.annotations.values() {
                if let Some(annotation) = annotation_list.first() {
                    return Some(annotation);
                }
            }
        }
        None
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
}

#[cfg(test)]
impl AnnotatedTrace {
    pub fn new_with_events(events: Vec<AnnotatedTraceEvent>) -> AnnotatedTrace {
        AnnotatedTrace { events, ..Default::default() }
    }
}
