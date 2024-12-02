use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use sk_store::{
    ExportedTrace,
    TraceEvent,
};
use tokio::sync::Mutex;

use super::*;

pub fn build_trace_data(has_start_marker: bool) -> Vec<u8> {
    // I want the trace data to be easily editable, so we load it from a plain-text JSON file and
    // then re-encode it into msgpack so we can pass the data to import
    let trace_data_file = File::open("../testdata/trace.json").unwrap();
    let reader = BufReader::new(trace_data_file);
    let mut exported_trace: ExportedTrace = serde_json::from_reader(reader).unwrap();

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
    DriverContext {
        name: TEST_DRIVER_NAME.into(),
        root_name: TEST_DRIVER_ROOT_NAME.into(),
        sim: Simulation::new(TEST_SIM_NAME, Default::default()),
        ctrl_ns: TEST_CTRL_NAMESPACE.into(),
        virtual_ns_prefix: TEST_VIRT_NS_PREFIX.into(),
        owners_cache,
        store,
    }
}
