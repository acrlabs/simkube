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

#[derive(Debug)]
enum TraceAction {
    PodCreated,
    PodDeleted,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
        tracer.tracked_pods = tracer.replay_trace(None).keys().map(|pod_name| (pod_name.clone(), 0)).collect();

        return Ok(tracer);
    }

    pub fn export(&self, start: i64, end: i64) -> SimKubeResult<Vec<u8>> {
        let mut events = vec![TraceEvent {
            ts: start,
            created_pods: self.replay_trace(Some(start)).values().cloned().collect(),
            deleted_pods: Vec::new(),
        }];
        events.extend(self.trace.iter().filter(|evt| evt.ts >= start && evt.ts < end).cloned());
        let data = rmp_serde::to_vec_named(&events)?;

        return Ok(data);
    }

    pub fn pods(&self) -> HashSet<String> {
        return self.tracked_pods.keys().cloned().collect();
    }

    pub fn pods_at(&self, ts: i64) -> HashSet<String> {
        let tracked_pods_at = self.replay_trace(Some(ts));
        return tracked_pods_at.keys().cloned().collect();
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

    fn replay_trace(&self, maybe_end: Option<i64>) -> HashMap<String, corev1::Pod> {
        let mut new_tracked_pods = HashMap::new();
        for (evt, _) in self.iter() {
            // trace should be end-exclusive, so we use <= here: anything that is at the
            // end_ts or greater gets discarded.
            if maybe_end.is_some_and(|end_ts| end_ts <= evt.ts) {
                break;
            }

            for pod in evt.created_pods.iter() {
                let ns_name = namespaced_name(pod);
                new_tracked_pods.insert(ns_name, pod.clone());
            }

            for pod in evt.deleted_pods.iter() {
                let ns_name = namespaced_name(pod);
                new_tracked_pods.remove(&ns_name);
            }
        }
        return new_tracked_pods;
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
