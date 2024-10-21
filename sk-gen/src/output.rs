//! This module contains graph/trace output utilities and functionality.

use std::collections::{
    HashMap,
    VecDeque,
};
use std::path::PathBuf;

use anyhow::Result;
use sk_core::k8s::GVK;
use sk_store::{
    PodLifecyclesMap,
    TraceEvent,
    TracerConfig,
    TrackedObjectConfig,
};

use crate::{
    Cli,
    ClusterGraph,
    Edge,
    Node,
    Walk,
};


// The rest of SimKube handles this data as a tuple, but since some of us are newer, we just use a
// struct.
/// A sequence of [`TraceEvent`] instances and additional metadata which can be simulated by
/// SimKube.
pub(crate) struct Trace {
    config: TracerConfig,
    /// At the moment, this is the only field we are using. This is just a queue of
    /// `TraceEvent`s.
    events: VecDeque<TraceEvent>,
    index: HashMap<String, u64>,
    pod_lifecycles: HashMap<String, PodLifecyclesMap>,
}

impl Trace {
    /// Creates a new `trace` from a `Walk`.
    ///
    /// This simply entails extracting the [`TraceEvent`]s from the [`Edge`]s of the walk.
    pub(crate) fn from_walk(walk: &Walk) -> Self {
        let events = walk
            .iter()
            .filter_map(|(edge, _node)| edge.as_ref().map(|e| e.trace_event.clone()))
            .collect();
        Self::from_trace_events(events)
    }

    /// Creates a new `Trace` from a sequence of `TraceEvent` instances.
    pub(crate) fn from_trace_events(events: Vec<TraceEvent>) -> Self {
        let events = VecDeque::from(events);

        let config = TracerConfig {
            tracked_objects: HashMap::from([(
                GVK::new("apps", "v1", "Deployment"),
                TrackedObjectConfig {
                    track_lifecycle: false,
                    pod_spec_template_path: None,
                },
            )]),
        };

        let index = HashMap::new(); // TODO
        let pod_lifecycles = HashMap::new(); // TODO

        Self { config, events, index, pod_lifecycles }
    }

    fn to_tuple(
        &self,
    ) -> (TracerConfig, VecDeque<TraceEvent>, HashMap<String, u64>, HashMap<String, PodLifecyclesMap>) {
        (self.config.clone(), self.events.clone(), self.index.clone(), self.pod_lifecycles.clone())
    }

    fn to_msgpack(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(&self.to_tuple())?)
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.to_tuple())?)
    }
}

/// Generate the simkube-consumable trace event (i.e. applied/deleted objects) to get from
/// `prev` to `next` state over `ts` seconds.
pub(crate) fn gen_trace_event(ts: i64, prev: &Node, next: &Node) -> TraceEvent {
    let mut applied_objs = Vec::new();
    let mut deleted_objs = Vec::new();

    for (name, deployment) in &prev.deployments {
        if !next.deployments.contains_key(name) {
            deleted_objs.push(deployment.to_dynamic_object());
        } else if deployment != &next.deployments[name] {
            applied_objs.push(next.deployments[name].to_dynamic_object());
        }
    }

    for (name, deployment) in &next.deployments {
        if !prev.deployments.contains_key(name) {
            applied_objs.push(deployment.to_dynamic_object());
        }
    }

    TraceEvent { ts, applied_objs, deleted_objs }
}

/// Display walks and traces as specified by user-provided CLI flags.
pub(crate) fn display_walks_and_traces(walks: &[Walk], traces: &[Trace], cli: &Cli) -> Result<()> {
    // create output directory if it doesn't exist
    if let Some(traces_dir) = &cli.traces_output_dir {
        if !traces_dir.exists() {
            std::fs::create_dir_all(traces_dir)?;
        }
    }

    for (i, (walk, trace)) in walks.iter().zip(traces.iter()).enumerate() {
        if let Some(traces_dir) = &cli.traces_output_dir {
            let data = trace.to_msgpack()?;
            let path = traces_dir.join(format!("trace-{i}.mp"));
            std::fs::write(path, data)?;
        }

        if cli.display_walks {
            println!("walk-{i}:");
            display_walk(walk);
            println!();
        }

        if cli.display_traces {
            println!("trace-{i}:");
            println!("{}", trace.to_json()?);
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
            println!("{:?}", e.action);
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
