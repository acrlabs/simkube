pub(super) mod ast;
pub(super) mod engine;

use std::fs;

use pest::Parser;
use pest_derive::Parser;
use sk_store::ExportedTrace;

use self::ast::parse_command;
use self::engine::apply_command_to_event;

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel/skel.pest"]
struct SkelParser;

pub fn apply_skel_file(trace: &ExportedTrace, skel_file: &str) -> anyhow::Result<ExportedTrace> {
    let skel_str = fs::read_to_string(skel_file)?;
    let skel = SkelParser::parse(Rule::skel, &skel_str)?;

    let parsed_commands = skel
        .filter_map(|cmd| match cmd.as_rule() {
            Rule::EOI => None,
            _ => Some(parse_command(cmd, trace.start_ts().unwrap_or_default())),
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let mut new_events = Vec::with_capacity(trace.events.len());
    for (evt, _) in trace.iter() {
        let mut new_event = evt.clone();
        for cmd in &parsed_commands {
            new_event = apply_command_to_event(cmd, new_event)?;
        }
        new_events.push(new_event);
    }

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
pub mod tests;
