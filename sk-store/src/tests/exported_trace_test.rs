use std::collections::HashMap;

use assertables::*;
use sk_core::k8s::PodLifecycleData;

use super::*;

#[fixture]
fn trace() -> ExportedTrace {
    ExportedTrace::default()
}

#[rstest]
fn test_lookup_pod_lifecycle_no_owner(trace: ExportedTrace) {
    let res = trace.lookup_pod_lifecycle(&DEPL_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle_no_hash(mut trace: ExportedTrace) {
    trace.index.insert(DEPL_GVK.clone(), TEST_DEPLOYMENT.into(), 1234);
    let res = trace.lookup_pod_lifecycle(&DEPL_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle(mut trace: ExportedTrace) {
    let owner_ns_name = format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}");
    let pod_lifecycle = PodLifecycleData::Finished(1, 2);

    trace.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), 1234);
    trace.pod_lifecycles = HashMap::from([(
        (DEPL_GVK.clone(), owner_ns_name.clone()),
        HashMap::from([(EMPTY_POD_SPEC_HASH, vec![pod_lifecycle.clone()])]),
    )]);

    let res = trace.lookup_pod_lifecycle(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, pod_lifecycle);
}

#[rstest]
fn test_trace_start_end_ts(mut trace: ExportedTrace) {
    trace.append_event(TraceEvent { ts: 0, ..Default::default() });
    trace.append_event(TraceEvent { ts: 1, ..Default::default() });

    assert_some_eq_x!(trace.start_ts(), 0);
    assert_some_eq_x!(trace.end_ts(), 1);
}
