use std::collections::{
    HashMap,
    HashSet,
    VecDeque,
};
use std::mem::take;
use std::sync::{
    Arc,
    Mutex,
};

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use serde::{
    Deserialize,
    Serialize,
};
use tracing::*;

use super::trace_filter::filter_event;
use super::*;
use crate::config::TracerConfig;
use crate::jsonutils;
use crate::k8s::{
    make_deletable,
    KubeResourceExt,
    PodLifecycleData,
};

#[derive(Debug)]
enum TraceAction {
    ObjectApplied,
    ObjectDeleted,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TraceEvent {
    pub ts: i64,
    pub applied_objs: Vec<DynamicObject>,
    pub deleted_objs: Vec<DynamicObject>,
}

#[derive(Default)]
pub struct Tracer {
    pub(super) config: TracerConfig,
    pub(super) events: VecDeque<TraceEvent>,
    pub(super) _pod_owners: OwnedPodMap,
    pub(super) index: HashMap<String, u64>,
}

impl Tracer {
    pub fn new(config: TracerConfig) -> Arc<Mutex<Tracer>> {
        Arc::new(Mutex::new(Tracer { config, ..Default::default() }))
    }

    pub fn import(data: Vec<u8>) -> anyhow::Result<Tracer> {
        let (config, events): (TracerConfig, VecDeque<TraceEvent>) = rmp_serde::from_slice(&data)?;

        let mut tracer = Tracer { config, events, ..Default::default() };
        let (_, index) = tracer.collect_events(0, i64::MAX, &TraceFilter::blank());
        tracer.index = index;

        Ok(tracer)
    }

    pub fn config(&self) -> &TracerConfig {
        &self.config
    }

    pub fn export(&self, start_ts: i64, end_ts: i64, filter: &TraceFilter) -> anyhow::Result<Vec<u8>> {
        info!("Exporting objs with filters: {:?}", filter);
        let (events, _) = self.collect_events(start_ts, end_ts, filter);
        let data = rmp_serde::to_vec_named(&(&self.config, &events))?;

        info!("Exported {} events.", events.len());
        Ok(data)
    }

    pub fn objs(&self) -> HashSet<String> {
        self.index.keys().cloned().collect()
    }

    pub fn objs_at(&self, end_ts: i64, filter: &TraceFilter) -> HashSet<String> {
        let (_, index) = self.collect_events(0, end_ts, filter);
        index.keys().cloned().collect()
    }

    pub fn start_ts(&self) -> Option<i64> {
        match self.iter().next() {
            Some((_, Some(ts))) => Some(ts),
            _ => None,
        }
    }

    pub(crate) fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) {
        let ns_name = obj.namespaced_name();
        let new_hash = jsonutils::hash(obj.data.get("spec"));
        let old_hash = if maybe_old_hash.is_some() { maybe_old_hash } else { self.index.get(&ns_name).cloned() };

        if Some(new_hash) != old_hash {
            self.append_event(ts, obj, TraceAction::ObjectApplied);
        }
        self.index.insert(ns_name, new_hash);
    }

    pub(crate) fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) {
        let ns_name = obj.namespaced_name();
        self.append_event(ts, obj, TraceAction::ObjectDeleted);
        self.index.remove(&ns_name);
    }

    pub(crate) fn update_all_objs(&mut self, objs: &Vec<DynamicObject>, ts: i64) {
        let mut old_index = take(&mut self.index);
        for obj in objs {
            let ns_name = obj.namespaced_name();
            let old_hash = old_index.remove(&ns_name);
            self.create_or_update_obj(obj, ts, old_hash);
        }

        for ns_name in old_index.keys() {
            self.delete_obj(&make_deletable(ns_name), ts);
        }
    }

    pub(crate) fn record_pod_lifecycle(
        &mut self,
        _ns_name: &str,
        _owners: Vec<metav1::OwnerReference>,
        _lifecycle_data: &PodLifecycleData,
    ) {
    }

    fn append_event(&mut self, ts: i64, obj: &DynamicObject, action: TraceAction) {
        info!("{} - {:?} @ {}", obj.namespaced_name(), action, ts);

        let obj = obj.clone();
        match self.events.back_mut() {
            Some(evt) if evt.ts == ts => match action {
                TraceAction::ObjectApplied => evt.applied_objs.push(obj),
                TraceAction::ObjectDeleted => evt.deleted_objs.push(obj),
            },
            _ => {
                let evt = match action {
                    TraceAction::ObjectApplied => TraceEvent { ts, applied_objs: vec![obj], ..Default::default() },
                    TraceAction::ObjectDeleted => TraceEvent { ts, deleted_objs: vec![obj], ..Default::default() },
                };
                self.events.push_back(evt);
            },
        }
    }

    fn collect_events(
        &self,
        start_ts: i64,
        end_ts: i64,
        filter: &TraceFilter,
    ) -> (Vec<TraceEvent>, HashMap<String, u64>) {
        let mut events = vec![TraceEvent { ts: start_ts, ..Default::default() }];
        let mut flattened_objects = HashMap::new();
        let mut index = HashMap::new();
        for (evt, _) in self.iter() {
            // trace should be end-exclusive, so we use >= here: anything that is at the
            // end_ts or greater gets discarded.  The event list is stored in
            // monotonically-increasing order so we are safe to break here.
            if evt.ts >= end_ts {
                break;
            }

            if let Some(new_evt) = filter_event(&evt, filter) {
                for obj in &new_evt.applied_objs {
                    let ns_name = obj.namespaced_name();
                    if new_evt.ts < start_ts {
                        flattened_objects.insert(ns_name.clone(), obj.clone());
                    }
                    let hash = jsonutils::hash(obj.data.get("spec"));
                    index.insert(ns_name, hash);
                }

                for obj in &evt.deleted_objs {
                    let ns_name = obj.namespaced_name();
                    if new_evt.ts < start_ts {
                        flattened_objects.remove(&ns_name);
                    }
                    index.remove(&ns_name);
                }

                if new_evt.ts >= start_ts {
                    events.push(new_evt.clone());
                }
            }
        }

        events[0].applied_objs = flattened_objects.values().cloned().collect();
        (events, index)
    }
}

pub struct TraceIterator<'a> {
    events: &'a VecDeque<TraceEvent>,
    idx: usize,
}

impl<'a> Tracer {
    pub fn iter(&'a self) -> TraceIterator<'a> {
        TraceIterator { events: &self.events, idx: 0 }
    }
}

impl<'a> Iterator for TraceIterator<'a> {
    type Item = (TraceEvent, Option<i64>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.events.is_empty() {
            return None;
        }

        let ret = match self.idx {
            i if i < self.events.len() - 1 => Some((self.events[i].clone(), Some(self.events[i + 1].ts))),
            i if i == self.events.len() - 1 => Some((self.events[i].clone(), None)),
            _ => None,
        };

        self.idx += 1;
        ret
    }
}
