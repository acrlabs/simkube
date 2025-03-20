use std::fs::File;
use std::io::BufReader;

use sk_store::ExportedTrace;

pub fn exported_trace_from_json(trace_type: &str) -> ExportedTrace {
    let filename = format!("{}/data/{trace_type}.json", env!("CARGO_MANIFEST_DIR"));
    let trace_data_file = File::open(filename).unwrap();
    let reader = BufReader::new(trace_data_file);
    serde_json::from_reader(reader).unwrap()
}
