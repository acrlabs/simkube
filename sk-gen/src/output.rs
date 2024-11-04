//! This module contains graph/trace output utilities and functionality.

use std::path::PathBuf;

use anyhow::Result;
use sk_store::{
    TraceEvent,
    TraceStorable,
    TraceStore,
};

use crate::{
    Cli,
    ClusterGraph,
    Node,
    Walk,
};

/// Generate the simkube-consumable trace event (i.e. applied/deleted objects) to get from
/// `prev` to `next` state over `ts` seconds.
pub(crate) fn gen_trace_event(ts: i64, prev: &Node, next: &Node) -> TraceEvent {
    let mut applied_objs = Vec::new();
    let mut deleted_objs = Vec::new();

    for (name, deployment) in &prev.objects {
        if !next.objects.contains_key(name) {
            deleted_objs.push(deployment.clone());
        } else if deployment != &next.objects[name] {
            applied_objs.push(next.objects[name].clone());
        }
    }

    for (name, deployment) in &next.objects {
        if !prev.objects.contains_key(name) {
            applied_objs.push(deployment.clone());
        }
    }

    TraceEvent { ts, applied_objs, deleted_objs }
}

/// Display walks and traces as specified by user-provided CLI flags.
pub(crate) fn display_walks_and_traces(walks: &[Walk], traces: &[TraceStore], cli: &Cli) -> Result<()> {
    // create output directory if it doesn't exist
    if let Some(traces_dir) = &cli.traces_output_dir {
        if !traces_dir.exists() {
            std::fs::create_dir_all(traces_dir)?;
        }
    }

    for (i, (walk, trace)) in walks.iter().zip(traces.iter()).enumerate() {
        let min_ts = trace.start_ts().unwrap();
        let max_ts = trace.end_ts().unwrap();
        let export_filters = sk_api::v1::ExportFilters::default(); // TODO ensure this is non-restrictive

        if let Some(traces_dir) = &cli.traces_output_dir {
            let data = trace.export(min_ts, max_ts, &export_filters)?;
            let path = traces_dir.join(format!("trace-{i}.mp"));
            std::fs::write(path, data)?;
        }

        if cli.display_walks {
            println!("walk-{i}:");
            display_walk(walk);
            println!();
        }
    }

    Ok(())
}

/// Helper function to display a walk (handling the case where the incoming edge is None for the
/// first node).
fn display_walk(walk: &Walk) {
    for (edge, node) in walk {
        if let Some(e) = edge {
            println!("{:#?}", e.action);
        }
        println!("{node:#?}");
    }
}

/// Exports the graphviz representation of the graph to a file, ensuring the parent directory
/// exists.
pub(crate) fn export_graphviz(graph: &ClusterGraph, output_file: &PathBuf) -> Result<()> {
    // if the parent directory doesn't exist, create it
    assert!(!output_file.is_dir(), "graph output file must not be a directory");

    if let Some(parent) = output_file.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(output_file, graph.to_graphviz())?;
    Ok(())
}
