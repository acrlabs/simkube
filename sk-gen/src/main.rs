#![deny(rustdoc::broken_intra_doc_links)]
//! `sk-gen` is a CLI tool for generating synthetic trace data for SimKube.

mod output;

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
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::DynamicObject;
use petgraph::prelude::*;
use rand::distributions::WeightedIndex;
use rand::prelude::*;
use sk_core::jsonutils::{
    ordered_eq,
    ordered_hash,
};
use sk_core::k8s::GVK;
use sk_store::{
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
    write_debug_info,
};

const BASE_TS: i64 = 1_728_334_068;

const REPLICA_COUNT_CHANGE: i32 = 1;
const REPLICA_COUNT_MIN: i32 = 0;
const REPLICA_COUNT_MAX: i32 = i32::MAX;

const RESOURCE_SCALE_FACTOR: f64 = 2.0;
const RESOURCE_SCALE_MIN: i64 = 1;

const SCALE_ACTION_PROBABILITY: f64 = 0.8;
const CREATE_DELETE_ACTION_PROBABILITY: f64 = 0.1;


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
    #[arg(short = 'l', long, value_parser = clap::value_parser!(u64).range(2..))]
    trace_length: u64,

    /// Path to input trace file (msgpack).
    #[arg(short, long)]
    input_trace: PathBuf,

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
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ObjectAction {
    Create,
    Delete,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ActionType {
    Increase,
    Decrease,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ResourceAction {
    Request { resource: String, action: ActionType },
    Limit { resource: String, action: ActionType },
    Claim,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ContainerAction {
    Resource(ResourceAction),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum DeploymentAction {
    ReplicaCount(ActionType),
    Object(ObjectAction),
    Container { name: String, action: ContainerAction },
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct ClusterAction {
    deployment_name: String,
    deployment_action: DeploymentAction,
}


#[derive(Clone, Debug)]
struct Node {
    deployments: BTreeMap<String, Deployment>,
}

impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ordered_hash(&serde_json::to_value(&self.deployments).unwrap()).hash(state);
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        ordered_eq(
            &serde_json::to_value(&self.deployments).unwrap(),
            &serde_json::to_value(&other.deployments).unwrap(),
        )
    }
}

impl Eq for Node {}

fn dynamic_object_to_deployment(dynamic_object: &DynamicObject) -> Result<Deployment> {
    let json = serde_json::to_value(dynamic_object).expect("All dynamic objects are serializable");
    // TODO: check explicitly that this is a deployment
    let deployment = serde_json::from_value(json)?;
    Ok(deployment)
}

fn deployment_to_dynamic_object(deployment: &Deployment) -> Result<DynamicObject> {
    let json = serde_json::to_value(deployment).expect("All deployments are serializable");
    let dynamic_object = serde_json::from_value(json).expect("DynamicObject should superset Deployment");
    Ok(dynamic_object)
}

fn scale_quantity(quantity_str: &str, scale: f64) -> Option<String> {
    // Parse number and suffix (e.g., "1048576" -> (1048576, ""))
    let mut num_str = String::new();
    let mut suffix = String::new();
    let mut in_suffix = false;

    for c in quantity_str.chars() {
        if c.is_ascii_digit() || c == '.' {
            if !in_suffix {
                num_str.push(c);
            } else {
                return None; // Invalid format
            }
        } else {
            in_suffix = true;
            suffix.push(c);
        }
    }

    // Parse the number
    let num: f64 = num_str.parse().ok()?;
    let scaled = (num * scale) as i64;

    if scaled < RESOURCE_SCALE_MIN {
        return None;
    }

    // Format back with the same suffix
    Some(format!("{}{}", scaled, suffix))
}

impl Node {
    fn new() -> Self {
        Self { deployments: BTreeMap::new() }
    }

    fn from_trace_store(trace_store: &TraceStore) -> (Vec<Self>, BTreeMap<String, Deployment>) {
        let mut candidate_objects = BTreeMap::new();

        let mut node = Node::new();
        let mut nodes = Vec::new();

        for (event, _ts) in trace_store.iter() {
            // TODO: add ts handling

            for applied_obj in &event.applied_objs {
                let name = applied_obj.metadata.name.as_ref().unwrap();

                let deployment = dynamic_object_to_deployment(&applied_obj)
                    .expect("All objects in imported trace should be deployments");
                node.deployments.insert(name.clone(), deployment.clone());

                if !candidate_objects.contains_key(name) {
                    candidate_objects.insert(name.clone(), deployment.clone());
                }
            }

            for deleted_obj in &event.deleted_objs {
                node.deployments.remove(deleted_obj.metadata.name.as_ref().unwrap());
            }

            nodes.push(node.clone());
        }
        (nodes, candidate_objects)
    }

    fn create_deployment(&self, name: &str, candidate_deployment: &BTreeMap<String, Deployment>) -> Option<Self> {
        let object = candidate_deployment.get(name)?;

        let mut next_state = self.clone();
        next_state.deployments.insert(name.to_string(), object.clone());
        Some(next_state)
    }

    fn delete_deployment(&self, name: &str) -> Option<Self> {
        if self.deployments.contains_key(name) {
            let mut next_state = self.clone();
            next_state.deployments.remove(name);
            Some(next_state)
        } else {
            None
        }
    }

    fn change_replica_count(&self, name: String, change: i32) -> Option<Self> {
        let replicas = self
            .deployments
            .get(&name)?
            .clone()
            .spec
            .as_mut()
            .and_then(|s| s.replicas.as_mut())
            .map(|r| *r)
            .unwrap_or(1);

        let new_replicas = replicas.checked_add(change)?;

        if new_replicas < REPLICA_COUNT_MIN || new_replicas > REPLICA_COUNT_MAX {
            return None;
        }

        let mut deployment = self.deployments.get(&name)?.clone();
        deployment.spec.as_mut().expect("All deployments should have a spec").replicas = Some(new_replicas);

        let mut next_state = self.clone();
        next_state.deployments.insert(name.clone(), deployment);
        Some(next_state)
    }

    fn resource_request(
        &self,
        deployment_name: String,
        container_name: String,
        action: ResourceAction,
    ) -> Option<Self> {
        let mut deployment = self.deployments.get(&deployment_name)?.clone();

        let resources = deployment
            .spec
            .get_or_insert_with(Default::default)
            .template
            .spec
            .get_or_insert_with(Default::default)
            .containers
            .iter_mut()
            .find(|container| container.name == container_name)?
            .resources
            .get_or_insert_with(Default::default);

        match action {
            ResourceAction::Request { resource, action } => {
                let requests = resources.requests.get_or_insert_with(BTreeMap::new);
                if let Some(current) = requests.get(&resource) {
                    let scale = match action {
                        ActionType::Increase => RESOURCE_SCALE_FACTOR,
                        ActionType::Decrease => 1.0 / RESOURCE_SCALE_FACTOR,
                    };
                    let new_value = scale_quantity(&current.0, scale)?;
                    requests.insert(resource, Quantity(new_value));
                }
            },
            ResourceAction::Limit { .. } => todo!(),
            ResourceAction::Claim => todo!(),
        }

        let mut next_state = self.clone();
        next_state.deployments.insert(deployment_name, deployment);
        Some(next_state)
    }

    fn perform_action(
        &self,
        ClusterAction { deployment_name, deployment_action: action_type }: ClusterAction,
        candidate_deployments: &BTreeMap<String, Deployment>,
    ) -> Option<Self> {
        match action_type {
            DeploymentAction::ReplicaCount(ActionType::Increase) => {
                self.change_replica_count(deployment_name, REPLICA_COUNT_CHANGE)
            },
            DeploymentAction::ReplicaCount(ActionType::Decrease) => {
                self.change_replica_count(deployment_name, -REPLICA_COUNT_CHANGE)
            },
            DeploymentAction::Object(ObjectAction::Create) => {
                self.create_deployment(&deployment_name, candidate_deployments)
            },
            DeploymentAction::Object(ObjectAction::Delete) => self.delete_deployment(&deployment_name),
            DeploymentAction::Container { name: container_name, action } => match action {
                ContainerAction::Resource(resource_action) => {
                    self.resource_request(deployment_name, container_name, resource_action)
                },
            },
        }
    }

    fn enumerate_actions(&self, candidate_deployments: &BTreeMap<String, Deployment>) -> Vec<ClusterAction> {
        let mut actions = Vec::new();

        // across all candidate deployments, we can try to create/delete according to whether the deployment
        // is already present
        for name in candidate_deployments.keys() {
            if self.deployments.contains_key(name) {
                // already created, so we can delete
                actions.push(ClusterAction {
                    deployment_name: name.clone(),
                    deployment_action: DeploymentAction::Object(ObjectAction::Delete),
                });
            } else {
                // not already created, so we can create
                actions.push(ClusterAction {
                    deployment_name: name.clone(),
                    deployment_action: DeploymentAction::Object(ObjectAction::Create),
                });
            }
        }

        // across all active deployments, we can try to increment/decrement, saving bounds checks for later
        for deployment in self.deployments.values() {
            let Some(deployment_name) = deployment.metadata.name.clone() else {
                continue;
            };

            actions.push(ClusterAction {
                deployment_name: deployment_name.clone(),
                deployment_action: DeploymentAction::ReplicaCount(ActionType::Increase),
            });
            actions.push(ClusterAction {
                deployment_name: deployment_name.clone(),
                deployment_action: DeploymentAction::ReplicaCount(ActionType::Decrease),
            });

            deployment
                .spec
                .as_ref()
                .and_then(|s| s.template.spec.as_ref())
                .map(|s| &s.containers)
                .into_iter()
                .flatten()
                .flat_map(|container| {
                    itertools::iproduct!([ActionType::Increase, ActionType::Decrease], ["memory", "cpu"]).map(
                        |(action, resource)| ClusterAction {
                            deployment_name: deployment_name.clone(),
                            deployment_action: DeploymentAction::Container {
                                name: container.name.clone(),
                                action: ContainerAction::Resource(ResourceAction::Request {
                                    resource: resource.to_string(),
                                    action,
                                }),
                            },
                        },
                    )
                })
                .for_each(|action| actions.push(action));
        }

        actions
    }

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

#[derive(Debug, Clone)]
struct Edge {
    action: ClusterAction,
    trace_event: TraceEvent,
}

type Walk = Vec<(Option<Edge>, Node)>;

struct ClusterGraph {
    candidate_deployments: BTreeMap<String, Deployment>,
    graph: DiGraph<Node, Edge>,
}

impl ClusterGraph {
    fn new(candidate_deployments: BTreeMap<String, Deployment>, starting_state: Vec<Node>, trace_length: u64) -> Self {
        let mut cluster_graph = Self { candidate_deployments, graph: DiGraph::new() };

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
        // perform bfs to a depth of trace_length. Queue item: `(depth, deployment)`
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

    fn generate_walks(&self, trace_length: u64) -> Vec<Walk> {
        let start_nodes: Vec<NodeIndex> = self.graph.node_indices().take(1).collect();
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

    fn dfs_walks(&self, current_node: NodeIndex, walk_length: u64) -> Vec<Vec<NodeIndex>> {
        let mut walks = Vec::new();

        let start_walk = vec![current_node];
        self.dfs_walks_helper(current_node, start_walk, walk_length, &mut walks);

        walks
    }

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
                .map(|(name, dep)| {
                    let replicas = dep.spec.as_ref().and_then(|s| s.replicas.as_ref()).unwrap_or(&1);

                    let resources = dep
                        .spec
                        .as_ref()
                        .and_then(|s| s.template.spec.as_ref())
                        .and_then(|s| s.containers.first())
                        .and_then(|c| c.resources.as_ref())
                        .map(|r| {
                            let requests = r
                                .requests
                                .as_ref()
                                .map(|reqs| {
                                    reqs.iter().map(|(k, v)| format!("{}={}", k, v.0)).collect::<Vec<_>>().join(",")
                                })
                                .unwrap_or_default();
                            format!(" [{}]", requests)
                        })
                        .unwrap_or_default();

                    format!("{}: {}{}", name, replicas, resources)
                })
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
                action.deployment_name.replace('"', "\\\"") // Escape any quotes in the name
            )
            .unwrap();
        }

        writeln!(&mut dot, "}}").unwrap();
        dot
    }

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
                        match edge.action.deployment_action {
                            DeploymentAction::ReplicaCount(_) | DeploymentAction::Container { .. } => {
                                SCALE_ACTION_PROBABILITY
                            },
                            DeploymentAction::Object(_) => CREATE_DELETE_ACTION_PROBABILITY,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    let input_trace_data: Vec<u8> = std::fs::read(&cli.input_trace)?;

    let trace = TraceStore::import(input_trace_data, &None)?;

    let (nodes, candidate_deployments) = Node::from_trace_store(&trace);


    // Construct the graph by searching all valid sequences of `trace_length`-1 actions from the
    // starting state for a total of `trace_length` nodes.
    let starting_state = vec![nodes[nodes.len() - 1].clone()];
    // let starting_state = nodes[..1].to_vec();
    let graph = ClusterGraph::new(candidate_deployments.clone(), starting_state, cli.trace_length);

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


        if let Some(output_dir) = &cli.traces_output_dir {
            write_debug_info(&candidate_deployments, &nodes, output_dir)?;
        }
        display_walks_and_traces(&walks, &traces, &cli)?;
    }

    Ok(())
}

fn tracestore_from_walk(walk: &Walk) -> TraceStore {
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
