mod ast;
mod context;
mod engine;
mod errors;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use pest::Parser;
use pest_derive::Parser;
use sk_store::{
    ExportedTrace,
    TraceEvent,
};

use crate::ast::{
    Command,
    parse_command,
};
use crate::engine::process_json_event;
use crate::errors::SkelError;

pub mod metric_names {
    pub const EVENT_MATCHED_COUNTER: &str = "trace_events_matched";
    pub const RESOURCE_MODIFIED_COUNTER: &str = "trace_event_resources_modified";
    pub const TOTAL_EVALUATION_TIME_GAUGE: &str = "total_evaluation_time";
}

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel.pest"]
struct SkelParser;

fn is_empty(evt: &serde_json::Value) -> anyhow::Result<bool> {
    Ok(evt
        .get("applied_objs")
        .ok_or(SkelError::MalformedTraceEvent("no applied_objs field".into()))?
        .as_array()
        .ok_or(SkelError::MalformedTraceEvent("applied_objs not an array".into()))?
        .is_empty()
        && evt
            .get("deleted_objs")
            .ok_or(SkelError::MalformedTraceEvent("no deleted_objs field".into()))?
            .as_array()
            .ok_or(SkelError::MalformedTraceEvent("deleted_objs not an array".into()))?
            .is_empty())
}

pub fn process_trace(
    trace: &ExportedTrace,
    commands: &Vec<Command>,
    event_counter: Option<Arc<AtomicUsize>>,
) -> anyhow::Result<Vec<TraceEvent>> {
    let mut new_events = Vec::with_capacity(trace.events.len());

    // To resolve JSON pointers we need things as json::Value objects; it's more
    // efficient to serialize all of the events once than it is to do it in the loop
    let trace_events_json = serde_json::to_value(&trace.events)?;
    for evt in trace_events_json.as_array().unwrap().iter() {
        let mut new_event = evt.clone();
        for cmd in commands {
            new_event = process_json_event(cmd, new_event)?;
        }

        // Only add the event if it actually does anything
        if !is_empty(&new_event)? {
            new_events.push(new_event);
        }
        if let Some(ref c) = event_counter {
            c.fetch_add(1, Ordering::Relaxed);
        }
    }

    Ok(serde_json::from_value(serde_json::Value::Array(new_events))?)
}

pub async fn apply_skel_file(
    trace: &ExportedTrace,
    skel_file: &str,
    event_counter: Arc<AtomicUsize>,
) -> anyhow::Result<ExportedTrace> {
    let skel_str = fs::read_to_string(skel_file)?;
    let skel = SkelParser::parse(Rule::skel, &skel_str)?;

    let parsed_commands = skel
        .filter_map(|cmd| match cmd.as_rule() {
            Rule::EOI => None,
            _ => Some(parse_command(cmd, trace.start_ts().unwrap_or_default())),
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let new_events = process_trace(trace, &parsed_commands, Some(event_counter))?;
    let new_trace = ExportedTrace {
        version: trace.version,
        config: trace.config.clone(),
        events: new_events,
        index: trace.index.clone(),
        pod_lifecycles: trace.pod_lifecycles.clone(),
    };

    Ok(new_trace)
}

#[cfg(test)]
mod tests;
