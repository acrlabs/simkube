use std::collections::HashMap;

use assertables::*;
use kube::api::DynamicObject;
use sk_testutils::EMPTY_POD_SPEC_HASH;

use super::*;
use crate::k8s::{
    PodLifecycleData,
    SkResourceExt,
};
use crate::trace::Trace;
use crate::trace::event::TraceEvent;


#[fixture]
fn test_trace() -> Trace {
    Trace::default()
}

#[rstest]
fn test_lookup_pod_lifecycle_no_owner(test_trace: Trace, test_deployment: DynamicObject) {
    let res = test_trace.lookup_pod_lifecycle(&test_deployment.resource_id(), EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle_no_hash(mut test_trace: Trace, test_deployment: DynamicObject) {
    let depl_id = test_deployment.resource_id();
    test_trace.index.insert(&depl_id, 1234);
    let res = test_trace.lookup_pod_lifecycle(&depl_id, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle(mut test_trace: Trace, test_deployment: DynamicObject) {
    let pod_lifecycle = PodLifecycleData::Finished(1, 2);

    let depl_id = test_deployment.resource_id();
    test_trace.index.insert(&depl_id, 1234);
    test_trace.pod_lifecycles =
        HashMap::from([(depl_id.clone(), HashMap::from([(EMPTY_POD_SPEC_HASH, vec![pod_lifecycle.clone()])]))]);

    let res = test_trace.lookup_pod_lifecycle(&depl_id, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, pod_lifecycle);
}

#[rstest]
fn test_trace_start_end_ts(mut test_trace: Trace) {
    test_trace.append_event(TraceEvent { ts: 0, ..Default::default() });
    test_trace.append_event(TraceEvent { ts: 1, ..Default::default() });

    assert_some_eq_x!(test_trace.start_ts(), 0);
    assert_some_eq_x!(test_trace.end_ts(), 1);
}
