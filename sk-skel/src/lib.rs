pub mod ast;
pub mod engine;

mod context;
mod errors;

use pest_derive::Parser;

pub mod metric_names {
    pub const EVENT_MATCHED_COUNTER: &str = "trace_events_matched";
    pub const RESOURCE_MODIFIED_COUNTER: &str = "trace_event_resources_modified";
    pub const TOTAL_EVALUATION_TIME_GAUGE: &str = "total_evaluation_time";
}

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel.pest"]
pub struct SkelParser;

#[cfg(test)]
mod tests;
