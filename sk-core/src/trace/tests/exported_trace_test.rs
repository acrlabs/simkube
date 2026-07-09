use std::collections::HashMap;

use assertables::*;
use sk_testutils::{
    EMPTY_POD_SPEC_HASH,
    TEST_DEPLOYMENT,
    TEST_NAMESPACE,
};

use super::*;
use crate::constants::DEPLOYMENT_GVK;
use crate::event::TraceEvent;
use crate::k8s::PodLifecycleData;
use crate::trace::Trace;


#[fixture]
fn test_trace() -> Trace {
    Trace::default()
}

#[rstest]
fn test_lookup_pod_lifecycle_no_owner(test_trace: Trace) {
    let res = test_trace.lookup_pod_lifecycle(&DEPLOYMENT_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle_no_hash(mut test_trace: Trace) {
    test_trace.index.insert(DEPLOYMENT_GVK.clone(), TEST_DEPLOYMENT.into(), 1234);
    let res = test_trace.lookup_pod_lifecycle(&DEPLOYMENT_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle(mut test_trace: Trace) {
    let owner_ns_name = format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}");
    let pod_lifecycle = PodLifecycleData::Finished(1, 2);

    test_trace.index.insert(DEPLOYMENT_GVK.clone(), owner_ns_name.clone(), 1234);
    test_trace.pod_lifecycles = HashMap::from([(
        (DEPLOYMENT_GVK.clone(), owner_ns_name.clone()),
        HashMap::from([(EMPTY_POD_SPEC_HASH, vec![pod_lifecycle.clone()])]),
    )]);

    let res = test_trace.lookup_pod_lifecycle(&DEPLOYMENT_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, pod_lifecycle);
}

#[rstest]
fn test_trace_start_end_ts(mut test_trace: Trace) {
    test_trace.append_event(TraceEvent { ts: 0, ..Default::default() });
    test_trace.append_event(TraceEvent { ts: 1, ..Default::default() });

    assert_some_eq_x!(test_trace.start_ts(), 0);
    assert_some_eq_x!(test_trace.end_ts(), 1);
}
