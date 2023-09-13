use std::collections::{
    HashMap,
    HashSet,
    VecDeque,
};
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
use serde_json::value::Value;
use tracing::*;

use super::trace_filter::{
    filter_event,
    TraceFilter,
};
use crate::util::*;

#[derive(Debug)]
enum TraceAction {
    ObjectCreated,
    ObjectDeleted,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TraceEvent {
    pub ts: i64,
    pub created_objs: Vec<DynamicObject>,
    pub deleted_objs: Vec<DynamicObject>,
}

pub struct Tracer {
    pub(super) trace: VecDeque<TraceEvent>,
    pub(super) tracked_objs: HashMap<String, u64>,
    pub(super) version: u64,
}

impl Tracer {
    pub fn new() -> Arc<Mutex<Tracer>> {
        Arc::new(Mutex::new(Tracer {
            trace: VecDeque::new(),
            tracked_objs: HashMap::new(),
            version: 0,
        }))
    }

    pub fn import(data: Vec<u8>) -> anyhow::Result<Tracer> {
        let trace = rmp_serde::from_slice(&data)?;

        let mut tracer = Tracer { trace, tracked_objs: HashMap::new(), version: 0 };
        let (_, tracked_objs) = tracer.collect_events(0, i64::MAX, &TraceFilter::blank());
        tracer.tracked_objs = tracked_objs;

        Ok(tracer)
    }

    pub fn export(&self, start_ts: i64, end_ts: i64, filter: &TraceFilter) -> anyhow::Result<Vec<u8>> {
        info!("Exporting objs with filters: {:?}", filter);
        let (events, _) = self.collect_events(start_ts, end_ts, filter);
        let data = rmp_serde::to_vec_named(&events)?;

        info!("Exported {} events.", events.len());
        Ok(data)
    }

    pub fn objs(&self) -> HashSet<String> {
        return self.tracked_objs.keys().cloned().collect();
    }

    pub fn objs_at(&self, end_ts: i64, filter: &TraceFilter) -> HashSet<String> {
        let (_, tracked_objs) = self.collect_events(0, end_ts, filter);
        tracked_objs.keys().cloned().collect()
    }

    pub fn start_ts(&self) -> Option<i64> {
        match self.iter().next() {
            Some((_, Some(ts))) => Some(ts),
            _ => None,
        }
    }

    pub(crate) fn create_obj(&mut self, obj: &DynamicObject, ts: i64) {
        let ns_name = namespaced_name(obj);
        if !self.tracked_objs.contains_key(&ns_name) {
            self.append_event(obj.clone(), ts, TraceAction::ObjectCreated);
        }
        self.tracked_objs.insert(ns_name, self.version);
    }

    pub(crate) fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) {
        let ns_name = namespaced_name(obj);
        if self.tracked_objs.contains_key(&ns_name) {
            self.append_event(obj.clone(), ts, TraceAction::ObjectDeleted);
        }
        self.tracked_objs.remove(&ns_name);
    }

    pub(crate) fn update_all_objs(&mut self, objs: Vec<DynamicObject>, ts: i64) {
        for obj in objs.iter() {
            self.create_obj(obj, ts);
        }

        let mut to_delete: Vec<String> = vec![];
        for (ns_name, version) in self.tracked_objs.iter() {
            if *version == self.version {
                continue;
            }
            to_delete.push(ns_name.into());
        }

        for ns_name in to_delete.iter() {
            let (ns, name) = split_namespaced_name(ns_name);
            let obj = DynamicObject {
                metadata: metav1::ObjectMeta {
                    namespace: Some(ns),
                    name: Some(name),
                    ..Default::default()
                },
                types: None,
                data: Value::Null,
            };
            self.delete_obj(&obj, ts);
        }

        self.version += 1;
    }

    fn append_event(&mut self, obj: DynamicObject, ts: i64, action: TraceAction) {
        info!("{} - {:?} @ {}", namespaced_name(&obj), action, ts);
        if let Some(evt) = self.trace.back_mut() {
            if evt.ts == ts {
                match action {
                    TraceAction::ObjectCreated => evt.created_objs.push(obj),
                    TraceAction::ObjectDeleted => evt.deleted_objs.push(obj),
                }
                return;
            }
        }

        let evt = match action {
            TraceAction::ObjectCreated => TraceEvent { ts, created_objs: vec![obj], deleted_objs: vec![] },
            TraceAction::ObjectDeleted => TraceEvent { ts, created_objs: vec![], deleted_objs: vec![obj] },
        };
        self.trace.push_back(evt);
    }

    fn collect_events(
        &self,
        start_ts: i64,
        end_ts: i64,
        filter: &TraceFilter,
    ) -> (Vec<TraceEvent>, HashMap<String, u64>) {
        let mut events = vec![TraceEvent {
            ts: start_ts,
            created_objs: vec![],
            deleted_objs: vec![],
        }];
        let mut flattened_obj_objects = HashMap::new();
        let mut tracked_objs = HashMap::new();
        for (evt, _) in self.iter() {
            // trace should be end-exclusive, so we use >= here: anything that is at the
            // end_ts or greater gets discarded.  The event list is stored in
            // monotonically-increasing order so we are safe to break here.
            if evt.ts >= end_ts {
                break;
            }

            if let Some(new_evt) = filter_event(&evt, filter) {
                for obj in new_evt.created_objs.iter() {
                    let ns_name = namespaced_name(obj);
                    if new_evt.ts < start_ts {
                        flattened_obj_objects.insert(ns_name.clone(), obj.clone());
                    }
                    tracked_objs.insert(ns_name, self.version);
                }

                for obj in evt.deleted_objs.iter() {
                    let ns_name = namespaced_name(obj);
                    if new_evt.ts < start_ts {
                        flattened_obj_objects.remove(&ns_name);
                    }
                    tracked_objs.remove(&ns_name);
                }
                if new_evt.ts >= start_ts {
                    events.push(new_evt.clone());
                }
            }
        }

        events[0].created_objs = flattened_obj_objects.values().cloned().collect();
        (events, tracked_objs)
    }
}

pub struct TraceIterator<'a> {
    trace: &'a VecDeque<TraceEvent>,
    idx: usize,
}

impl<'a> Tracer {
    pub fn iter(&'a self) -> TraceIterator<'a> {
        TraceIterator { trace: &self.trace, idx: 0 }
    }
}

impl<'a> Iterator for TraceIterator<'a> {
    type Item = (TraceEvent, Option<i64>);

    fn next(&mut self) -> Option<Self::Item> {
        let ret = match self.idx {
            i if i < self.trace.len() - 1 => Some((self.trace[i].clone(), Some(self.trace[i + 1].ts))),
            i if i == self.trace.len() - 1 => Some((self.trace[i].clone(), None)),
            _ => None,
        };

        self.idx += 1;
        ret
    }
}
