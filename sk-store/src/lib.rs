#![cfg_attr(coverage, feature(coverage_attribute))]
mod config;
mod event;
mod filter;
mod index;
mod iter;
mod manager;
mod pod_owners_map;
mod store;
mod watchers;

use kube::api::DynamicObject;
use sk_core::errors::*;
use sk_core::k8s::{
    GVK,
    PodLifecycleData,
};
use sk_core::prelude::*;

pub use crate::config::{
    TracerConfig,
    TrackedObjectConfig,
};
pub use crate::event::{
    TraceAction,
    TraceEvent,
    TraceEventList,
};
pub use crate::index::TraceIndex;
pub use crate::iter::TraceIterator;
pub use crate::manager::TraceManager;
pub use crate::store::{
    ExportedTrace,
    TraceStore,
};

const CURRENT_TRACE_FORMAT_VERSION: u16 = 2;

pub trait TraceStorable {
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) -> EmptyResult;
    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult;
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

#[cfg(test)]
mod tests;

#[cfg(feature = "mock")]
#[cfg_attr(coverage, coverage(off))]
pub mod mock {
    use mockall::mock;

    use super::*;

    mock! {
        pub TraceStore {}

        impl TraceStorable for TraceStore {
            fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) -> EmptyResult;
            fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult;
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
