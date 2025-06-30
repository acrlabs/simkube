// The annotated trace code is actually fairly complex; we need to take a trace (which already has
// several layers of nesting) and also append annotations about which events and which objects
// within those events are failing validation.
//
// The structure of an annotated trace is as follows:
//    - path (read-only): the location of the original trace
//    - base (read-only): the imported/parsed trace located at `path`; we store the base trace data
//      so that we can recompute a fixed/patched version by iteratively applying all the patches in
//      order on-demand
//    - patches (read-write): the list of patches applied to the original trace value; this will
//      allow us in the future to "undo" patches, by walking back up the list and recomputing the
//      events
//    - events (read-write): the computed list of (annotated) events after all the patches have been
//      applied
//
// In addition to the event data (timestamp, applied objects, and deleted objects), an annotated
// event contains a set of annotations indicating which objects failed some validation check.
// Throughout this code, we use the convention that the annotation index can be between 0 and
// applied_objs.len() + deleted_objs.len(), where if the index is larger than applied_objs.len(),
// you must subtract applied_objs.len() and use the resulting value to index into deleted_objs.
//
// Because each object can fail multiple different validation checks, the "value" for a particular
// object index is a vector of annotations, where an annotation contains a ValidatorCode (which can
// be used to look up more information about the failed check), together with a list of _possible_
// (and probably mutually-exclusive) patches that we can apply to the object that will fix the
// validation issue.  The first patch in this list is the "recommended" fix, in that this is the
// one that will be applied by `skctl validate check --fix`.  The others may also solve the
// problem, but are probably not what most users want; we may show these in skctl xray eventually.
//
// Lastly, an individual patch applies in one or more locations in the trace (for example, all
// deployments with a particular name), and has one or more operations (that is, json-patch
// operations) that need to be applied at that location.

use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't
use std::iter::once;
use std::slice;

use json_patch_ext::prelude::*;
use serde_json::json;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;
use sk_store::{
    TraceAction,
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
    // Everywhere applies to every object in the trace
    Everywhere,
    // ObjectReference applies to objects matching a particular type and namespaced name
    ObjectReference(TypeMeta, String),
    // InsertAt takes a particular timestamp, and inserts a new event at that timestamp with a
    // given action (apply or delete); the object is given by its TypeMeta and ObjectMeta; if you
    // need to modify its data, you should also include a json_patch object that modifies the root
    InsertAt(i64, TraceAction, TypeMeta, Box<metav1::ObjectMeta>), // objectmeta is big, use a box
}

#[derive(Clone, Debug)]
pub struct AnnotatedTracePatch {
    pub locations: PatchLocations,
    pub ops: Vec<PatchOperation>,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub code: ValidatorCode,
    pub patches: Vec<AnnotatedTracePatch>,
}

#[derive(Clone, Debug, Default)]
pub struct AnnotatedTraceEvent {
    pub data: TraceEvent,
    // Maybe overkill, but we use a BTreeMap instead of Vector so that we can easily skip over
    // objects in the trace event that don't have any annotations
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

    pub(super) events: Vec<AnnotatedTraceEvent>,
}

impl AnnotatedTrace {
    pub async fn new(trace_path: &str) -> anyhow::Result<AnnotatedTrace> {
        let object_store = SkObjectStore::new(trace_path)?;
        let trace_data = object_store.get().await?.to_vec();
        let base = TraceStore::import(trace_data, None)?;
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
        for obj in self.matched_objects(&patch.locations) {
            count += 1;
            for op in &patch.ops {
                patch_ext(&mut obj.data, op.clone())?;
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
                let event_patches = validator.check_next_event(event, self.base.config())?;
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

    fn object_iter_mut(&mut self) -> impl Iterator<Item = &mut DynamicObject> {
        self.events
            .iter_mut()
            .flat_map(|e| e.data.applied_objs.iter_mut().chain(e.data.deleted_objs.iter_mut()))
    }
}

impl<'a> AnnotatedTrace {
    fn matched_objects(
        &'a mut self,
        locations: &'a PatchLocations,
    ) -> Box<dyn Iterator<Item = &'a mut DynamicObject> + 'a> {
        match locations {
            PatchLocations::Everywhere => Box::new(self.object_iter_mut()),
            PatchLocations::ObjectReference(type_, ns_name) => Box::new(self.object_iter_mut().filter(move |obj| {
                obj.types.as_ref().is_some_and(|t| t == type_) && &obj.namespaced_name() == ns_name
            })),
            PatchLocations::InsertAt(relative_ts, action, type_meta, object_meta) => {
                let insert_ts = self.start_ts().unwrap_or_default() + relative_ts;
                let insert_idx = find_or_create_event_at_ts(&mut self.events, insert_ts);

                let new_obj = DynamicObject {
                    types: Some(type_meta.clone()),
                    metadata: *object_meta.clone(),
                    data: json!({}),
                };
                let obj = match action {
                    TraceAction::ObjectApplied => {
                        self.events[insert_idx].data.applied_objs.push(new_obj);
                        self.events[insert_idx].data.applied_objs.iter_mut().last().unwrap()
                    },
                    TraceAction::ObjectDeleted => {
                        self.events[insert_idx].data.deleted_objs.push(new_obj);
                        self.events[insert_idx].data.deleted_objs.iter_mut().last().unwrap()
                    },
                };
                Box::new(once(obj))
            },
        }
    }
}

pub(super) fn find_or_create_event_at_ts(events: &mut Vec<AnnotatedTraceEvent>, ts: i64) -> usize {
    let new_event = AnnotatedTraceEvent {
        data: TraceEvent { ts, ..Default::default() },
        ..Default::default()
    };
    // Walk through the events list backwards until we find the first one less than the given ts
    match events.iter().rposition(|e| e.data.ts <= ts) {
        Some(i) => {
            // If we found one, and the ts isn't equal, create an event with the specified
            // timestamp; this goes at index i+1 since it needs to go after the lower (found) ts
            if events[i].data.ts < ts {
                events.insert(i + 1, new_event);
                i + 1
            } else {
                i // otherwise the timestamp is equal so return this index
            }
        },
        None => {
            // In this case there are no events in the trace, so we add one at the beginning
            events.push(new_event);
            0
        },
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
impl AnnotatedTrace {
    pub fn new_with_events(events: Vec<AnnotatedTraceEvent>) -> AnnotatedTrace {
        AnnotatedTrace { events, ..Default::default() }
    }

    pub fn new_from_test_json(trace_type: &str) -> AnnotatedTrace {
        let exported_trace = sk_testutils::exported_trace_from_json(trace_type);
        let annotated_events = exported_trace
            .events()
            .iter()
            .cloned()
            .map(|e| AnnotatedTraceEvent::new(e))
            .collect();
        let base = TraceStore::from_exported_trace(exported_trace, None).unwrap();

        AnnotatedTrace {
            base,
            events: annotated_events,
            ..Default::default()
        }
    }
}
