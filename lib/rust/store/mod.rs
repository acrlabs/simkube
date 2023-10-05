mod pod_owners_map;
pub mod storage;
mod trace_filter;
mod trace_store;

use std::collections::{
    HashMap,
    VecDeque,
};

use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
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
use crate::config::TracerConfig;
use crate::errors::*;
use crate::k8s::PodLifecycleData;

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

#[cfg_attr(test, automock)]
pub(crate) trait TraceStorable {
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>);
    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64);
    fn update_all_objs(&mut self, objs: &Vec<DynamicObject>, ts: i64);
    fn record_pod_lifecycle(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<corev1::Pod>,
        owners: Vec<metav1::OwnerReference>,
        lifecycle_data: PodLifecycleData,
    ) -> EmptyResult;
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

#[cfg(test)]
use mockall::automock;
