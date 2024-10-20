//! `sk-gen` is a CLI tool for generating synthetic trace data for SimKube.
//!
//! # Overview:
//! ## Some core types
//! A [`Node`] represents a cluster state, containing a map from unique names to active
//! [`Deployment`] states. `Node` implements [`Eq`] and [`Hash`], which we use to ensure that
//! equivalent `Node`s are not duplicated in the graph.
//!
//! A [`Deployment`] is a simplified representation of a Kubernetes deployment spec, containing only
//! the fields we are considering.
//!
//! An [`DeploymentAction`] (e.g. `CreateDeployment`, `DeleteDeployment`, `IncrementReplicas`,
//! `DecrementReplicas`) can be performed on individual deployment instances.
//!
//! An [`ClusterAction`] contains a name of a candidate deployment alongside an [`DeploymentAction`] such
//! that it can be applied to a `Node` without ambiguity as to which deployment it applies. Not
//! all `DeploymentAction`s are valid for every `Deployment`, and neither are all `ClusterAction` instances valid
//! for every `Node`. For instance, we cannot delete a `Deployment` that does not exist, nor can we
//! increment/decrement the replicas of a `Deployment` that is not active.
//! 
//! A [`TraceEvent`] represents the Kubernetes API call which corresponds to an `ClusterAction`. 
//! 
//! An [`Edge`] stores both an `ClusterAction` and the corresponding `TraceEvent`.
//! 
//! A [`Trace`] is a sequence of [`TraceEvent`]s along with some additional metadata. 
//! A `Trace` is read by SimKube to drive a simulation.
//!
//! 
//! ## The graph
//!
//! The Kubernetes cluster state graph is represented as a [`ClusterGraph`]. Walks of this graph map 1:1 to traces which can be read by SimKube.
//!
//! ### Parameters
//! - [`trace_length`](CLI::trace_length): we construct the graph so as to contain all walks of
//!   length `trace_length` starting from the initial `Node`.
//! - `starting_state`: The initial [`Node`] from which to start the graph construction. We presently use a `Node` with no active [`Deployment`]s.
//! - `candidate_deployments`: A map from unique deployment names to corresponding initial
//!   [`Deployment`] configurations which are added whenever a `CreateDeployment` action is
//!   performed. We generate candidate deployments as `dep-1`, `dep-2`, etc. according to the
//!   [`deployment_count`](CLI::deployment_count) argument.
//!
//! ### Construction
//! - Starting from an initial [`Node`] with no active deployments, perform a breadth-first search.
//! - For each node visited:
//!   - Construct every [`ClusterAction`] applicable to the current `Node`, filtering for only those which produce a valid next `Node`.
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

use std::collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
};
use std::fmt::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use kube::api::DynamicObject;
use petgraph::prelude::*;
use serde_json::json;

use sk_store::TraceEvent;

use crate::output::{
    display_walks_and_traces,
    gen_trace_event,
    export_graphviz,
    Trace,
};

/// The maximum number of replicas a deployment can have.
const MAX_REPLICAS: u32 = u32::MAX;
/// The minimum number of replicas a deployment can have.
const MIN_REPLICAS: u32 = 0;
/// The starting timestamp for the first `TraceEvent` in a generated `Trace`.
const BASE_TS: i64 = 1_728_334_068;

// the clap crate allows us to define a CLI interface using a struct and some #[attributes]
/// `sk-gen` is a CLI tool for generating synthetic trace data which is ingestible by SimKube.
///
/// If no trace/walk output is requested, the tool will only generate the graph, which runs
/// considerably faster for substantially high input values.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CLI {
    /// Trace length (>= 3, including start state). 
    /// 
    /// A graph is constructed so as to contain all `trace_length`-walks from the starting state, then we enumerate all such walks.
    #[arg(short = 'l', long, value_parser = clap::value_parser!(u64).range(3..))]
    trace_length: usize,

    /// Number of candidate deployments 
    /// 
    /// These are generated as `dep-1`, `dep-2`, ... `dep-N`.
    #[arg(short, long)]
    deployment_count: usize,

    /// If provided, output file in which graphviz representation of the graph will be written.
    #[arg(short = 'g', long)]
    graph_output_file: Option<PathBuf>,

    /// If provided, output directory to which traces will be written.
    /// 
    /// Traces are stored as msgpack files of the form `trace-{n}.mp`. Each can be read individually by SimKube.
    #[arg(short = 'o', long)]
    traces_output_dir: Option<PathBuf>,

    /// Display walks to stdout. Walks are displayed as a list of nodes and intermediate actions.
    #[arg(short = 'w', long)]
    display_walks: bool,

    /// Display traces to stdout as JSON.
    #[arg(short = 't', long)]
    display_traces: bool,
}

/// Actions which can be applied to a [`Deployment`].
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
enum DeploymentAction {
    IncrementReplicas,
    DecrementReplicas,
    CreateDeployment,
    DeleteDeployment,
}

/// An action to be applied to a [`Node`] on a specific candidate [`Deployment`].
///
/// Not all cluster actions are necessarily valid, even if they have a valid name. For instance, we cannot
/// delete a deployment that does not actively exist in the cluster.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct ClusterAction {
    /// The unique name by which the target [`Deployment`] is identified in the `candidate_deployments`
    /// map of [`ClusterGraph`].
    target_name: String,
    /// The [`Deployment`]-level action to perform on the target `Deployment`.
    action_type: DeploymentAction,
}

/// The aspects of a Kubernetes deployment spec which we are considering in our generation.
///
/// We don't want to be lugging YAML around everywhere, especially when the graph gets very large.
/// Defining our own representation also allows us to define exactly which fields we are
/// considering, and how they change with respect to each [`DeploymentAction`].
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct Deployment {
    /// The name of the deployment, unique within the cluster (at least until namespace support is
    /// added).
    name: String,
    /// The number of replicas of the deployment.
    replica_count: u32,
}

impl Deployment {
    /// Creates a new deployment with a given name and replica count.
    fn new(name: String, replica_count: u32) -> Self {
        Self { name, replica_count }
    }

    /// Attempts to increment the replica count of this deployment.
    ///
    /// Returns None if the increment would exceed the maximum number of replicas.
    #[allow(clippy::absurd_extreme_comparisons)]
    fn increment(&self) -> Option<Self> {
        self.replica_count
            .checked_add(1)
            .filter(|rc| *rc <= MAX_REPLICAS)
            .map(|replica_count| Self { replica_count, ..self.clone() })
    }

    /// Attempts to decrement the replica count of this deployment.
    ///
    /// Returns None if the decrement would bring the replica count below the minimum number of
    /// replicas.
    #[allow(clippy::absurd_extreme_comparisons)]
    fn decrement(&self) -> Option<Self> {
        self.replica_count
            .checked_sub(1)
            .filter(|rc| *rc >= MIN_REPLICAS)
            .map(|replica_count| Self { replica_count, ..self.clone() })
    }

    /// Converts this deployment to a [`DynamicObject`].
    ///
    /// A [`DynamicObject`] represents a Kubernetes deployment spec, what we've been lovingly calling "YAML".
    fn to_dynamic_object(&self) -> DynamicObject {
        DynamicObject {
            metadata: kube::api::ObjectMeta {
                namespace: Some("default".to_string()),
                name: Some("min-dep".to_string()),
                ..Default::default()
            },
            types: Some(kube::api::TypeMeta {
                kind: "Deployment".to_string(),
                api_version: "apps/v1".to_string(),
            }),
            data: json!({
                "apiVersion": "apps/v1",
                "kind": "Deployment",
                "spec": {
                    "replicas": self.replica_count,
                    "selector": {
                        "matchLabels": {
                            "app": "minimal-app"
                        }
                    },
                    "template": {
                        "metadata": {
                            "labels": {
                                "app": "minimal-app"
                            }
                        },
                        "spec": {
                            "containers": [{
                                "name": "minimal-container",
                                "image": "nginx:latest"
                            }]
                        }
                    }
                }
            }),
        }
    }
}

/// A cluster state.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct Node {
    /// The names of the active deployments in the cluster and their configurations.
    ///
    /// Assuming we are in the same namespace, the use of a map enforces that only one deployment
    /// of each name may exist at once.
    /// 
    /// To derive [`Hash`] for [`Node`], we use [`BTreeMap`] which implements `Hash` as our keys (the deployment names) implement [`Ord`],
    deployments: BTreeMap<String, Deployment>,
}

impl Node {
    /// Creates a new state with no active [`Deployment`]s.
    ///
    /// This can be revised in future to, for instance, start at the end of an existing trace.
    fn new() -> Self {
        Self { deployments: BTreeMap::new() }
    }

    /// Attempts to create a [`Deployment`] in this state.
    ///
    /// Returns [`None`] if the deployment already exists.
    fn create_deployment(&self, name: &str, candidate_deployments: &BTreeMap<String, Deployment>) -> Option<Self> {
        if self.deployments.contains_key(name) {
            return None;
        }

        candidate_deployments.get(name).map(|deployment| {
            let mut next_state = self.clone();
            next_state.deployments.insert(name.to_string(), deployment.clone());
            next_state
        })
    }

    /// Attempts to delete a [`Deployment`] from this state.
    ///
    /// Returns [`None`] if the deployment does not exist.
    fn delete_deployment(&self, name: &str) -> Option<Self> {
        self.deployments.get(name).map(|_| {
            let mut next_state = self.clone();
            next_state.deployments.remove(name);
            next_state
        })
    }

    /// Attempts to increment the replica count of an active [`Deployment`] in this state.
    ///
    /// Returns [`None`] if the deployment does not exist.
    fn increment_replica_count(&self, name: String) -> Option<Self> {
        self.deployments.get(&name).and_then(Deployment::increment).map(|next| {
            let mut next_state = self.clone();
            next_state.deployments.insert(name, next);
            next_state
        })
    }

    /// Attempts to decrement the replica count of an active [`Deployment`] in this state.
    ///
    /// Returns [`None`] if the deployment does not exist.
    fn decrement_replica_count(&self, name: String) -> Option<Self> {
        self.deployments.get(&name).and_then(Deployment::decrement).map(|next| {
            let mut next_state = self.clone();
            next_state.deployments.insert(name, next);
            next_state
        })
    }

    /// Attempts to perform an [`ClusterAction`] on this [`Node`] to obtain a next [`Node`].
    ///
    /// Returns [`None`] if the action is invalid.
    fn perform_action(
        &self,
        ClusterAction { target_name: name, action_type }: ClusterAction,
        candidate_deployments: &BTreeMap<String, Deployment>,
    ) -> Option<Self> {
        match action_type {
            DeploymentAction::IncrementReplicas => self.increment_replica_count(name),
            DeploymentAction::DecrementReplicas => self.decrement_replica_count(name),
            DeploymentAction::CreateDeployment => self.create_deployment(&name, candidate_deployments),
            DeploymentAction::DeleteDeployment => self.delete_deployment(&name),
        }
    }

    /// Enumerates at least all possible `ClusterAction` instances.
    /// 
    /// Not all returned cluster actions are necessarily valid. [`Node::valid_action_states`] will `filter_map` out all cluster actions which produce
    /// invalid `None` next states.
    fn enumerate_actions(&self, candidate_deployments: &BTreeMap<String, Deployment>) -> Vec<ClusterAction> {
        let mut actions = Vec::new();

        // across all candidate deployments, we can try to create/delete according to whether the deployment
        // is already present
        for name in candidate_deployments.keys() {
            if self.deployments.contains_key(name) {
                // already created, so we can delete
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: DeploymentAction::DeleteDeployment,
                });
            } else {
                // not already created, so we can create
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: DeploymentAction::CreateDeployment,
                });
            }
        }

        // across all active deployments, we can try to increment/decrement, saving bounds checks for later
        for name in self.deployments.keys() {
            actions.push(ClusterAction {
                target_name: name.clone(),
                action_type: DeploymentAction::IncrementReplicas,
            });
            actions.push(ClusterAction {
                target_name: name.clone(),
                action_type: DeploymentAction::DecrementReplicas,
            });
        }

        actions
    }

    /// Gets at least all possible actions (via [`Node::enumerate_actions`]), attempts them all, and
    /// `filter_map`s out all invalid (None) next states. Returns a list of `(action, next_state)`
    /// pairs corresponding to each valid action.
    fn valid_action_states(&self, candidate_deployments: &BTreeMap<String, Deployment>) -> Vec<(ClusterAction, Self)> {
        self.enumerate_actions(candidate_deployments)
            .into_iter()
            .filter_map(|action| {
                self.perform_action(action.clone(), candidate_deployments)
                    .map(|next_state| (action, next_state))
            })
            .collect()
    }
}

/// A directed transition between two [`Node`]s in the cluster.
///
/// It contains an [`ClusterAction`], the internal representation of the action, and stores the
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
}

/// A walk is a sequence of (incoming edge, node) pairs.
/// The first node has no incoming edge.
type Walk = Vec<(Option<Edge>, Node)>;

/// A graph of cluster states in which [`Walk`]s map 1:1 with [`Trace`]s.
struct ClusterGraph {
    /// A map of unique deployment names to [`Deployment`] configurations.
    ///
    /// Each [`Deployment`] in this map represents the initial state of each deployment when
    /// initialized by an [`ClusterAction::CreateDeployment`].
    candidate_deployments: BTreeMap<String, Deployment>,
    /// The graph itself. 
    /// 
    /// Each [`Node`] is a cluster state and each [`Edge`] corresponds to a call to the Kubernetes API.
    graph: DiGraph<Node, Edge>,
}

impl ClusterGraph {
    /// Construct a new graph starting from a given (presently hard-coded) starting state.
    /// This is achieved via a search over all state reachable within `trace_length` actions from
    /// the starting state.
    fn new(candidate_deployments: BTreeMap<String, Deployment>, starting_state: Node, trace_length: usize) -> Self {
        let mut cluster_graph = Self { candidate_deployments, graph: DiGraph::new() };

        let starting_node_idx = cluster_graph.graph.add_node(starting_state.clone());

        // we want to track nodes we've seen before to prevent duplicates...
        // petgraph may have internal capabilities for this, but I haven't had the time to look
        // if this stays a part of our code, we may want to wrap the graph w/ tracking data in a new struct
        // -HM
        let mut node_to_index: HashMap<Node, NodeIndex> = HashMap::new();
        node_to_index.insert(starting_state.clone(), starting_node_idx);

        // To find the graph containing all valid traces of trace_length with a given start state, we
        // perform bfs to a depth of trace_length. Queue item: `(depth, deployment)`
        let mut bfs_queue: VecDeque<(usize, Node)> = VecDeque::new();
        bfs_queue.push_back((1, starting_state)); // start at depth 1
        let mut visited = HashSet::new();

        while let Some((depth, node)) = bfs_queue.pop_front() {
            let node_idx = *node_to_index.get(&node).expect("node not found in node_to_index");

            if depth >= trace_length {
                continue;
            }

            let not_previously_seen = visited.insert(node.clone());
            assert!(not_previously_seen);

            node.valid_action_states(&cluster_graph.candidate_deployments)
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

                    // Because we are not revisiting outgoing nodes, we can be sure that the edge does not already exist
                    // so long as the same (node, node) edge is not achievable by distinct actions
                    cluster_graph
                        .graph
                        .update_edge(node_idx, next_idx, Edge { action, trace_event });
                });
        }

        cluster_graph
    }

    /// Generate all walks of `trace_length` starting from the first node in the graph.
    ///
    /// Returns a list of [`Walk`]s, where each is a list of `(incoming edge, node)` pairs.
    /// The first node of each walk, and thus the first pair, has no incoming edge, but all remaining
    /// pairs contain `Some` edge.
    fn generate_walks(&self, trace_length: usize) -> Vec<Walk> {
        let walk_start_node = self.graph.node_indices().next().unwrap();

        // We use a depth-first search because eventually we may want to use stochastic methods which do not
        // fully enumerate the neighborhood of each visited node.

        self.dfs_walks(walk_start_node, trace_length)
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

    /// Perform a depth-first search over all walks of length `walk_length` starting from
    /// `current_node`.
    fn dfs_walks(&self, current_node: NodeIndex, walk_length: usize) -> Vec<Vec<NodeIndex>> {
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
        walk_length: usize,
        walks: &mut Vec<Vec<NodeIndex>>,
    ) {
        if current_walk.len() == walk_length {
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
                .deployments
                .iter()
                .map(|(name, dep)| format!("{}: {}", name, dep.replica_count))
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
                match action.action_type {
                    DeploymentAction::IncrementReplicas => "replicas++",
                    DeploymentAction::DecrementReplicas => "replicas--",
                    DeploymentAction::CreateDeployment => "create",
                    DeploymentAction::DeleteDeployment => "delete",
                },
                action.target_name.replace('"', "\\\"") // Escape any quotes in the name
            )
            .unwrap();
        }

        writeln!(&mut dot, "}}").unwrap();
        dot
    }
}

/// This mod contains graph/trace output utilities and functionality.
mod output {
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
        Node,
        Walk,
        ClusterGraph,
        CLI,
    };


    // The rest of SimKube handles this data as a tuple, but since some of us are newer, we just use a
    // struct.
    /// A sequence of [`TraceEvent`] instances and additional metadata which can be simulated by SimKube.
    pub(crate) struct Trace {
        config: TracerConfig,
        /// At the moment, this is the only field we are using. This is just a queue of `TraceEvent`s.
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
    pub(crate) fn display_walks_and_traces(walks: &[Walk], traces: &[Trace], cli: &CLI) -> Result<()> {
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
            }
            println!();
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

    /// Exports the graphviz representation of the graph to a file, ensuring the parent directory exists.
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
}

/// Generates `num_deployments` candidate deployments with names `dep-1`, `dep-2`, ..., `dep-n`.
fn generate_candidate_deployments(num_deployments: usize) -> BTreeMap<String, Deployment> {
    (1..=num_deployments)
        .map(|i| format!("dep-{i}"))
        .map(|name| (name.clone(), Deployment::new(name, 1)))
        .collect()
}

fn main() -> Result<()> {
    let cli = CLI::parse();

    let candidate_deployments = generate_candidate_deployments(cli.deployment_count);
    let starting_state = Node::new(); // Start with no active deployments

    // Construct the graph by searching all valid sequences of `trace_length`-1 actions from the
    // starting state for a total of `trace_length` nodes.
    let graph = ClusterGraph::new(candidate_deployments, starting_state, cli.trace_length);

    // if the user provided a path for us to save the graphviz representation, do so
    if let Some(graph_output_file) = &cli.graph_output_file {
        export_graphviz(&graph, graph_output_file)?;
    }

    // If we don't need to output walks or traces, we don't need to generate them.
    if cli.graph_output_file.is_some() || cli.traces_output_dir.is_some() || cli.display_walks || cli.display_traces {
        let walks = graph.generate_walks(cli.trace_length);

        let traces: Vec<Trace> = walks
            .iter()
            .map(Trace::from_walk)
            .collect();

        display_walks_and_traces(&walks, &traces, &cli)?;
    }

    Ok(())
}
