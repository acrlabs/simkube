use std::sync::mpsc;

use pest::Parser;
use sk_core::event::TraceEvent;
use sk_skel::ast::{
    Command,
    parse_command,
};
use sk_skel::engine::process_event;
use sk_skel::{
    Rule,
    SkelParser,
};

use crate::ExportedTrace;

pub fn process_trace(
    trace: &ExportedTrace,
    commands: &Vec<Command>,
    update_channel: Option<mpsc::Sender<()>>,
) -> anyhow::Result<Vec<TraceEvent>> {
    let mut new_events = Vec::with_capacity(trace.events.len());
    for (evt, _) in trace.iter() {
        let mut new_event = evt.clone();
        for cmd in commands {
            new_event = process_event(cmd, new_event)?;
        }

        // Only add the event if it actually does anything
        if !new_event.applied_objs.is_empty() || !new_event.deleted_objs.is_empty() {
            new_events.push(new_event);
        }
        if let Some(ref c) = update_channel {
            let _ = c.send(()); // if we can't send on the channel, nbd
        }
    }

    Ok(new_events)
}

pub async fn apply_skel(
    trace: &ExportedTrace,
    skel_str: &str,
    update_channel: mpsc::Sender<()>,
) -> anyhow::Result<ExportedTrace> {
    let skel = SkelParser::parse(Rule::skel, skel_str)?;

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
