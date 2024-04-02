use kube::api::DynamicObject;
use mockall::mock;

use crate::errors::*;
use crate::prelude::*;
use crate::store::{
    TraceIterator,
    TraceStorable,
};

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
