mod config;
mod pod_owners_map;
mod trace_filter;
mod trace_store;
pub mod watchers;

use std::collections::VecDeque;

use kube::api::DynamicObject;
use serde::{
    Deserialize,
    Serialize,
};
use sk_core::errors::*;
use sk_core::k8s::PodLifecycleData;
use sk_core::prelude::*;

pub use crate::config::{
    TracerConfig,
    TrackedObjectConfig,
};
pub use crate::trace_store::TraceStore;

#[cfg(test)]
mod tests;
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
    fn end_ts(&self) -> Option<i64>;
    fn iter(&self) -> TraceIterator<'_>;
}

#[cfg(feature = "testutils")]
pub mod mock {
    use mockall::mock;

    use super::*;

    mock! {
        pub TraceStore {}

        impl TraceStorable for TraceStore {
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
            fn end_ts(&self) -> Option<i64>;
            fn iter<'a>(&'a self) -> TraceIterator<'a>;
        }
    }
}

#[cfg(feature = "testutils")]
pub use crate::pod_owners_map::PodLifecyclesMap;
