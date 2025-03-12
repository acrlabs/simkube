#![deny(rustdoc::broken_intra_doc_links)]
//! `sk-gen` is a CLI tool for generating synthetic trace data for SimKube.
//!
//! # Overview:
//! ## Core types
//! [`Node`] represents a cluster state, containing a map from unique names to active
//! Kubernetes objects. `Node` implements [`Eq`] and [`Hash`], which we use to ensure that
//! equivalent `Node`s are not duplicated in the graph.
//!
//! [`ObjectAction`] (e.g. `CreateObject`, `DeleteObject`) can be performed on individual objects.
//!
//! [`ClusterAction`] is an object name paired with a [`ObjectAction`] to execute
//! WARNING: Not all `ObjectAction`s are valid for every object
//!       Similarly Not all `ClusterAction`s are valid for every `Node`
//!       For instance, we cannot delete an object that does not exist.
//!
//! [`TraceEvent`] represents the Kubernetes API call which corresponds to a `ClusterAction`.
//!
//! [`Edge`] stores both a `ClusterAction` and the corresponding `TraceEvent`.
//!
//! [`Trace`] is a sequence of [`TraceEvent`]s along with some additional metadata. A `Trace` is
//! read by SimKube to drive a simulation.
//!
//!
//! ## The graph
//!
//! The Kubernetes cluster state graph is represented as a [`ClusterGraph`]. Walks of this graph map
//! 1:1 to traces which can be read by SimKube.
//!
//! ### Parameters
//! - [`trace_length`](Cli::trace_length): we construct the graph so as to contain all walks of
//!   length `trace_length` starting from the initial `Node`.
//! - `starting_state`: The initial [`Node`] from which to start the graph construction. We
//!   presently use a `Node` with no active objects.
//! - `candidate_objects`: A map from unique object names to corresponding initial
//!   object configurations which are added whenever a `CreateObject` action is
//!   performed. We generate candidate objects as `obj-1`, `obj-2`, etc. according to the
//!   [`object_count`](Cli::object_count) argument.
//!
//! ### Construction
//! - Starting from an initial [`Node`] with no active objects, perform a breadth-first search.
//! - For each node visited:
//!   - Construct every [`ClusterAction`] applicable to the current `Node`, filtering for only those
//!     which produce a valid next `Node`.
//!   - Construct an [`Edge`] from the current `Node` to the next valid `Node`, recording both the
//!     `ClusterAction` and the corresponding `TraceEvent`.
//!   - Continue to a depth of `trace_length - 1` actions, such that the graph contains all walks on
//!     `trace_length` nodes from the initial `Node`.
//!
//! ## Extracting traces from the graph
//!
//!
//! [`Trace`] instances are obtained from the graph by enumerating all walks of length
//! `trace_length` through the graph via a depth-first search, and extracting the [`TraceEvent`]
//! from each [`Edge`].
//!
//! The graph generation and trace extraction steps are separated for conceptual simplicity, and in
//! anticipation of stochastic methods for trace generation.

mod output;

use std::collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
};
use std::fmt::Write;
use std::fs::File;
use std::hash::Hash;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use json_patch::diff;
use kube::api::DynamicObject;
use petgraph::prelude::*;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use rand_distr::{
    Distribution,
    Poisson,
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{
    json,
    Value,
};
use sk_core::k8s::GVK;
use sk_store::{
    ExportedTrace,
    TraceEvent,
    TraceStorable,
    TraceStore,
    TracerConfig,
    TrackedObjectConfig,
};

use crate::output::{
    display_walks_and_traces,
    export_graphviz,
    gen_trace_event,
};

/// The starting timestamp for the first [`TraceEvent`] in a generated [`Trace`].
const BASE_TS: i64 = 1_728_334_068;

const CREATE_DELETE_ACTION_PROBABILITY: f64 = 0.1;

fn generate_diff(prev: &Node, next: &Node) -> Value {
    let prev_json = serde_json::to_value(prev).expect("Failed to serialize prev node");
    let next_json = serde_json::to_value(next).expect("Failed to serialize next node");

    // diff returns a Patch, so we need to convert it back to Value
    serde_json::to_value(diff(&prev_json, &next_json)).expect("Failed to convert patch to value")
}

fn parse_trace_file(path: &PathBuf) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let exported_trace: ExportedTrace = serde_json::from_reader(reader)?;

    // convert the TraceStore into a sequence of Nodes
    Ok(exported_trace
        .events()
        .iter()
        .map(|trace_event| {
            Node::from_objects(
                trace_event
                    .applied_objs
                    .iter()
                    .map(|obj| (obj.metadata.name.clone().unwrap(), obj.clone().into())) // TODO verify metadata.name is unique
                    .collect(),
            )
        })
        .collect())
}

fn import_traces(trace_files: &[PathBuf]) -> Result<Vec<Node>> {
    let mut traces = Vec::new();
    for trace_file in trace_files {
        let trace_nodes = parse_trace_file(trace_file)?;
        traces.extend(trace_nodes);

        todo!("henry + saya add edges between added nodes");
    }
    Ok(traces)
}

// the clap crate allows us to define a CLI interface using a struct and some #[attributes]
/// `sk-gen` is a CLI tool for generating synthetic trace data which is ingestible by SimKube.
///
/// If no trace/walk output is requested, the tool will only generate the graph, which runs
/// considerably faster for substantially high input values.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Trace length (>= 3, including start state).
    ///
    /// A graph is constructed so as to contain all `trace_length`-walks from the starting state,
    /// then we enumerate all such walks.
    #[arg(short = 'l', long, value_parser = clap::value_parser!(u64).range(3..))]
    trace_length: u64,

    /// Number of candidate objects
    ///
    /// These are generated as `obj-1`, `obj-2`, ... `obj-N`.
    #[arg(short, long)]
    object_count: usize,

    /// Number of sample walks to generate (if not specified, generates all possible walks)
    #[arg(short, long)]
    num_samples: Option<usize>,

    /// If provided, output file in which graphviz representation of the graph will be written.
    #[arg(short = 'g', long)]
    graph_output_file: Option<PathBuf>,

    /// If provided, output directory to which traces will be written.
    ///
    /// Traces are stored as msgpack files of the form `trace-{n}.mp`. Each can be read individually
    /// by SimKube.
    #[arg(short = 'o', long)]
    traces_output_dir: Option<PathBuf>,

    /// Display walks to stdout. Walks are displayed as a list of nodes and intermediate actions.
    #[arg(short = 'w', long)]
    display_walks: bool,

    #[arg(short = 'f', long)]
    trace_files: Option<Vec<PathBuf>>,
}

/// Actions which can be applied to an object.
/// This is a placeholder interface for action types that could be implemented.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ObjectAction {
    /// Create a new object in the cluster
    CreateObject,
    /// Delete an existing object from the cluster
    DeleteObject,
    /// Placeholder for additional object action types that could be implemented
    CustomAction(String),
}

/// An action to be applied to a [`Node`] on one of its active objects.
///
/// This is a placeholder interface for cluster actions.
/// Implementations would define what actions are valid in which contexts.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct ClusterAction {
    /// The unique name by which the target object is identified
    target_name: String,
    /// The action to perform on the target object.
    action_type: ObjectAction,
}



/// A cluster state at an (unspecified) point in time. This tracks which of the candidate
/// objects are active and their state.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Serialize)]

struct Node {
    /// The names of the active objects in the cluster and their configurations.
    ///
    /// Assuming we are in the same namespace, the use of a map enforces that only one object
    /// of each name may exist at once.
    ///
    /// To derive [`Hash`] for [`Node`], we use [`BTreeMap`] which implements `Hash` as our keys
    /// (the object names) implement [`Ord`],
    objects: BTreeMap<String, DynamicObjectWrapper>,
    timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DynamicObjectWrapper {
    dynamic_object: DynamicObject,
}

struct DynamicAction {
    // P(this action or deletion | object is being acted upon)
    probability: f64,
    dynamic_action_type: DynamicActionType,
}

enum DynamicActionType {
    /// Create or modify
    Create {
        applied: DynamicObject,
    },
    Delete, 
}

trait K8sObject: std::fmt::Debug {
    // TODO intialization parameterization
    fn new_boxed() -> Box<Self> where Self: Sized;
    fn current_state(&self) -> DynamicObjectWrapper;
    fn enumerate_actions(&self) -> Vec<DynamicAction>;
}



impl From<DynamicObject> for DynamicObjectWrapper {
    fn from(value: DynamicObject) -> Self {
        Self { dynamic_object: value }
    }
}

impl PartialEq for DynamicObjectWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.dynamic_object == other.dynamic_object
    }
}

impl Eq for DynamicObjectWrapper {}

impl std::hash::Hash for DynamicObjectWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let json = serde_json::to_string(&self.dynamic_object).unwrap();
        json.hash(state);
    }
}

impl Node {
    fn from_objects(objects: BTreeMap<String, DynamicObjectWrapper>) -> Self {
        let mut ret = Self::new();
        ret.objects = objects;
        ret
    }

    /// Creates a new state with no active objects.
    ///
    /// This can be revised in future to, for instance, start at the end of an existing trace.
    fn new() -> Self {
        Self { objects: BTreeMap::new(), timestamp: 0 }
    }

    /// Attempts to create an object in this state.
    ///
    /// Returns [`None`] if the object already exists.
    fn create_object(
        &self,
        name: &str,
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
    ) -> Option<Self> {
        let object = candidate_objects.get(name)?;

        let mut next_state = self.clone();
        next_state.objects.insert(name.to_string(), object.clone());
        Some(next_state)
    }

    /// Attempts to delete an object from this state.
    ///
    /// Returns [`None`] if the object does not exist.
    fn delete_object(&self, name: &str) -> Option<Self> {
        if self.objects.contains_key(name) {
            let mut next_state = self.clone();
            next_state.objects.remove(name);
            Some(next_state)
        } else {
            None
        }
    }

    /// Attempts to perform a [`ClusterAction`] on this [`Node`] to obtain a next [`Node`].
    ///
    /// This is a placeholder implementation.
    /// A real implementation would apply the action to create a new cluster state.
    fn perform_action(
        &self,
        ClusterAction { target_name: object_name, action_type }: ClusterAction,
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
    ) -> Option<Self> {
        let new_node = match action_type {
            ObjectAction::CreateObject => self.create_object(&object_name, candidate_objects),
            ObjectAction::DeleteObject => self.delete_object(&object_name),
            ObjectAction::CustomAction(_) => {
                // Placeholder for custom actions
                None
            }
        };
        if let Some(mut new_node) = new_node {
            let poisson = Poisson::new(2.0).unwrap();
            let wait_time = poisson.sample(&mut thread_rng());
            new_node.timestamp = self.timestamp + wait_time as u64;
            Some(new_node)
        } else {
            None
        }
    }

    /// Enumerates at least all possible `ClusterAction` instances.
    ///
    /// Not all returned cluster actions are necessarily valid. [`Node::valid_action_states`] will
    /// filter out all cluster actions which produce invalid `None` next states.
    fn enumerate_actions(&self, candidate_objects: &BTreeMap<String, DynamicObjectWrapper>) -> Vec<ClusterAction> {
        let mut actions = Vec::new();

        // across all candidate objects, we can try to create/delete according to whether the object
        // is already present
        for name in candidate_objects.keys() {
            if self.objects.contains_key(name) {
                // already created, so we can delete
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: ObjectAction::DeleteObject,
                });
            } else {
                // not already created, so we can create
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: ObjectAction::CreateObject,
                });
            }
        }

        // We no longer handle replica operations
        
        actions
    }

    /// Attempts all possible actions, returning a list of `(action, next_state)` pairs
    /// corresponding to each action which produces a valid next state.
    fn valid_action_states(
        &self,
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
    ) -> Vec<(ClusterAction, Self)> {
        self.enumerate_actions(candidate_objects)
            .into_iter()
            .filter_map(|action| {
                self.perform_action(action.clone(), candidate_objects)
                    .map(|next_state| (action, next_state))
            })
            .collect()
    }
}

/// A directed transition between two [`Node`]s in the cluster.
///
/// It contains a [`ClusterAction`], the internal representation of the action, and stores the
/// corresponding [`TraceEvent`].
#[derive(Debug, Clone)]
struct Edge {
    /// The internal (condensed) representation of the action.
    action: ClusterAction,
    /// The corresponding `TraceEvent` in a trace consumable by simkube.
    ///
    /// Storing this on the `Edge` lets us avoid the need to recompute the event on every walk which
    /// traverses this edge.
    trace_event: TraceEvent,
    // JSON patch diff between previous and next node
    diff: Value,
}

/// A walk is a sequence of (incoming edge, node) pairs.
/// The first node has no incoming edge.
type Walk = Vec<(Option<Edge>, Node)>;

/// A graph of cluster states in which [`Walk`]s map 1:1 with [`Trace`]s.
struct ClusterGraph {
    /// A map of unique object names to object configurations.
    ///
    /// Each object in this map represents the initial state of each object when
    /// initialized by a `CreateObject`.
    candidate_objects: BTreeMap<String, DynamicObjectWrapper>,
    /// The graph itself.
    ///
    /// Each [`Node`] is a cluster state and each [`Edge`] corresponds to a call to the Kubernetes
    /// API.
    graph: DiGraph<Node, Edge>,
}

impl ClusterGraph {
    /// Construct a new graph starting from a given initial state.
    /// 
    /// Uses a placeholder action generation system to expand the graph.
    /// The graph is constructed using BFS to a depth of `trace_length - 1`.
    fn new(
        candidate_objects: BTreeMap<String, DynamicObjectWrapper>,
        starting_state: Vec<Node>,
        trace_length: u64,
    ) -> Self {
        let mut cluster_graph = Self { candidate_objects, graph: DiGraph::new() };

        // we want to track nodes we've seen before to prevent duplicates...
        // petgraph may have internal capabilities for this, but I haven't had the time to look
        // if this stays a part of our code, we may want to wrap the graph w/ tracking data in a new struct
        // -HM
        let mut node_to_index: HashMap<Node, NodeIndex> = HashMap::new();
        for node in &starting_state {
            let node_idx = cluster_graph.graph.add_node(node.clone());
            node_to_index.insert(node.clone(), node_idx);
        }

        // To find the graph containing all valid traces of trace_length with a given start state, we
        // perform bfs to a depth of trace_length. Queue item: `(depth, node)`
        let mut bfs_queue: VecDeque<(u64, Node)> = VecDeque::new();
        for node in starting_state {
            bfs_queue.push_back((1, node));
        }
        let mut visited = HashSet::new();

        while let Some((depth, node)) = bfs_queue.pop_front() {
            let node_idx = *node_to_index.get(&node).expect("node not found in node_to_index");

            if depth >= trace_length {
                continue;
            }

            let not_previously_seen = visited.insert(node.clone());
            if !not_previously_seen {
                continue;
            }

            node.valid_action_states(&cluster_graph.candidate_objects)
                .into_iter()
                .for_each(|(action, next_state)| {
                    let next_idx = *node_to_index.entry(next_state.clone()).or_insert_with(|| {
                        let node = cluster_graph.graph.add_node(next_state.clone());
                        bfs_queue.push_back((depth + 1, next_state.clone()));
                        node
                    });

                    // We precompute the trace_event once here for our edge rather than recomputing it every
                    // time the edge is traversed in a walk.
                    let trace_event = gen_trace_event(BASE_TS + depth as i64, &node, &next_state);

                    let diff = generate_diff(&node, &next_state);

                    // Because we are not revisiting outgoing nodes, we can be sure that the edge does not already exist
                    // so long as the same (node, node) edge is not achievable by distinct actions
                    cluster_graph
                        .graph
                        .update_edge(node_idx, next_idx, Edge { action, trace_event, diff });
                });
        }

        cluster_graph
    }

    /// Generate all walks of `trace_length` starting from the first node in the graph.
    ///
    /// Returns a list of [`Walk`]s, where each is a list of `(incoming edge, node)` pairs.
    /// The first node of each walk, and thus the first pair, has no incoming edge, but all
    /// remaining pairs contain `Some` edge.
    fn generate_walks(&self, trace_length: u64) -> Vec<Walk> {
        let start_nodes: Vec<NodeIndex> = self.graph.node_indices().collect();
        let mut all_walks = Vec::new();

        // We use a depth-first search because eventually we may want to use stochastic methods which do not
        // fully enumerate the neighborhood of each visited node.
        for walk_start_node in start_nodes {
            let walks = self.dfs_walks(walk_start_node, trace_length);
            all_walks.extend(walks.into_iter().map(|walk_indices| {
                let mut walk = Vec::new();

                let start_node = self.graph.node_weight(walk_indices[0]).unwrap().clone();
                walk.push((None, start_node));

                for window in walk_indices.windows(2) {
                    let (prev, next) = (window[0], window[1]);

                    let edge_idx = self.graph.find_edge(prev, next).unwrap();
                    let node = self.graph.node_weight(next).unwrap().clone();
                    let edge = self.graph.edge_weight(edge_idx).cloned().unwrap();
                    walk.push((Some(edge), node));
                }

                walk
            }));
        }

        all_walks
    }

    /// Perform a depth-first search over all walks of length `walk_length` starting from
    /// `current_node`.
    fn dfs_walks(&self, current_node: NodeIndex, walk_length: u64) -> Vec<Vec<NodeIndex>> {
        let mut walks = Vec::new();

        let start_walk = vec![current_node];
        self.dfs_walks_helper(current_node, start_walk, walk_length, &mut walks);

        walks
    }

    /// Recursive helper for [`Self::dfs_walks`].
    fn dfs_walks_helper(
        &self,
        current_node: NodeIndex,
        current_walk: Vec<NodeIndex>,
        walk_length: u64,
        walks: &mut Vec<Vec<NodeIndex>>,
    ) {
        if current_walk.len() as u64 == walk_length {
            walks.push(current_walk);
            return;
        }

        for neighbor in self.graph.neighbors(current_node) {
            let mut new_walk = current_walk.clone();
            new_walk.push(neighbor);
            self.dfs_walks_helper(neighbor, new_walk, walk_length, walks);
        }
    }

    /// Output a graphviz representation of the graph.
    fn to_graphviz(&self) -> String {
        let mut dot = String::new();
        writeln!(&mut dot, "digraph ClusterGraph {{").unwrap();

        // certain visualization software seem not to like this annotation, so it is presently omitted.
        // writeln!(&mut dot, "  node [shape=box];").unwrap();

        for node_index in self.graph.node_indices() {
            let node = &self.graph[node_index];
            let label = node
                .objects
                .iter()
                // .map(|(name, dep)| format!("{}: {}", name, dep.replica_count))
                .map(|(name, dep)| format!("{name}"))
                .collect::<Vec<_>>()
                .join("\\n");
            writeln!(&mut dot, "  {} [label=\"{}\"];", node_index.index(), label).unwrap();
        }

        for edge in self.graph.edge_references() {
            let action = &edge.weight().action;
            writeln!(
                &mut dot,
                "  {} -> {} [label=\"{} {}\"];",
                edge.source().index(),
                edge.target().index(),
                format!("{:?}", action),
                action.target_name.replace('"', "\\\"") // Escape any quotes in the name
            )
            .unwrap();
        }

        writeln!(&mut dot, "}}").unwrap();
        dot
    }

    /// Generate n walks of length `trace_length` using weighted sampling.
    fn walks_with_sampling(&self, start_node: NodeIndex, walk_length: u64, num_samples: usize) -> Vec<Vec<NodeIndex>> {
        let mut rng = thread_rng();
        let mut samples = Vec::new();

        for _ in 0..num_samples {
            let mut current_walk = vec![start_node];
            let mut current_node = start_node;

            for _ in 1..walk_length {
                let neighbors: Vec<_> = self.graph.neighbors(current_node).collect();
                if neighbors.is_empty() {
                    break;
                }

                let weights: Vec<f64> = neighbors
                    .iter()
                    .map(|&n| {
                        let edge = self.graph.edge_weight(self.graph.find_edge(current_node, n).unwrap()).unwrap();
                        match edge.action.action_type {
                            ObjectAction::CreateObject | ObjectAction::DeleteObject => {
                                CREATE_DELETE_ACTION_PROBABILITY
                            },
                            ObjectAction::CustomAction(_) => {
                                // Default weight for custom actions
                                CREATE_DELETE_ACTION_PROBABILITY
                            }
                        }
                    })
                    .collect();

                let dist = WeightedIndex::new(&weights).unwrap();
                let next_node = neighbors[dist.sample(&mut rng)];

                current_walk.push(next_node);
                current_node = next_node;
            }

            samples.push(current_walk);
        }

        samples
    }

    /// Generate n walks of length `trace_length` using weighted sampling.
    fn generate_n_walks_with_sampling(&self, trace_length: u64, num_samples: usize) -> Vec<Walk> {
        let walk_start_node = self.graph.node_indices().next().unwrap();
        let sampled_walks = self.walks_with_sampling(walk_start_node, trace_length, num_samples);

        sampled_walks
            .into_iter()
            .map(|walk_indices| {
                let mut walk = Vec::new();

                let start_node = self.graph.node_weight(walk_indices[0]).unwrap().clone();
                walk.push((None, start_node));

                for window in walk_indices.windows(2) {
                    let (prev, next) = (window[0], window[1]);

                    let edge_idx = self.graph.find_edge(prev, next).unwrap();
                    let node = self.graph.node_weight(next).unwrap().clone();
                    let edge = self.graph.edge_weight(edge_idx).cloned().unwrap();
                    walk.push((Some(edge), node));
                }

                walk
            })
            .collect()
    }
}

/// Generates simple placeholder objects with names `obj-1`, `obj-2`, ..., `obj-n`.
/// 
/// This is a placeholder implementation that creates basic Kubernetes objects.
/// A real implementation would create objects with meaningful structure and properties.
fn generate_candidate_objects(num_objects: usize) -> BTreeMap<String, DynamicObjectWrapper> {
    (1..=num_objects)
        .map(|i| format!("obj-{i}"))
        .map(|name| {
            let gvk = kube::core::gvk::GroupVersionKind::gvk("group", "v1", "Kind");
            let api_resource = kube::core::discovery::ApiResource::from_gvk(&gvk);
            let obj = DynamicObject::new(&name, &api_resource);
            (name, obj.into())
        })
        .collect()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Generate simple placeholder objects
    let candidate_objects = generate_candidate_objects(cli.object_count);

    let mut starting_state = if let Some(trace_files) = &cli.trace_files {
        import_traces(trace_files)?
    } else {
        // Create a minimal starting state with one object
        let target_name = candidate_objects
            .keys()
            .next()
            .expect("candidate_objects should not be empty")
            .clone();

        let a = Node::new();
        let b = a.create_object(&target_name, &candidate_objects).unwrap();
        
        vec![a, b]
    };

    // Construct the graph using the placeholder action system
    // This builds a graph of possible cluster states connected by actions
    let graph = ClusterGraph::new(candidate_objects, starting_state, cli.trace_length);

    // if the user provided a path for us to save the graphviz representation, do so
    if let Some(graph_output_file) = &cli.graph_output_file {
        export_graphviz(&graph, graph_output_file)?;
    }

    // If we don't need to output walks or traces, we don't need to generate them.
    if cli.graph_output_file.is_some() || cli.traces_output_dir.is_some() || cli.display_walks {
        let walks = if let Some(num_samples) = cli.num_samples {
            graph.generate_n_walks_with_sampling(cli.trace_length, num_samples)
        } else {
            graph.generate_walks(cli.trace_length)
        };

        let traces: Vec<TraceStore> = walks.iter().map(tracestore_from_walk).collect();

        display_walks_and_traces(&walks, &traces, &cli)?;
    }

    Ok(())
}

fn tracestore_from_walk(walk: &Walk) -> TraceStore {
    let config = TracerConfig {
        tracked_objects: HashMap::from([(
            GVK::new("group", "v1", "Kind"),
            TrackedObjectConfig {
                track_lifecycle: false,
                pod_spec_template_path: None,
            },
        )]),
    };

    let mut trace_store = TraceStore::new(config);

    let events = walk
        .iter()
        .filter_map(|(edge, _node)| edge.as_ref().map(|e| e.trace_event.clone()))
        .collect::<Vec<_>>();

    for (ts, trace_event) in events.into_iter().enumerate() {
        for obj in trace_event.applied_objs {
            trace_store.create_or_update_obj(&obj, ts as i64, None); // TODO check on maybe_old_hash
        }

        for obj in trace_event.deleted_objs {
            trace_store.delete_obj(&obj, ts as i64);
        }
    }

    trace_store
}
