use std::collections::{
    HashMap,
    VecDeque,
};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use simkube::store::{
    PodLifecyclesMap,
    TraceEvent,
};
use tokio::sync::Mutex;

use super::*;

pub fn build_trace_data(has_start_marker: bool) -> Vec<u8> {
    // I want the trace data to be easily editable, so we load it from a plain-text JSON file and
    // then re-encode it into msgpack so we can pass the data to import
    let trace_data_file = File::open("./driver/tests/data/trace.json").unwrap();
    let reader = BufReader::new(trace_data_file);
    let (config, mut events, index, lifecycle_data): (
        TracerConfig,
        VecDeque<TraceEvent>,
        HashMap<String, u64>,
        HashMap<String, PodLifecyclesMap>,
    ) = serde_json::from_reader(reader).unwrap();

    if has_start_marker {
        events.push_front(TraceEvent {
            ts: 1709241485,
            applied_objs: vec![],
            deleted_objs: vec![],
        });
    }

    rmp_serde::to_vec_named(&(&config, &events, &index, &lifecycle_data)).unwrap()
}

pub fn build_driver_context(
    owners_cache: Arc<Mutex<OwnersCache>>,
    store: Arc<dyn TraceStorable + Send + Sync>,
) -> DriverContext {
    DriverContext {
        name: TEST_DRIVER_NAME.into(),
        root_name: TEST_DRIVER_ROOT_NAME.into(),
        sim: Simulation::new(TEST_SIM_NAME, Default::default()),
        virtual_ns_prefix: TEST_VIRT_NS_PREFIX.into(),
        owners_cache,
        store,
    }
}
