use std::collections::{
    HashMap,
    HashSet,
    VecDeque,
};
use std::sync::{
    Arc,
    Mutex,
};

use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use serde::{
    Deserialize,
    Serialize,
};
use tracing::*;

use crate::prelude::*;
use crate::util::*;
use crate::watchertracer::trace_filter::filter_event;
use crate::watchertracer::TraceFilter;

#[derive(Debug)]
enum TraceAction {
    PodCreated,
    PodDeleted,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TraceEvent {
    pub ts: i64,
    pub created_pods: Vec<corev1::Pod>,
    pub deleted_pods: Vec<corev1::Pod>,
}

pub struct Tracer {
    pub(super) trace: VecDeque<TraceEvent>,
    pub(super) tracked_pods: HashMap<String, u64>,
    pub(super) version: u64,
}

impl Tracer {
    pub fn new() -> Arc<Mutex<Tracer>> {
        return Arc::new(Mutex::new(Tracer {
            trace: VecDeque::new(),
            tracked_pods: HashMap::new(),
            version: 0,
        }));
    }

    pub fn import(data: Vec<u8>) -> SimKubeResult<Tracer> {
        let trace = rmp_serde::from_slice(&data)?;

        let mut tracer = Tracer { trace, tracked_pods: HashMap::new(), version: 0 };
        let (_, tracked_pods) = tracer.collect_events(0, i64::MAX, &TraceFilter::blank());
        tracer.tracked_pods = tracked_pods;

        return Ok(tracer);
    }

    pub fn export(&self, start_ts: i64, end_ts: i64, filter: &TraceFilter) -> SimKubeResult<Vec<u8>> {
        info!("Exporting pods with filters: {:?}", filter);
        let (events, _) = self.collect_events(start_ts, end_ts, filter);
        let data = rmp_serde::to_vec_named(&events)?;

        info!("Exported {} events.", events.len());
        return Ok(data);
    }

    pub fn pods(&self) -> HashSet<String> {
        return self.tracked_pods.keys().cloned().collect();
    }

    pub fn pods_at(&self, end_ts: i64, filter: &TraceFilter) -> HashSet<String> {
        let (_, tracked_pods) = self.collect_events(0, end_ts, filter);
        return tracked_pods.keys().cloned().collect();
    }

    pub(super) fn create_pod(&mut self, pod: &corev1::Pod, ts: i64) {
        let ns_name = namespaced_name(pod);
        if !self.tracked_pods.contains_key(&ns_name) {
            self.append_event(pod.clone(), ts, TraceAction::PodCreated);
        }
        self.tracked_pods.insert(ns_name, self.version);
    }

    pub(super) fn delete_pod(&mut self, pod: &corev1::Pod, ts: i64) {
        let ns_name = namespaced_name(pod);
        if self.tracked_pods.contains_key(&ns_name) {
            self.append_event(pod.clone(), ts, TraceAction::PodDeleted);
        }
        self.tracked_pods.remove(&ns_name);
    }

    pub(super) fn update_all_pods(&mut self, pods: Vec<corev1::Pod>, ts: i64) {
        for pod in pods.iter() {
            self.create_pod(pod, ts);
        }

        let mut to_delete: Vec<String> = vec![];
        for (ns_name, version) in self.tracked_pods.iter() {
            if *version == self.version {
                continue;
            }
            to_delete.push(ns_name.into());
        }

        for ns_name in to_delete.iter() {
            let (ns, name) = split_namespaced_name(ns_name);
            let pod = corev1::Pod {
                metadata: metav1::ObjectMeta {
                    namespace: Some(ns),
                    name: Some(name),
                    ..Default::default()
                },
                spec: None,
                status: None,
            };
            self.delete_pod(&pod, ts);
        }

        self.version += 1;
    }

    fn append_event(&mut self, pod: corev1::Pod, ts: i64, action: TraceAction) {
        info!("{} - {:?} @ {}", namespaced_name(&pod), action, ts);
        if let Some(evt) = self.trace.back_mut() {
            if evt.ts == ts {
                match action {
                    TraceAction::PodCreated => evt.created_pods.push(pod),
                    TraceAction::PodDeleted => evt.deleted_pods.push(pod),
                }
                return;
            }
        }

        let evt = match action {
            TraceAction::PodCreated => TraceEvent { ts, created_pods: vec![pod], deleted_pods: vec![] },
            TraceAction::PodDeleted => TraceEvent { ts, created_pods: vec![], deleted_pods: vec![pod] },
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
            created_pods: vec![],
            deleted_pods: vec![],
        }];
        let mut flattened_pod_objects = HashMap::new();
        let mut tracked_pods = HashMap::new();
        for (evt, _) in self.iter() {
            // trace should be end-exclusive, so we use >= here: anything that is at the
            // end_ts or greater gets discarded.  The event list is stored in
            // monotonically-increasing order so we are safe to break here.
            if evt.ts >= end_ts {
                break;
            }

            if let Some(new_evt) = filter_event(&evt, filter) {
                for pod in new_evt.created_pods.iter() {
                    let ns_name = namespaced_name(pod);
                    if new_evt.ts < start_ts {
                        flattened_pod_objects.insert(ns_name.clone(), pod.clone());
                    }
                    tracked_pods.insert(ns_name, self.version);
                }

                for pod in evt.deleted_pods.iter() {
                    let ns_name = namespaced_name(pod);
                    if new_evt.ts < start_ts {
                        flattened_pod_objects.remove(&ns_name);
                    }
                    tracked_pods.remove(&ns_name);
                }
                if new_evt.ts >= start_ts {
                    events.push(new_evt.clone());
                }
            }
        }

        events[0].created_pods = flattened_pod_objects.values().cloned().collect();
        return (events, tracked_pods);
    }
}

pub struct TraceIterator<'a> {
    trace: &'a VecDeque<TraceEvent>,
    idx: usize,
}

impl<'a> Tracer {
    pub fn iter(&'a self) -> TraceIterator<'a> {
        return TraceIterator { trace: &self.trace, idx: 0 };
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
        return ret;
    }
}
