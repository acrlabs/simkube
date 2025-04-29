#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all, clippy::pedantic)]

use load::{
    Arena,
    Loader,
};
use petgraph::dot::Dot;
use rayon::prelude::*;
use sk_core::jsonutils::{
    ordered_eq,
    ordered_hash,
};
use sk_store::TraceStorable;
mod contraction_hierarchies;
mod output;
use std::cmp::Ordering;
use std::collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
};
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::Result;
use chrono::{
    DateTime,
    Utc,
};
use clap::Parser;
use contraction_hierarchies::{
    CHNode,
    Distance,
};
use daft::Diffable;
use indicatif::{
    ProgressBar,
    ProgressStyle,
};
use jaq_core::{
    load,
    Compiler,
    Ctx,
    RcIter,
};
use jaq_json::Val;
use kube::api::DynamicObject;
use kube::Resource;
use ordered_float::OrderedFloat;
use petgraph::prelude::*;
use rand::distributions::{
    Distribution,
    WeightedIndex,
};
use rand::thread_rng;
use serde::{
    Deserialize,
    Serialize,
};
use sk_core::k8s::GVK;
use sk_core::logging;
use sk_store::{
    ExportedTrace,
    TraceEvent,
    TraceStore,
    TracerConfig,
    TrackedObjectConfig,
};
use tracing::{
    debug,
    error,
    info,
    instrument,
    warn,
};

use crate::contraction_hierarchies::CHEdge;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    num_samples: usize,
    #[arg(short = 'l', long)]
    trace_length: u64,
    #[arg(short = 'e', long, default_value_t = 3)]
    enumeration_steps: u64,
    #[arg(short, long)]
    input_traces: Vec<PathBuf>,
    #[arg(short, long, default_value = "info")]
    verbosity: String,
    #[arg(long, default_value_t = 0.5, value_parser = parse_contraction_strength)]
    contraction_strength: f64,
}

/// Custom parser for `contraction_strength` to enforce range [0.0, 1.0]
fn parse_contraction_strength(s: &str) -> Result<f64, String> {
    let val: f64 = s.parse().map_err(|_| format!("'{s}' isn't a valid float number"))?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!("value must be between 0.0 and 1.0, got: {val}"))
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, Serialize, Deserialize)]
enum ObjectType {
    Synthetic,
    Observed,
}

// I believe this uniquely identifies an object in the cluster.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Serialize)]
struct ObjectKey {
    name: String,
    gvk: GVK,
    // namespace: String, // TODO: add namespace support
}

impl From<&DynamicObject> for ObjectKey {
    fn from(value: &DynamicObject) -> Self {
        let gvk = GVK::from_dynamic_obj(value).unwrap();
        let name = value.meta().name.clone().unwrap();
        ObjectKey { name, gvk }
    }
}

impl Ord for ObjectKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.gvk
            .group
            .cmp(&other.gvk.group)
            .then(self.gvk.version.cmp(&other.gvk.version))
            .then(self.gvk.kind.cmp(&other.gvk.kind))
            .then(self.name.cmp(&other.name))
    }
}

impl PartialOrd for ObjectKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, Serialize)]
pub(crate) struct Node {
    object_type: ObjectType,
    objects: BTreeMap<ObjectKey, DynamicObjectNewType>,
    ts: i64,
}

impl Node {
    #[instrument(level = "debug", skip(self, patch), fields(patch_ts = patch.ts))]
    fn apply_patch(&self, patch: &TraceEvent) -> Result<Node> {
        if patch.ts < self.ts {
            return Err(anyhow::anyhow!("patch is earlier than node timestamp"));
        }

        let mut next_node = self.clone();
        next_node.ts = patch.ts;

        assert_eq!(
            patch
                .applied_objs
                .iter()
                .map(ObjectKey::from)
                .chain(patch.deleted_objs.iter().map(ObjectKey::from))
                .collect::<HashSet<_>>()
                .len(),
            patch.applied_objs.len() + patch.deleted_objs.len(),
            "objects must appear only once across the union of applied and deleted objects in a TraceEvent"
        );

        for obj in &patch.applied_objs {
            let key = ObjectKey::from(obj);

            // if object already exists, merge the fields rather than overwriting
            if let Some(existing_obj) = next_node.objects.get(&key) {
                let existing_json = serde_json::to_value(&existing_obj.dynamic_object).unwrap();
                let new_json = serde_json::to_value(obj).unwrap();

                // start with existing object and update with new fields
                if let (serde_json::Value::Object(existing_map), serde_json::Value::Object(new_map)) =
                    (existing_json.clone(), new_json)
                {
                    let mut merged = existing_map;
                    for (k, v) in new_map {
                        merged.insert(k, v);
                    }

                    if let Ok(merged_obj) = serde_json::from_value(serde_json::Value::Object(merged)) {
                        next_node
                            .objects
                            .insert(key, DynamicObjectNewType { dynamic_object: merged_obj });
                    } else {
                        next_node.objects.insert(key, obj.clone().into());
                    }
                } else {
                    panic!("failed to merge objects: {existing_json:?}");
                }
            } else {
                // if object doesn't exist yet, simply insert it
                next_node.objects.insert(key, obj.clone().into());
            }
        }

        for obj in &patch.deleted_objs {
            next_node.objects.remove(&ObjectKey::from(obj));
        }

        Ok(next_node)
    }

    /// Creates a new state with no active objects.
    #[instrument]
    fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            ts: 0,
            object_type: ObjectType::Synthetic,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Patch {
    trace_event: TraceEvent,
}

impl From<TraceEvent> for Patch {
    fn from(value: TraceEvent) -> Self {
        Patch { trace_event: value }
    }
}

impl std::ops::Deref for Patch {
    type Target = TraceEvent;

    fn deref(&self) -> &Self::Target {
        &self.trace_event
    }
}

impl Hash for Patch {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ordered_hash(&serde_json::to_value(&self.trace_event).unwrap()).hash(state);
    }
}

impl Eq for Patch {}

impl PartialEq for Patch {
    fn eq(&self, other: &Self) -> bool {
        ordered_eq(
            &serde_json::to_value(&self.trace_event).unwrap(),
            &serde_json::to_value(&other.trace_event).unwrap(),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
struct Action {
    patch: Patch,
    message: Option<String>,
    probability: OrderedFloat<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
struct Edge {
    object_type: ObjectType,
    action: Action,
}

impl Distance for Edge {
    fn probability(&self) -> OrderedFloat<f64> {
        self.action.probability
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DynamicObjectNewType {
    dynamic_object: DynamicObject,
}

impl From<DynamicObject> for DynamicObjectNewType {
    fn from(value: DynamicObject) -> Self {
        Self { dynamic_object: value }
    }
}

impl From<DynamicObjectNewType> for DynamicObject {
    fn from(value: DynamicObjectNewType) -> Self {
        value.dynamic_object
    }
}

impl PartialEq for DynamicObjectNewType {
    fn eq(&self, other: &Self) -> bool {
        ordered_eq(
            &serde_json::to_value(&self.dynamic_object).unwrap(),
            &serde_json::to_value(&other.dynamic_object).unwrap(),
        )
    }
}

impl Eq for DynamicObjectNewType {}

impl std::hash::Hash for DynamicObjectNewType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ordered_hash(&serde_json::to_value(&self.dynamic_object).unwrap()).hash(state);
    }
}

struct Simulation;

impl Simulation {
    #[instrument(skip(next_action_fn, input_traces), fields(input_traces_count = input_traces.len()))]
    fn run<F>(next_action_fn: F, input_traces: Vec<Vec<TraceEvent>>) -> Result<()>
    where
        F: Fn(&Node) -> Vec<Action> + Clone + Sync,
    {
        let args = Cli::parse();
        let output_dir = create_timestamped_output_dir()?;

        // Get the first event from the first trace to use as starting state for all traces
        let first_trace_event = input_traces.first().and_then(|trace| trace.first()).cloned();

        if first_trace_event.is_none() {
            warn!("No initial trace event found in input traces. Generated traces will not have a consistent starting state.");
        }

        // Phase 1: Graph Construction
        let (mut state_graph, mut node_to_index) = Self::construct_graph(input_traces)?;
        info!(
            "Constructed initial graph from traces: {} nodes, {} edges",
            state_graph.node_count(),
            state_graph.edge_count()
        );

        // Phase 2: Graph Expansion
        Self::expand_graph(&mut state_graph, &mut node_to_index, next_action_fn, args.enumeration_steps)?;
        info!("Expanded graph: {} nodes", state_graph.node_count());
        info!("Expanded graph: {} nodes, {} edges", state_graph.node_count(), state_graph.edge_count());

        // Write expanded graph before contraction
        let graphable_expanded = state_graph.map(
            |i, n| format!("{} -- {:?}", i.index(), n.object_type),
            |i, e| {
                format!(
                    "{} -- {:?} {}",
                    i.index(),
                    e.object_type,
                    match &e.action.message {
                        Some(m) => format!(" {m}"),
                        None => String::new(),
                    },
                )
            },
        );
        let state_graph_dot = Dot::new(&graphable_expanded);
        let state_graph_str = format!("{state_graph_dot}");
        write_dot_file(&output_dir, "expanded_state_graph.dot", &state_graph_str)?;


        let processed_core_graph = Self::contract_graph(state_graph, &output_dir)?;

        let graphable_core = processed_core_graph.map(
            |i, n| format!("{} -- {:?}", i.index(), n.object_type),
            |i, e| {
                format!(
                    "{} -- {:?} {}",
                    i.index(),
                    e.object_type,
                    match &e.action.message {
                        Some(m) => format!(" {m}"),
                        None => String::new(),
                    },
                )
            },
        );
        let core_graph_dot = Dot::new(&graphable_core);
        let core_graph_str = format!("{core_graph_dot}");
        write_dot_file(&output_dir, "core_graph.dot", &core_graph_str)?;

        Self::generate_traces(
            &processed_core_graph,
            &output_dir,
            args.num_samples,
            args.trace_length,
            first_trace_event,
        )?;

        Ok(())
    }

    #[instrument(skip(input_traces), fields(input_traces_count = input_traces.len()))]
    fn construct_graph(input_traces: Vec<Vec<TraceEvent>>) -> Result<(DiGraph<Node, Edge>, HashMap<Node, NodeIndex>)> {
        let mut state_graph = DiGraph::new();
        let mut node_to_index: HashMap<Node, NodeIndex> = HashMap::new();

        // Import traces and add to state graph
        for trace in input_traces {
            let mut trace = trace.into_iter();

            let Some(first_event) = trace.next() else {
                continue;
            };

            for deleted_object in &first_event.deleted_objs {
                warn!("Ignoring deleted object in first event of trace: {:?}", deleted_object);
            }

            let objects = first_event
                .applied_objs
                .iter()
                .map(|obj| (ObjectKey::from(obj), obj.clone().into()))
                .collect();

            let mut current_node = Node {
                object_type: ObjectType::Observed,
                objects,
                ts: first_event.ts,
            };

            // Turn each event of the trace into a Node and add Edges
            for event in trace {
                let next_node = current_node.apply_patch(&event)?;

                let edge = Edge {
                    object_type: ObjectType::Observed,
                    action: Action {
                        patch: event.into(),
                        probability: OrderedFloat(1.0),
                        message: Some("generated from trace".to_string()), /* TODO: include file of origin for multi
                                                                            * trace import disambiguation, perhaps in
                                                                            * an enum field of{Synthetic, Observed
                                                                            * {file: "..."}} */
                    },
                };

                let current_node_index = *node_to_index
                    .entry(current_node.clone())
                    .or_insert_with(|| state_graph.add_node(current_node.clone()));

                let next_node_index = *node_to_index
                    .entry(next_node.clone())
                    .or_insert_with(|| state_graph.add_node(next_node.clone()));

                state_graph.update_edge(current_node_index, next_node_index, edge); // Use update_edge to avoid duplicates if trace revisits state
                current_node = next_node;
            }
        }
        Ok((state_graph, node_to_index))
    }

    #[instrument(skip(graph), fields(nodes = graph.node_count(), edges = graph.edge_count()))]
    fn normalize_edge_probabilities(graph: &mut DiGraph<Node, Edge>) {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        // Process each node in the graph
        for node_idx in graph.node_indices() {
            let outgoing_edge_ids: Vec<_> = graph
                .edges_directed(node_idx, Direction::Outgoing)
                .map(|edge| (edge.id(), edge.weight().action.probability.into_inner()))
                .collect();

            if outgoing_edge_ids.is_empty() {
                continue; // Skip nodes with no outgoing edges
            }

            let total_probability: f64 = outgoing_edge_ids.iter().map(|(_, prob)| *prob).sum();

            if total_probability == 0.0 {
                debug!("Node {:?} has outgoing edges but total probability is 0", node_idx);
                continue;
            }

            for (edge_idx, prob) in outgoing_edge_ids {
                if let Some(edge_weight) = graph.edge_weight_mut(edge_idx) {
                    let normalized_prob = prob / total_probability;
                    edge_weight.action.probability = OrderedFloat(normalized_prob);
                }
            }
        }
    }

    #[instrument(skip(graph, node_to_index, next_action_fn), fields(initial_nodes = graph.node_count(), enumeration_steps))]
    fn expand_graph<F>(
        graph: &mut DiGraph<Node, Edge>,
        node_to_index: &mut HashMap<Node, NodeIndex>,
        next_action_fn: F,
        enumeration_steps: u64,
    ) -> Result<()>
    where
        F: Fn(&Node) -> Vec<Action> + Sync,
    {
        let starting_nodes: Vec<NodeIndex> = graph.node_indices().collect(); // Collect indices before iteration
        let mut bfs_queue: VecDeque<(u64, NodeIndex)> = VecDeque::new();
        for node_idx in starting_nodes {
            bfs_queue.push_back((0, node_idx)); // Start depth at 0 for initial nodes
        }
        let mut current_layer: Vec<NodeIndex> = graph.node_indices().collect(); // Start with all existing nodes
        let mut next_layer: Vec<NodeIndex> = Vec::new();
        let mut visited_in_expansion = HashSet::new(); // Track nodes visited *during this expansion*
        let mut depth = 0u64;

        while !current_layer.is_empty() && depth < enumeration_steps {
            info!("Expanding graph at depth: {}. Nodes in current layer: {}", depth, current_layer.len());

            // Create a progress bar for this layer
            let pb = ProgressBar::new(current_layer.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} nodes ({percent}%) - {msg}")
                .unwrap()
                .progress_chars("#>-"));
            pb.set_message(format!("Layer {depth}"));

            // Process current layer nodes in parallel and collect results
            let current_layer_copy = std::mem::take(&mut current_layer); // Take ownership of current layer

            // First, filter out already visited nodes (to avoid redundant work)
            let to_process: Vec<_> = current_layer_copy
                .into_iter()
                .filter(|&idx| !visited_in_expansion.contains(&idx))
                .collect();

            // Mark these nodes as visited now (before processing)
            for &node_idx in &to_process {
                visited_in_expansion.insert(node_idx);
            }

            let total_nodes = to_process.len();

            // Define a thread-safe counter for progress updates
            let counter = std::sync::atomic::AtomicUsize::new(0);

            // Process nodes in parallel and collect results
            let results: Vec<_> = to_process
                .par_iter()
                .map(|&current_idx| {
                    // Update progress (approximately)
                    let idx = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if idx % 10 == 0 || idx == total_nodes - 1 {
                        pb.set_position((idx as u64).min(total_nodes as u64));
                    }
                    
                    let current_node_data = if let Some(node) = graph.node_weight(current_idx) {
                        node.clone()
                    } else {
                        warn!(
                            "Node index {:?} not found in graph during expansion at depth {}",
                            current_idx, depth
                        );
                        return (current_idx, vec![], 0); // Return empty results for this node
                    };
                    
                    let actions = next_action_fn(&current_node_data);

                    let mut node_results = Vec::new();
                    let mut added_count = 0;

                    for action in actions {
                        let next_node_data = match current_node_data.apply_patch(&action.patch) {
                            Ok(mut node) => {
                                // Mark nodes generated during expansion as Synthetic
                                node.object_type = ObjectType::Synthetic;
                                node
                            },
                            Err(e) => {
                                warn!(
                                    "Skipping invalid action generated by next_action_fn: {:?}. Error: {}",
                                    action.message, e
                                );
                                continue;
                            },
                        };

                        node_results.push((next_node_data, action, current_idx));
                        added_count += 1;
                    }

                    (current_idx, node_results, added_count)
                })
                .collect();

            // Now serially update the graph with the results
            let mut total_added = 0;
            let mut nodes_to_add_next_layer = Vec::new();

            for (current_idx, node_results, added_count) in results {
                total_added += added_count;

                for (next_node_data, action, _) in node_results {
                    let next_idx = {
                        let entry = node_to_index.entry(next_node_data.clone());
                        *entry.or_insert_with(|| {
                            let new_idx = graph.add_node(next_node_data);
                            debug!(
                                "Added new synthetic node {:?} (derived from {:?}) at depth {}",
                                new_idx, current_idx, depth
                            );
                            new_idx
                        })
                    };

                    // Add edge regardless of whether the node was new or existing
                    let edge = Edge { object_type: ObjectType::Synthetic, action };
                    debug!(
                        "Adding/Updating synthetic edge from {:?} to {:?} (action: '{:?}')",
                        current_idx,
                        next_idx,
                        edge.action.message.as_deref().unwrap_or("N/A")
                    );
                    graph.update_edge(current_idx, next_idx, edge);

                    // If this neighbor hasn't been visited *in this expansion* and we are within depth limit,
                    // add it to the next layer to be processed.
                    if !visited_in_expansion.contains(&next_idx) && (depth + 1) < enumeration_steps {
                        nodes_to_add_next_layer.push(next_idx);
                    }
                }
            }

            // Deduplicate nodes for next layer (could happen if multiple nodes in current_layer point to the
            // same next_idx)
            for node_idx in nodes_to_add_next_layer {
                if !next_layer.contains(&node_idx) {
                    debug!("Adding node {:?} to next layer (depth {})", node_idx, depth + 1);
                    next_layer.push(node_idx);
                }
            }

            // Finalize the progress bar
            pb.finish_with_message(format!(
                "Layer {} completed - {} nodes in next layer, added {} nodes",
                depth,
                next_layer.len(),
                total_added
            ));

            // Prepare for the next iteration
            depth += 1;
            current_layer = std::mem::take(&mut next_layer); // next_layer becomes current_layer
                                                             // next_layer is now empty, ready for
                                                             // the next depth
        }

        // Normalize outgoing edge probabilities for each node after expansion is complete
        info!("Normalizing edge probabilities after expansion");
        Self::normalize_edge_probabilities(graph);

        info!("Expansion finished at depth {}. Final graph node count: {}", depth, graph.node_count());
        Ok(())
    }

    // TODO consider protecting nodes that are originally in the trace from being contracted
    #[instrument(skip(state_graph), fields(nodes = state_graph.node_count()))]
    fn contract_graph(state_graph: DiGraph<Node, Edge>, output_dir: &PathBuf) -> Result<DiGraph<Node, Edge>> {
        let args = Cli::parse(); // Need access to args for contraction_strength
        let total_nodes = state_graph.node_count();

        // conduct CH
        let heuristic_graph = crate::contraction_hierarchies::HeuristicGraph::new(state_graph.clone());
        let contraction_order = heuristic_graph.contraction_order();

        let num_contractions_target = (total_nodes as f64 * args.contraction_strength).round() as usize;

        // Ensure we don't try to contract more nodes than available in the order
        let core_graph_iteration = num_contractions_target.min(contraction_order.len());

        info!(
            "Targeting core graph after {} contractions (strength: {}, total nodes: {})",
            core_graph_iteration, args.contraction_strength, total_nodes
        );

        let mut ch = crate::contraction_hierarchies::CH::new(state_graph.clone(), contraction_order.into_iter()); // Pass full order

        let pb = ProgressBar::new(core_graph_iteration as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.yellow/blue}] {pos}/{len} contractions ({percent}%) - {msg}")
            .unwrap()
            .progress_chars("=>-"));
        pb.set_message("Starting graph contraction");

        let mut contracted_count = 0;

        // Get the core graph at the calculated iteration with progress tracking
        let core_graph_result = ch.core_graph_with_progress(core_graph_iteration, |current_iteration| {
            contracted_count = current_iteration;
            pb.set_position(current_iteration as u64);
            pb.set_message(format!("Contracted {current_iteration} nodes"));
        })?;

        pb.finish_with_message(format!("Contraction complete: {contracted_count} nodes contracted"));

        info!(
            "Obtained core graph with {} nodes, {} edges",
            core_graph_result.node_count(),
            core_graph_result.edge_count()
        );


        // convert from the CH-internal graph with CH-specific metadata to the raw core graph
        let processed_graph = core_graph_result.filter_map(|_node_idx, node| {
            match node {
                 // Keep original nodes, map Contracted nodes back to original if needed (core_graph should handle this based on iteration)
                CHNode::Original { node } | CHNode::Contracted { node, .. } => Some(node.clone()),
            }
        }, |edge_idx, edge| {
             match edge {
                CHEdge::Original { edge } => Some(edge.clone()),
                CHEdge::Shortcut { edges, nodes, .. } => {
                     // Check if this shortcut should be included based on the core_graph logic (it should be if returned by core_graph)
                     // The processing logic to reconstruct the action remains the same.
                     let message_opt = Some(edges.iter()
                        .filter_map(|edge| edge.action.message.clone())
                        .collect::<Vec<_>>()
                        .join(" -> "))
                        .filter(|s| !s.is_empty());

                     // Need the graph the edge belongs to, to get node data - use core_graph_result
                     let graph_ref = &core_graph_result;
                    // Calculate the resulting state change of the shortcut
                     let start_node_idx = nodes.first()?;
                     let start_node_data = match &graph_ref[*start_node_idx] {
                         // We expect Original or Contracted here from core_graph output
                         CHNode::Original { node } | CHNode::Contracted { node, .. } => node,
                     };

                    let mut current_node_state = start_node_data.clone();
                    let mut final_ts = start_node_data.ts;

                    // Apply the patches from the *original* edges stored in the shortcut
                    for shortcut_edge_original in edges {
                        match current_node_state.apply_patch(&shortcut_edge_original.action.patch) {
                            Ok(next_node) => {
                                current_node_state = next_node;
                                final_ts = current_node_state.ts;
                            }
                            Err(e) => {
                                error!("Error applying patch from original edge within shortcut {:?}: {}. Skipping shortcut edge.", edge_idx, e);
                                return None;
                            }
                        }
                    }

                    // Use the *original* start node state and the *final* state after all shortcut patches
                    let (applied_objs, deleted_objs) = diff_objects(&start_node_data.objects, &current_node_state.objects);

                    let combined_patch = TraceEvent {
                        ts: final_ts,
                        applied_objs,
                        deleted_objs,
                    };

                    // Calculate probability from component edges instead of using stored surprisal
                    let probability = edges.iter()
                        .map(|edge| edge.action.probability.into_inner())
                        .product::<f64>()
                        .into();

                    Some(Edge {
                        object_type: ObjectType::Synthetic,
                        action: Action {
                            patch: combined_patch.into(),
                            probability,
                            message: message_opt,
                        },
                    })
                },
                CHEdge::Orphaned { .. } => None, // Remove orphaned edges
            }
        });

        info!(
            "Processed core graph: {} nodes, {} edges",
            processed_graph.node_count(),
            processed_graph.edge_count()
        );

        // visualize the processed graph (the one used for sampling)
        let processed_graph_pretty = processed_graph.map(
            |i, n| format!("{} -- {:?}", i.index(), n.object_type),
            |i, e| {
                format!(
                    "{} -- {:?} P={:.2e} {}", // Show probability
                    i.index(),
                    e.object_type,
                    e.action.probability.into_inner(),
                    match &e.action.message {
                        Some(m) => m,
                        None => "[no message]",
                    },
                )
            },
        );
        let processed_graph_dot = Dot::new(&processed_graph_pretty);
        write_dot_file(output_dir, "processed_sample_graph.dot", &format!("{processed_graph_dot}"))?;

        Ok(processed_graph)
    }

    #[instrument(skip(graph, initial_event), fields(nodes = graph.node_count(), num_samples, trace_length))]
    fn generate_traces(
        graph: &DiGraph<Node, Edge>,
        output_dir: &PathBuf,
        num_samples: usize,
        trace_length: u64,
        initial_event: Option<TraceEvent>,
    ) -> Result<()> {
        // TODO: provide mechanism to configure start node, whether the start of the original trace(s), the
        // end of the orirginal trace(s), or something else entirely TODO: If we protect original
        // nodes from contraction, we can be more specific about where we want to start
        let start_node = if let Some(idx) = graph.node_indices().find(|&idx| graph.neighbors(idx).next().is_some()) {
            info!("Starting trace generation from the first node with neighbors: {:?}", idx);
            idx
        } else {
            error!("Cannot generate traces: No node with outgoing edges found in graph.");
            return Ok(()); // Nothing to do if graph is empty
        };

        // Sample walks through the graph
        info!("Starting trace sampling from node index: {:?}", start_node);
        // If there's no initial event, increase the walk length by 1 to compensate
        let adjusted_length = if initial_event.is_some() {
            trace_length
        } else {
            info!("No initial event provided, increasing walk length by 1 to compensate");
            trace_length + 1
        };
        let walks = Self::walks_with_sampling(graph, start_node, adjusted_length, num_samples);
        info!("Generated {} walks", walks.len());

        // Create a progress bar for tracking
        let pb = ProgressBar::new(walks.len() as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.green/blue}] {pos}/{len} traces ({percent}%) - {msg}")
            .unwrap()
            .progress_chars("#>-"));
        pb.set_message("Generating trace files");

        // Create a thread-safe counter for progress updates
        let counter = std::sync::atomic::AtomicUsize::new(0);

        // Process walks in parallel
        let results: Vec<_> = walks
            .par_iter()
            .enumerate()
            .map(|(i, walk)| {
                // Update progress counter
                let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if count % 10 == 0 || count == walks.len() - 1 {
                    pb.set_position(count as u64);
                    pb.set_message(format!("Generated {}/{} traces", count, walks.len()));
                }

                if walk.len() <= 1 {
                    return Ok(format!("Skipped walk {i} (too short)"));
                }

                let trace_path = output_dir.join(format!("trace_{i}.json"));
                match Self::generate_trace_file(graph, walk, &trace_path, initial_event.clone()) {
                    Ok(_) => Ok(format!("Generated trace_{i}.json")),
                    Err(e) => Err(format!("Failed to generate trace_{i}.json: {e}")),
                }
            })
            .collect();

        // Check results and report errors
        let mut success_count = 0;
        let mut error_count = 0;

        for result in results {
            match result {
                Ok(msg) => {
                    if msg.starts_with("Generated") {
                        success_count += 1;
                    }
                },
                Err(err) => {
                    error!("{}", err);
                    error_count += 1;
                },
            }
        }

        pb.finish_with_message(format!(
            "Completed trace generation: {success_count} successful, {error_count} failed",
        ));

        if error_count > 0 {
            warn!("{} traces failed to generate", error_count);
        }

        Ok(())
    }

    #[instrument(skip(graph, walk, initial_event))]
    fn generate_trace_file(
        graph: &DiGraph<Node, Edge>,
        walk: &[NodeIndex],
        path: &PathBuf,
        initial_event: Option<TraceEvent>,
    ) -> Result<()> {
        let mut trace_events = Vec::new();

        if let Some(event) = initial_event {
            info!("Prepending initial trace event to generated trace");
            trace_events.push(event);
        }

        // iterate through pairs of nodes in the walk to find edges
        for window in walk.windows(2) {
            let u = window[0];
            let v = window[1];
            if let Some(edge_index) = graph.find_edge(u, v) {
                if let Some(edge) = graph.edge_weight(edge_index) {
                    trace_events.push(edge.action.patch.trace_event.clone());
                } else {
                    // Should not happen if find_edge succeeded
                    error!("Edge index {:?} found but has no weight between {:?} and {:?}", edge_index, u, v);
                    return Err(anyhow::anyhow!("Inconsistent graph state: edge weight missing"));
                }
            } else {
                // This indicates a potential issue either in walk generation or the graph structure
                error!("No edge found between consecutive nodes {:?} and {:?} in walk. Walk: {:?}", u, v, walk);
                // Depending on desired behavior, we could either error out or just log and continue
                return Err(anyhow::anyhow!("Invalid walk: missing edge between consecutive nodes"));
            }
        }

        if trace_events.is_empty() {
            return Err(anyhow::anyhow!("Generated walk resulted in zero trace events"));
        }

        let exported_trace = tracestore_from_events(&trace_events);

        let file = File::create(path)?; // Propagate error
        serde_json::to_writer_pretty(&file, &exported_trace)?; // Propagate error
        Ok(())
    }

    #[instrument(skip(graph), fields(start_node = start_node.index(), walk_length, num_samples))]
    fn walks_with_sampling(
        graph: &DiGraph<Node, Edge>,
        start_node: NodeIndex,
        walk_length: u64,
        num_samples: usize,
    ) -> Vec<Vec<NodeIndex>> {
        // Use parallel iteration to generate walks in parallel
        (0..num_samples)
            .into_par_iter()
            .map(|sample_idx| {
                let mut rng = thread_rng();
                info!("Sample {}: starting walk from node {:?}", sample_idx, start_node);
                let mut current_walk = vec![start_node];
                let mut current_node = start_node;

                for step in 1..walk_length {
                    debug!("Sample {} step {}: at node {:?}", sample_idx, step, current_node);
                    let neighbors: Vec<_> = graph.neighbors(current_node).collect();
                    debug!("Sample {} step {}: found neighbors {:?}", sample_idx, step, neighbors);
                    if neighbors.is_empty() {
                        info!("Sample {} step {}: no neighbors, ending walk", sample_idx, step);
                        break;
                    }

                    let weights: Vec<f64> = neighbors
                        .iter()
                        .filter_map(|&n| graph.find_edge(current_node, n)) // Find edge index first
                        .filter_map(|edge_idx| graph.edge_weight(edge_idx)) // Get edge weight
                        .map(|edge| edge.action.probability.into_inner().max(0.0)) // Ensure probability is non-negative
                        .collect();
                    debug!("Sample {} step {}: weights {:?}", sample_idx, step, weights);

                    let total_weight: f64 = weights.iter().sum();
                    if total_weight <= 0.0 {
                        info!(
                            "Sample {} step {}: no outgoing edges with positive probability at node {:?}",
                            sample_idx, step, current_node
                        );
                        break;
                    }

                    let dist = match WeightedIndex::new(&weights) {
                        Ok(d) => d,
                        Err(e) => {
                            error!(
                                "Failed to create WeightedIndex at node {:?} (weights: {:?}): {}. Stopping walk.",
                                current_node, weights, e
                            );
                            break; // Stop walk if distribution fails
                        },
                    };
                    
                    let next_neighbor_index = dist.sample(&mut rng);
                    // Ensure the sampled index is valid for the neighbors list
                    let Some(&next_node) = neighbors.get(next_neighbor_index) else {
                        error!(
                            "WeightedIndex sampled invalid index {} for neighbors (len {}). Stopping walk.",
                            next_neighbor_index,
                            neighbors.len()
                        );
                        break;
                    };

                    current_walk.push(next_node);
                    current_node = next_node;
                }
                info!("Sample {} complete: walk length {}", sample_idx, current_walk.len());
                current_walk
            })
            .collect()
    }
}

#[instrument(skip(node), fields(object_count = node.objects.len()))]
fn next_action_fn(node: &Node) -> Vec<Action> {
    let double_memory_script = r#" [
        range(0; length) as $i |
        [ .[] | . ] |
        .[$i].spec.template.spec.containers[0].resources.requests.memory |=
            (capture("(?<num>^[0-9]+)(?<unit>.*)") | "\((.num | tonumber) * 2)\(.unit)")
        ]
        "#;

    let halve_memory_script = r#"
        [
        range(0; length) as $i |
        ([ .[] | . ] |
        .[$i].spec.template.spec.containers[0].resources.requests.memory |=
            (capture("(?<num>^[0-9]+)(?<unit>.*)")
            | ((.num | tonumber) / 2) as $halved_value
            | "\(($halved_value | floor))\(.unit)")
        ) as $modified_list
        |
        ($modified_list[$i].spec.template.spec.containers[0].resources.requests.memory
            | capture("(?<num>^[0-9.]+)(?<unit>.*)")
            | (.num | tonumber) as $num
            | .unit as $unit
            | if ($unit == "Gi" and ($num * 1024) < 256) or ($unit == "Mi" and $num < 256)
            then empty else $modified_list end
        )
        ]
        "#;

    let increment_replica_script = r"
        [
        range(0; length) as $i |
        [ .[] | . ] |
        .[$i].spec.replicas |= . + 1
        ]
        ";

    let decrement_replica_script = r#"
        [
        range(0; length) as $i |
        try (
            [ .[] | . ] |
            if .[$i].spec.replicas > 1
            then .[$i].spec.replicas |= . - 1
            else error("Cannot decrement replica count")
            end
        ) catch empty
        ]
        "#;

    let double_cpu_script = r#"
        [
        range(0; length) as $i |
        try (
            [ .[] | . ] |
            .[$i].spec.template.spec.containers[0].resources.requests.cpu |= (
            capture("(?<num>^[0-9]+(?:\\.[0-9]+)?)(?<unit>m?)") |
            (
                (if .unit == "m"
                then (.num | tonumber)
                else (.num | tonumber * 1000)
                end * 2) as $mcores

                | if ($mcores % 1000 == 0)
                then "\($mcores / 1000)"
                else "\($mcores)m"
                end
            )
            )
        ) catch empty
        ]
    "#;

    let halve_cpu_script = r#"
        [
        range(0; length) as $i |
        [ .[] | . ] |
        try (
            .[$i].spec.template.spec.containers[0].resources.requests.cpu |= (
            capture("(?<num>^[0-9]+(?:\\.[0-9]+)?)(?<unit>m?)") |
            (
                (if .unit == "m"
                then (.num | tonumber)
                else (.num | tonumber * 1000)
                end) as $mcores

                | if ($mcores / 2) >= 1
                then
                    ($mcores / 2) as $new_mcores
                    | if .unit == "m"
                    then "\($new_mcores | floor)m"
                    else "\($new_mcores / 1000)"
                    end
                else error("Cannot decrement CPU")
                end
            )
            )
        ) catch .
        ]
    "#;

    let action_scripts = [
        ("double_memory", double_memory_script),
        ("halve_memory", halve_memory_script),
        ("increment_replicas", increment_replica_script),
        ("decrement_replicas", decrement_replica_script),
        ("double_cpu", double_cpu_script),
        ("halve_cpu", halve_cpu_script),
    ];

    let objects_json =
        serde_json::to_value(node.objects.values().map(|d| d.dynamic_object.clone()).collect::<Vec<_>>())
            .expect("Failed to serialize objects to JSON");

    action_scripts
        .into_iter()
        .flat_map(|(action_message, jq_script)| {
            let message = Some(action_message.to_string());
            debug!("message: {:?}", message);

            let program = load::File { code: jq_script, path: () };

            let arena = Arena::default();
            let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));

            let modules = match loader.load(&arena, program) {
                Ok(modules) => modules,
                Err(err) => {
                    error!("Failed to load jaq script for action '{}': {:?}", action_message, err);
                    return Vec::new();
                },
            };

            let filter = match Compiler::default()
                .with_funs(jaq_std::funs().chain(jaq_json::funs()))
                .compile(modules)
            {
                Ok(filter) => filter,
                Err(err) => {
                    warn!("Failed to compile jaq script for action '{}': {:?}", action_message, err);
                    return Vec::new();
                },
            };

            let inputs = RcIter::new(core::iter::empty());

            let jq_output = filter.run((Ctx::new([], &inputs), Val::from(objects_json.clone())));

            let mut results = Vec::new();
            for result in jq_output {
                match result {
                    Ok(val) => match serde_json::from_value::<Vec<Vec<DynamicObject>>>(val.into()) {
                        Ok(dynamic_object_list) => {
                            results.push(dynamic_object_list);
                        },
                        Err(e) => {
                            error!(
                                "Error deserializing jaq result for action '{}': {}. Input was {} objects",
                                action_message,
                                e,
                                node.objects.len()
                            );
                        },
                    },
                    Err(e) => {
                        error!("Error running jaq filter for action '{}': {}", action_message, e);
                    },
                }
            }

            let objects_list = results
                .into_iter()
                .flat_map(|dynamic_object_list| {
                    dynamic_object_list
                        .into_iter()
                        .map(|dynamic_object_list| {
                            dynamic_object_list
                                .into_iter()
                                .map(|obj| (ObjectKey::from(&obj), DynamicObjectNewType { dynamic_object: obj }))
                                .collect::<BTreeMap<_, _>>()
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();

            objects_list
                .into_iter()
                .map(|objects| {
                    let (applied_objs, deleted_objs) = diff_objects(&node.objects, &objects);

                    Action {
                        patch: TraceEvent { ts: node.ts + 1, applied_objs, deleted_objs }.into(),
                        probability: OrderedFloat(1.0), // TODO
                        message: message.clone(),
                    }
                })
                .collect()
        })
        .collect()
}

#[instrument]
fn create_timestamped_output_dir() -> Result<PathBuf> {
    let base_dir = PathBuf::from("runs");

    std::fs::create_dir_all(&base_dir)?;
    let now: DateTime<Utc> = SystemTime::now().into();

    // filesystem compatibility
    let timestamp = now.to_rfc3339().replace([':', '.'], "-");

    let output_dir = base_dir.join(timestamp);
    std::fs::create_dir_all(&output_dir)?;

    let metadata = serde_json::json!({
        "timestamp": now.to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "command_args": std::env::args().collect::<Vec<_>>(),
    });

    let metadata_path = output_dir.join("metadata.json");
    let mut file = File::create(metadata_path)?;
    file.write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;

    Ok(output_dir)
}

#[instrument(skip(dot_content))]
fn write_dot_file(output_dir: &PathBuf, filename: &str, dot_content: &str) -> Result<PathBuf> {
    let file_path = output_dir.join(filename);
    let mut file = File::create(&file_path)?;
    write!(file, "{dot_content}")?;

    debug!("Graph written to: {}", file_path.display());

    Ok(file_path)
}

#[instrument]
fn main() -> Result<()> {
    let args = Cli::parse();

    logging::setup(&args.verbosity);

    let input_traces = args
        .input_traces
        .iter()
        .map(|path| {
            let file = File::open(path).unwrap();
            let trace: ExportedTrace = rmp_serde::from_read(&file).unwrap();
            trace.events()
        })
        .collect();


    Simulation::run(next_action_fn, input_traces)
}


#[instrument(skip(events), fields(event_count = events.len()))]
fn tracestore_from_events(events: &[TraceEvent]) -> TraceStore {
    let config = TracerConfig {
        tracked_objects: HashMap::from([(
            GVK::new("apps", "v1", "Deployment"),
            TrackedObjectConfig {
                track_lifecycle: false,
                pod_spec_template_path: None,
            },
        )]),
    };

    let mut trace_store = TraceStore::new(config);


    for (ts, trace_event) in events.iter().enumerate() {
        for obj in trace_event.applied_objs.clone() {
            trace_store.create_or_update_obj(&obj, ts as i64, None).unwrap(); // TODO check on
                                                                              // maybe_old_hash
        }

        for obj in trace_event.deleted_objs.clone() {
            trace_store.delete_obj(&obj, ts as i64).unwrap();
        }
    }

    trace_store
}

// helper to compute applied (created/updated) and deleted objects using daft's diff on BTreeMaps in
// accordance with trace event format
fn diff_objects<'a>(
    before: &'a BTreeMap<ObjectKey, DynamicObjectNewType>,
    after: &'a BTreeMap<ObjectKey, DynamicObjectNewType>,
) -> (Vec<DynamicObject>, Vec<DynamicObject>) {
    let diff = before.diff(after);

    // Added keys => created objects, push the new value.
    let mut applied: Vec<DynamicObject> = diff.added.values().map(|v| v.dynamic_object.clone()).collect();

    // Removed keys => deleted objects, push the old value.
    let deleted: Vec<DynamicObject> = diff.removed.values().map(|v| v.dynamic_object.clone()).collect();

    // Modified => same key present but value changed; treat as updated, push the *after* value.
    applied.extend(diff.modified_values().map(|leaf| leaf.after.dynamic_object.clone()));

    (applied, deleted)
}

impl Diffable for DynamicObjectNewType {
    type Diff<'daft> = daft::Leaf<&'daft DynamicObjectNewType>;

    // Stop diffing at a leaf

    fn diff<'daft>(&'daft self, other: &'daft Self) -> Self::Diff<'daft> {
        daft::Leaf { before: self, after: other }
    }
}
