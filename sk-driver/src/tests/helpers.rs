use std::sync::Arc;

use sk_api::v1::SimulationSpec;
use sk_store::TraceEvent;
use tokio::sync::Mutex;

use super::*;

pub fn build_trace_data(has_start_marker: bool) -> Vec<u8> {
    // I want the trace data to be easily editable, so we load it from a plain-text JSON file and
    // then re-encode it into msgpack so we can pass the data to import
    let mut exported_trace = exported_trace_from_json("trace");

    if has_start_marker {
        exported_trace.prepend_event(TraceEvent {
            ts: 1709241485,
            applied_objs: vec![],
            deleted_objs: vec![],
        });
    }

    rmp_serde::to_vec_named(&exported_trace).unwrap()
}

pub fn build_driver_context(
    owners_cache: Arc<Mutex<OwnersCache>>,
    store: Arc<dyn TraceStorable + Send + Sync>,
) -> DriverContext {
    let sim = Simulation::new(TEST_SIM_NAME, SimulationSpec { speed: Some(1.0), ..Default::default() });
    DriverContext {
        name: TEST_DRIVER_NAME.into(),
        root_name: TEST_DRIVER_ROOT_NAME.into(),
        sim,
        ctrl_ns: TEST_CTRL_NAMESPACE.into(),
        virtual_ns_prefix: TEST_VIRT_NS_PREFIX.into(),
        owners_cache,
        store,
    }
}
