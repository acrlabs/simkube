use std::sync::Arc;

use sk_store::TraceEvent;
use tokio::sync::Mutex;

use super::*;

pub const TRACE_START: i64 = 1709241485;

pub fn build_trace_data(has_start_marker: bool, duration: Option<i64>) -> Vec<u8> {
    // I want the trace data to be easily editable, so we load it from a plain-text JSON file and
    // then re-encode it into msgpack so we can pass the data to import
    let mut exported_trace = exported_trace_from_json("trace");

    if has_start_marker {
        exported_trace.prepend_event(TraceEvent {
            ts: TRACE_START,
            applied_objs: vec![],
            deleted_objs: vec![],
        });
    }
    if let Some(d) = duration {
        exported_trace.append_event(TraceEvent {
            ts: TRACE_START + d,
            applied_objs: vec![],
            deleted_objs: vec![],
        });
    }

    rmp_serde::to_vec_named(&exported_trace).unwrap()
}

pub fn build_driver_context(owners_cache: OwnersCache, trace: ExportedTrace) -> DriverContext {
    DriverContext {
        name: TEST_DRIVER_NAME.into(),
        sim_name: TEST_SIM_NAME.into(),
        root_name: TEST_DRIVER_ROOT_NAME.into(),
        ctrl_ns: TEST_CTRL_NAMESPACE.into(),
        virtual_ns_prefix: TEST_VIRT_NS_PREFIX.into(),
        owners_cache: Arc::new(Mutex::new(owners_cache)),
        trace: Arc::new(trace),
    }
}
