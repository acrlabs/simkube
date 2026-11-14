mod ast;
mod context;
mod engine;
mod errors;

use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use pest::Parser;
use pest_derive::Parser;
use sk_core::prelude::*;

use crate::ast::{
    Command,
    parse_command,
};
use crate::engine::process_event;

pub mod metric_names {
    pub const EVENT_MATCHED_COUNTER: &str = "trace_events_matched";
    pub const RESOURCE_MODIFIED_COUNTER: &str = "trace_event_resources_modified";
    pub const TOTAL_EVALUATION_TIME_GAUGE: &str = "total_evaluation_time";
}

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel.pest"]
struct SkelParser;

// SKEL main execution function
// takes a parsed set of skel commands and runs them against a trace
pub fn process_trace(
    trace: &Trace,
    commands: &[Command],
    event_counter: Option<Arc<AtomicUsize>>,
) -> anyhow::Result<Vec<TraceEvent>> {
    let mut new_events = Vec::with_capacity(trace.events.len());

    // To resolve JSON pointers we need things as json::Value objects; it's more
    // efficient to serialize all of the events once than it is to do it in the loop
    for evt in trace.events.iter() {
        let new_event = process_event(commands, evt)?;

        // Only add the event if it actually does anything
        if !new_event.is_empty() {
            new_events.push(new_event);
        }
        if let Some(ref c) = event_counter {
            c.fetch_add(1, Ordering::Relaxed);
        }
    }

    Ok(new_events)
}

/// SKEL Main function, parses a SKEL file/string  and applies it to a trace end to end returning
/// progress updates through event_counter
pub async fn apply_skel(
    trace: &Trace,
    skel_str: &str,
    event_counter: Option<Arc<AtomicUsize>>,
) -> anyhow::Result<Trace> {
    let parsed_commands = parse_skel_commands(skel_str, trace.start_ts().unwrap_or_default())?;
    let new_events = process_trace(trace, &parsed_commands, event_counter)?;
    let new_trace = Trace {
        version: trace.version,
        config: trace.config.clone(),
        events: new_events,
        index: trace.index.clone(),
    };

    Ok(new_trace)
}

/// Main Parsing function, handles parsing of a SKEL string/file, and returns all the commands
/// contained within dispatches to individual handlers through [parse_command]
pub fn parse_skel_commands(skel_str: &str, start_ts: i64) -> anyhow::Result<Vec<Command>> {
    let skel = SkelParser::parse(Rule::skel, skel_str)?;
    skel.filter_map(|cmd| match cmd.as_rule() {
        Rule::EOI => None,
        _ => Some(parse_command(cmd, start_ts)),
    })
    .collect::<Result<Vec<_>, anyhow::Error>>()
}

#[cfg(test)]
mod tests;
