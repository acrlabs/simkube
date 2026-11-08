mod ast;
mod context;
mod engine;
mod errors;

use std::fs;
use std::sync::mpsc;

use pest::Parser;
use pest_derive::Parser;
use sk_store::{
    ExportedTrace,
    TraceEvent,
};

use self::ast::{
    Command,
    parse_command,
};
use self::engine::process_event;

pub mod metric_names {
    pub const EVENT_MATCHED_COUNTER: &str = "trace_events_matched";
    pub const RESOURCE_MODIFIED_COUNTER: &str = "trace_event_resources_modified";
    pub const TOTAL_EVALUATION_TIME_GAUGE: &str = "total_evaluation_time";
}

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel.pest"]
struct SkelParser;

pub fn process_trace(
    trace: &ExportedTrace,
    commands: &Vec<Command>,
    update_channel: Option<mpsc::Sender<()>>,
) -> anyhow::Result<Vec<TraceEvent>> {
    let mut new_events = Vec::with_capacity(trace.events.len());
    let trace_events_json = serde_json::to_value(&trace.events).unwrap();
    for evt in trace_events_json.as_array().unwrap().iter() {
        let mut new_event = evt.clone();
        for cmd in commands {
            new_event = process_event(cmd, new_event)?;
        }

        // Only add the event if it actually does anything
        if !new_event.get("applied_objs").unwrap().as_array().unwrap().is_empty()
            || !new_event.get("deleted_objs").unwrap().as_array().unwrap().is_empty()
        {
            new_events.push(new_event);
        }
        if let Some(ref c) = update_channel {
            let _ = c.send(()); // if we can't send on the channel, nbd
        }
    }

    Ok(serde_json::from_value(serde_json::Value::Array(new_events)).unwrap())
}

pub async fn apply_skel_file(
    trace: &ExportedTrace,
    skel_file: &str,
    update_channel: mpsc::Sender<()>,
) -> anyhow::Result<ExportedTrace> {
    let skel_str = fs::read_to_string(skel_file)?;
    let skel = SkelParser::parse(Rule::skel, &skel_str)?;

    let parsed_commands = skel
        .filter_map(|cmd| match cmd.as_rule() {
            Rule::EOI => None,
            _ => Some(parse_command(cmd, trace.start_ts().unwrap_or_default())),
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let new_events = process_trace(trace, &parsed_commands, Some(update_channel))?;
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
