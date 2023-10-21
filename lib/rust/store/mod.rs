mod pod_owners_map;
pub mod storage;
mod trace_filter;
mod trace_store;

use std::collections::{
    HashMap,
    VecDeque,
};

use kube::api::DynamicObject;
use serde::{
    Deserialize,
    Serialize,
};

use self::pod_owners_map::{
    PodLifecyclesMap,
    PodOwnersMap,
};
use self::trace_filter::filter_event;
pub use self::trace_filter::TraceFilter;
use crate::errors::*;
use crate::prelude::*;

#[derive(Debug)]
enum TraceAction {
    ObjectApplied,
    ObjectDeleted,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TraceEvent {
    pub ts: i64,
    pub applied_objs: Vec<DynamicObject>,
    pub deleted_objs: Vec<DynamicObject>,
}

pub struct TraceIterator<'a> {
    events: &'a VecDeque<TraceEvent>,
    idx: usize,
}

pub trait TraceStorable {
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>);
    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64);
    fn update_all_objs(&mut self, objs: &[DynamicObject], ts: i64);
    fn lookup_pod_lifecycle(&self, owner_ns_name: &str, pod_hash: u64, seq: usize) -> PodLifecycleData;
    fn record_pod_lifecycle(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<corev1::Pod>,
        owners: Vec<metav1::OwnerReference>,
        lifecycle_data: &PodLifecycleData,
    ) -> EmptyResult;
    fn config(&self) -> &TracerConfig;
    fn has_obj(&self, ns_name: &str) -> bool;
    fn start_ts(&self) -> Option<i64>;
    fn iter(&self) -> TraceIterator<'_>;
}

#[derive(Default)]
pub struct TraceStore {
    config: TracerConfig,
    events: VecDeque<TraceEvent>,
    pod_owners: PodOwnersMap,
    index: HashMap<String, u64>,
}

#[cfg(test)]
mod tests;
