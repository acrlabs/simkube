pub mod storage;
mod trace_filter;
mod trace_store;

use std::collections::HashMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use serde::{
    Deserialize,
    Serialize,
};

use self::trace_filter::filter_event;
pub use self::trace_filter::TraceFilter;
pub use self::trace_store::TraceStore;
use crate::k8s::PodLifecycleData;

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

#[cfg_attr(test, automock)]
pub(crate) trait TraceStorable {
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>);
    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64);
    fn update_all_objs(&mut self, objs: &Vec<DynamicObject>, ts: i64);
    fn record_pod_lifecycle(
        &mut self,
        _ns_name: &str,
        _owners: Vec<metav1::OwnerReference>,
        _lifecycle_data: &PodLifecycleData,
    );
}

type OwnedPodMap = HashMap<String, HashMap<u64, Vec<(i64, i64)>>>;

#[cfg(test)]
mod tests;

#[cfg(test)]
use mockall::automock;
