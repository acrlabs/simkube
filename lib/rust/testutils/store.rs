use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use mockall::mock;

use crate::config::TracerConfig;
use crate::errors::*;
use crate::k8s::PodLifecycleData;
use crate::store::{
    TraceIterator,
    TraceStorable,
};

mock! {
    pub TraceStore {}

    impl TraceStorable for TraceStore {
        fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>);
        fn delete_obj(&mut self, obj: &DynamicObject, ts: i64);
        fn update_all_objs(&mut self, objs: &Vec<DynamicObject>, ts: i64);
        fn lookup_pod_lifecycle(
            &self,
            pod: &corev1::Pod,
            owner_ns_name: &str,
            seq: usize,
        ) -> anyhow::Result<PodLifecycleData>;
        fn record_pod_lifecycle(
            &mut self,
            ns_name: &str,
            maybe_pod: Option<corev1::Pod>,
            owners: Vec<metav1::OwnerReference>,
            lifecycle_data: PodLifecycleData,
        ) -> EmptyResult;
        fn config(&self) -> &TracerConfig;
        fn start_ts(&self) -> Option<i64>;
        fn iter<'a>(&'a self) -> TraceIterator<'a>;
    }
}
