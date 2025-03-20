mod config;
mod event_list;
mod filter;
mod index;
mod pod_owners_map;
mod store;
pub mod watchers;

use std::collections::HashMap;

use kube::api::DynamicObject;
use serde::{
    Deserialize,
    Serialize,
};
use sk_core::errors::*;
use sk_core::k8s::{
    PodLifecycleData,
    GVK,
};
use sk_core::prelude::*;

pub use crate::config::{
    TracerConfig,
    TrackedObjectConfig,
};
pub use crate::event_list::TraceEventList;
pub use crate::index::TraceIndex;
use crate::pod_owners_map::PodLifecyclesMap;
pub use crate::store::TraceStore;

#[derive(Clone, Copy, Debug)]
pub enum TraceAction {
    ObjectApplied,
    ObjectDeleted,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TraceEvent {
    pub ts: i64,
    pub applied_objs: Vec<DynamicObject>,
    pub deleted_objs: Vec<DynamicObject>,
}

impl TraceEvent {
    pub fn len(&self) -> usize {
        self.applied_objs.len() + self.deleted_objs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.applied_objs.is_empty() && self.deleted_objs.is_empty()
    }
}

pub struct TraceIterator<'a> {
    events: &'a TraceEventList,
    idx: usize,
}

const CURRENT_TRACE_FORMAT_VERSION: u16 = 2;

pub trait TraceStorable {
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) -> EmptyResult;
    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult;
    fn update_all_objs_for_gvk(&mut self, gvk: &GVK, objs: &[DynamicObject], ts: i64) -> EmptyResult;
    fn lookup_pod_lifecycle(&self, gvk: &GVK, owner_ns_name: &str, pod_hash: u64, seq: usize) -> PodLifecycleData;
    fn record_pod_lifecycle(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<corev1::Pod>,
        owners: Vec<metav1::OwnerReference>,
        lifecycle_data: &PodLifecycleData,
    ) -> EmptyResult;
    fn config(&self) -> &TracerConfig;
    fn has_obj(&self, gvk: &GVK, ns_name: &str) -> bool;
    fn start_ts(&self) -> Option<i64>;
    fn end_ts(&self) -> Option<i64>;
    fn iter(&self) -> TraceIterator<'_>;
}

#[derive(Deserialize, Serialize)]
pub struct ExportedTrace {
    version: u16,
    config: TracerConfig,
    events: Vec<TraceEvent>,
    index: TraceIndex,
    pod_lifecycles: HashMap<(GVK, String), PodLifecyclesMap>,
}

impl ExportedTrace {
    pub fn prepend_event(&mut self, event: TraceEvent) {
        let mut tmp = vec![event];
        tmp.append(&mut self.events);
        self.events = tmp;
    }

    pub fn events(&self) -> Vec<TraceEvent> {
        self.events.clone()
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "mock")]
pub mod mock {
    use mockall::mock;

    use super::*;

    mock! {
        pub TraceStore {}

        impl TraceStorable for TraceStore {
            fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) -> EmptyResult;
            fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult;
            fn update_all_objs_for_gvk(&mut self, gvk: &GVK, objs: &[DynamicObject], ts: i64) -> EmptyResult;
            fn lookup_pod_lifecycle(&self, owner_gvk: &GVK, owner_ns_name: &str, pod_hash: u64, seq: usize) -> PodLifecycleData;
            fn record_pod_lifecycle(
                &mut self,
                ns_name: &str,
                maybe_pod: Option<corev1::Pod>,
                owners: Vec<metav1::OwnerReference>,
                lifecycle_data: &PodLifecycleData,
            ) -> EmptyResult;
            fn config(&self) -> &TracerConfig;
            fn has_obj(&self, gvk: &GVK, ns_name: &str) -> bool;
            fn start_ts(&self) -> Option<i64>;
            fn end_ts(&self) -> Option<i64>;
            fn iter<'a>(&'a self) -> TraceIterator<'a>;
        }
    }
}
