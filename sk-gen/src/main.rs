use std::collections::{
    BTreeMap,
    HashMap,
    HashSet,
    VecDeque,
};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use anyhow::{
    bail,
    Result,
};
use clap::{
    Parser,
    ValueEnum,
    Subcommand,
};
use kube::api::DynamicObject;
use petgraph::prelude::*;
use serde_json::json;
use sk_core::k8s::GVK;
use sk_store::{
    PodLifecyclesMap,
    TraceEvent,
    TracerConfig,
    TrackedObjectConfig,
};
use tracing::{
    info,
    warn,
};

const MAX_REPLICAS: u32 = u32::MAX;
const MIN_REPLICAS: u32 = 0;
const BASE_TS: i64 = 1728334068;

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
enum DisplayMode {
    Trace,
    Nodes,
    Actions,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate traces
    Traces(TracesArgs),
    /// Generate graph
    Graph(GraphArgs),
}

#[derive(Parser)]
struct TracesArgs {
    /// trace length (>= 3, including start state)
    #[arg(short, long, value_parser = clap::value_parser!(u64).range(3..))]
    trace_length: u64,

    /// output modes to stdout, leave blank for no stdout
    #[arg(short, long, value_delimiter = ',')]
    display_modes: Option<Vec<DisplayMode>>,

    /// if provided, output directory in which msgpack traces will be written
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// force overwrite existing trace files
    #[arg(short, long, default_value_t = false)]
    force_overwrite: bool,
}

#[derive(Parser)]
struct GraphArgs {
    /// trace length (>= 3, including start state)
    #[arg(short, long, value_parser = clap::value_parser!(u64).range(3..))]
    trace_length: u64,

    /// graphviz dot file output
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let candidate_deployments: BTreeMap<String, Deployment> = BTreeMap::from_iter((1..=5).map(|i| {
        let name = format!("dep-{i}");
        (name.clone(), Deployment::new(name, 1))
    }));

    let starting_state = Node::new();

    match &cli.command {
        Commands::Traces(args) => generate_traces(args, candidate_deployments, starting_state),
        Commands::Graph(args) => generate_graph(args, candidate_deployments, starting_state),
    }
}

fn generate_traces(args: &TracesArgs, candidate_deployments: BTreeMap<String, Deployment>, starting_state: Node) -> Result<()> {
    if let Some(dir) = &args.output_dir {
        validate_overwrite_conditions(dir, args.trace_length, args.force_overwrite)?;
    }

    let graph = ClusterGraph::new(candidate_deployments, starting_state, args.trace_length);
    let walks = graph.generate_walks(args.trace_length);

    if walks.is_empty() {
        warn!("No walks generated");
    }

    graph.output_traces(walks, args)?;
    Ok(())
}

fn generate_graph(args: &GraphArgs, candidate_deployments: BTreeMap<String, Deployment>, starting_state: Node) -> Result<()> {
    let graph = ClusterGraph::new(candidate_deployments, starting_state, args.trace_length);

    let path = args.output.clone();

    // ensure parent exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let contents = graph.to_graphviz();
    std::fs::write(path, contents)?;
    Ok(())
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
enum ActionType {
    IncrementReplicas,
    DecrementReplicas,
    CreateDeployment,
    DeleteDeployment,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct Action {
    name: String,
    action_type: ActionType,
}


#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct Deployment {
    name: String,
    replica_count: u32,
}

impl Deployment {
    fn new(name: String, replica_count: u32) -> Self {
        Self { name, replica_count }
    }

    #[allow(clippy::absurd_extreme_comparisons)]
    fn increment(&self) -> Option<Self> {
        self.replica_count
            .checked_add(1)
            .filter(|rc| *rc <= MAX_REPLICAS)
            .map(|replica_count| Self { replica_count, ..self.clone() })
    }

    #[allow(clippy::absurd_extreme_comparisons)]
    fn decrement(&self) -> Option<Self> {
        self.replica_count
            .checked_sub(1)
            .filter(|rc| *rc >= MIN_REPLICAS)
            .map(|replica_count| Self { replica_count, ..self.clone() })
    }

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


/// `Node` represents a state of the cluster
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct Node {
    // To derive Hash for Node, we BTreeMap which implements Hash as our keys implement Ord
    /// An ordered map (deployment name => deployment configuration).
    /// Assuming we are in the same namespace, this reflects that only one deployment of each name
    /// may exist at once.
    deployments: BTreeMap<String, Deployment>,
}

impl Node {
    /// Create a new state with no active deployments. This can be revised to, for instance, start
    /// at the end of an existing trace.
    fn new() -> Self {
        Self { deployments: BTreeMap::new() }
    }

    /// Attempt to create a deployment in this state.
    /// Returns None if the deployment already exists.
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

    /// Attempt to delete a deployment from this state.
    /// Returns None if the deployment does not exist.
    fn delete_deployment(&self, name: String) -> Option<Self> {
        self.deployments.get(&name).map(|_| {
            let mut next_state = self.clone();
            next_state.deployments.remove(&name);
            next_state
        })
    }

    /// Attempt to increment the replica count of an active deployment in this state.
    /// Returns None if the deployment does not exist.
    fn increment_replica_count(&self, name: String) -> Option<Self> {
        self.deployments.get(&name).and_then(Deployment::increment).map(|next| {
            let mut next_state = self.clone();
            next_state.deployments.insert(name, next);
            next_state
        })
    }

    /// Attempt to decrement the replica count of an active deployment in this state.
    /// Returns None if the deployment does not exist.
    fn decrement_replica_count(&self, name: String) -> Option<Self> {
        self.deployments.get(&name).and_then(Deployment::decrement).map(|next| {
            let mut next_state = self.clone();
            next_state.deployments.insert(name, next);
            next_state
        })
    }

    /// Attempt to perform an action in this state.
    /// Returns None if the action is invalid.
    fn perform_action(
        &self,
        Action { name, action_type }: Action,
        candidate_deployments: &BTreeMap<String, Deployment>,
    ) -> Option<Self> {
        match action_type {
            ActionType::IncrementReplicas => self.increment_replica_count(name),
            ActionType::DecrementReplicas => self.decrement_replica_count(name),
            ActionType::CreateDeployment => self.create_deployment(&name, candidate_deployments),
            ActionType::DeleteDeployment => self.delete_deployment(name),
        }
    }

    /// Enumerate at least all possible actions
    /// Returns list of actions which may or may not be valid, but contains at least all possible
    /// actions `valid_action_states` will filter_map out all actions which produce invalid
    /// (None) next states
    fn enumerate_actions(&self, candidate_deployments: &BTreeMap<String, Deployment>) -> Vec<Action> {
        let mut actions = Vec::new();

        // across all candidate deployments, we can try to create/delete according to whether the deployment
        // is already present
        for name in candidate_deployments.keys() {
            if !self.deployments.contains_key(name) {
                // not already created, so we can create
                actions.push(Action {
                    name: name.clone(),
                    action_type: ActionType::CreateDeployment,
                });
            } else {
                // already created, so we can delete
                actions.push(Action {
                    name: name.clone(),
                    action_type: ActionType::DeleteDeployment,
                });
            }
        }

        // across all active deployments, we can try to increment/decrement, saving bounds checks for later
        for name in self.deployments.keys() {
            actions.push(Action {
                name: name.clone(),
                action_type: ActionType::IncrementReplicas,
            });
            actions.push(Action {
                name: name.clone(),
                action_type: ActionType::DecrementReplicas,
            });
        }

        actions
    }

    /// Get at least all possible actions (via `enumerate_actions`), attempt them all, and
    /// filter_map out all invalid (None) next states Return list of (action, next_state) for
    /// each valid action
    fn valid_action_states(&self, candidate_deployments: &BTreeMap<String, Deployment>) -> Vec<(Action, Self)> {
        self.enumerate_actions(candidate_deployments)
            .into_iter()
            .filter_map(|action| {
                self.perform_action(action.clone(), candidate_deployments)
                    .map(|state| (action, state))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Edge {
    /// The internal (condensed) representation of the action.
    action: Action,
    /// The corresponding trace event which is consumable by simkube.
    trace_event: TraceEvent,
}

struct ClusterGraph {
    candidate_deployments: BTreeMap<String, Deployment>,
    graph: DiGraph<Node, Edge>,
}

impl ClusterGraph {
    /// Construct a new graph starting from a given (presently hard-coded) starting state.
    /// This is achieved via a search over all state reachable within <trace_length> actions from
    /// the starting state.
    fn new(candidate_deployments: BTreeMap<String, Deployment>, starting_state: Node, trace_length: u64) -> Self {
        let mut graph = DiGraph::new();
        let starting_node_idx = graph.add_node(starting_state.clone());

        // we want to track deployment configurations we've seen before to prevent duplicate nodes...
        // petgraph may have internal capabilities for this, but I haven't had the time to look
        // if this stays a part of our code, we may want to wrap the graph w/ tracking data in a new struct
        // -HM
        let mut configuration_to_index: HashMap<Node, NodeIndex> = HashMap::new();
        configuration_to_index.insert(starting_state.clone(), starting_node_idx);

        // To find the graph containing all valid traces of trace_length with a given start state, we
        // perform bfs to a depth of trace_length. Queue item: (depth, deployment)
        let mut bfs_queue: VecDeque<(u64, Node)> = VecDeque::new();
        bfs_queue.push_back((1, starting_state)); // start at depth 1
        let mut visited = HashSet::new();

        while let Some((depth, configuration)) = bfs_queue.pop_front() {
            let node_idx = *configuration_to_index
                .get(&configuration)
                .expect("configuration not in index");

            if depth >= trace_length {
                continue;
            }

            let not_previously_seen = visited.insert(configuration.clone());
            assert!(not_previously_seen);

            configuration
                .valid_action_states(&candidate_deployments)
                .into_iter()
                .for_each(|(action, next)| {
                    tracing::debug!(action = ?action, "Considering action");
                    let next_idx = if let Some(node) = configuration_to_index.get(&next) {
                        // we've already seen this node so no need to revisit, but we still need to add the dge
                        *node
                    } else {
                        let node = graph.add_node(next.clone());
                        configuration_to_index.insert(next.clone(), node);
                        bfs_queue.push_back((depth + 1, next.clone()));
                        // we need to add the node so we can add the edge, but we haven't visited it yet
                        node
                    };

                    // because we are not revisiting outgoing nodes, we can be sure that the edge does not already exist
                    // EXCEPT if the the same (node, node) edge is achievable by different actions
                    graph.update_edge(
                        node_idx,
                        next_idx,
                        Edge {
                            action,
                            trace_event: Self::gen_trace_event(BASE_TS + depth as i64, &configuration, &next),
                        },
                    );
                });
        }

        Self { candidate_deployments, graph }
    }

    /// Generate the simkube-consumable trace event (i.e. applied/deleted objects) to get from
    /// `prev` to `next` state over `ts` seconds.
    fn gen_trace_event(ts: i64, prev: &Node, next: &Node) -> TraceEvent {
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

    /// Generate all walks of length `trace_length` starting from the first node in the graph.
    /// Returns a list of walks, where each walk is a list of (incoming edge, node) pairs.
    /// The first node of each walk, and thus the first pair has no incoming edge, but all remaining
    /// pairs contain Some edge.
    fn generate_walks(&self, trace_length: u64) -> Vec<Vec<(Option<Edge>, Node)>> {
        let walk_start_node = self.graph.node_indices().next().unwrap();
        let walk_length = trace_length as usize;

        // We use a depth-first search because eventually we may want to use stochastic methods which do not
        // fully enumerate the neighborhood of each visited node.

        self.dfs_walks(walk_start_node, walk_length)
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

    /// Recursive helper for `dfs_walks`.
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
                    ActionType::IncrementReplicas => "replicas++",
                    ActionType::DecrementReplicas => "replicas--",
                    ActionType::CreateDeployment => "create",
                    ActionType::DeleteDeployment => "delete",
                },
                action.name.replace("\"", "\\\"") // Escape any quotes in the name
            )
            .unwrap();
        }

        writeln!(&mut dot, "}}").unwrap();
        dot
    }

    /// Generate output in accordance with CLI arguments.
    fn output_traces(&self, walks: Vec<Vec<(Option<Edge>, Node)>>, args: &TracesArgs) -> Result<()> {
        if let Some(dir) = &args.output_dir {
            if !dir.exists() {
                info!("creating output directory at {}", dir.display());
                std::fs::create_dir_all(dir)?;
            }
        }


        for (i, walk) in walks.into_iter().enumerate() {
            let trace_events = walk
                .iter()
                .filter_map(|(edge, _node)| edge.as_ref().map(|e| e.trace_event.clone()))
                .collect();
            let data = generate_synthetic_trace(trace_events);

            if let Some(display_modes) = &args.display_modes {
                let display_trace = display_modes.contains(&DisplayMode::Trace);
                let display_actions = display_modes.contains(&DisplayMode::Actions);
                let display_nodes = display_modes.contains(&DisplayMode::Nodes);

                println!("walk-{}:", i);
                if display_trace {
                    let json_pretty = serde_json::to_string_pretty(&data)?;
                    println!("trace:\n{}", json_pretty);
                }
                if display_actions || display_nodes {
                    for (j, (edge, node)) in walk.iter().enumerate() {
                        println!("  step-{}:", j);
                        if display_actions {
                            if let Some(edge) = edge {
                                println!("    Action: {:?}", edge.action);
                            }
                        }
                        if display_nodes {
                            println!("    Deployment Configurations:");
                            for (name, config) in &node.deployments {
                                println!("      {}: {:?}", name, config);
                            }
                        }
                    }
                }
            }

            if let Some(dir) = &args.output_dir {
                let data = rmp_serde::to_vec(&data)?;
                let path = dir.join(format!("trace-{}.mp", i));
                std::fs::write(path, data)?;
            }
        }

        // graph generation
        // TODO: break out graph generation to its own subcommand
        if let Some(dir) = &args.output_dir {
            let path = dir.join("graph.dot");
            let contents = self.to_graphviz();
            std::fs::write(path, contents)?;
        }
        Ok(())
    }
}


/// Validates that we aren't overwriting files unless --force-overwrite has been specified.
fn validate_overwrite_conditions(dir: &Path, trace_length: u64, force_overwrite: bool) -> Result<()> {
    if !dir.exists() {
        // Can't overwrite files in a non-existent directory.
        return Ok(());
    }

    let existing_files: Vec<_> = dir.read_dir()?.collect();

    if !existing_files.is_empty() {
        warn!("Output directory {} exists and is not empty", dir.display());
    }

    let re = regex::Regex::new(r"trace-(\d+)\.mp")?;
    for file in existing_files {
        let path = file.as_ref().unwrap().path();
        if let Some(captures) = re.captures(path.to_str().unwrap()) {
            let trace_num = captures[1].parse::<usize>()?;
            if trace_num <= trace_length as usize {
                if !force_overwrite {
                    bail!(
                        "Output file {} would be overwritten, but force-overwrite is not set",
                        path.to_str().unwrap()
                    );
                } else {
                    info!("Output file {} will be overwritten as force-overwrite is set", path.to_str().unwrap());
                }
            }
        }
    }
    Ok(())
}


/// The final output step to generate a full trace from a list of trace events.
pub fn generate_synthetic_trace(
    events: Vec<TraceEvent>,
) -> (TracerConfig, VecDeque<TraceEvent>, HashMap<String, u64>, HashMap<String, PodLifecyclesMap>) {
    let events = VecDeque::from(events);

    let config = TracerConfig {
        // TODO not even sure if this is even close to right
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

    (config, events, index, pod_lifecycles)
}