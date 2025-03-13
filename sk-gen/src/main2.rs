#![deny(rustdoc::broken_intra_doc_links)]


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
use std::sync::LazyLock;

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


static BASE_TS: LazyLock<i64> = LazyLock::new(|| {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
});

// Define the probability for various action types
const CREATE_DELETE_ACTION_PROBABILITY: f64 = 0.7;
const CUSTOM_ACTION_PROBABILITY: f64 = 0.3;


fn generate_diff(prev: &Node, next: &Node) -> Value {
    let prev_json = serde_json::to_value(prev).expect("Failed to serialize prev node");
    let next_json = serde_json::to_value(next).expect("Failed to serialize next node");

    serde_json::to_value(diff(&prev_json, &next_json)).expect("Failed to convert patch to value")
}

fn parse_trace_file(path: &PathBuf) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let exported_trace: ExportedTrace = serde_json::from_reader(reader)?;

    Ok(exported_trace
        .events()
        .iter()
        .map(|trace_event| {
            Node::from_objects(
                trace_event
                    .applied_objs
                    .iter()
                    .map(|obj| (obj.metadata.name.clone().unwrap(), obj.clone().into()))
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


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short = 'l', long, value_parser = clap::value_parser!(u64).range(3..))]
    trace_length: u64,

    #[arg(short, long)]
    object_count: usize,

    #[arg(short, long)]
    num_samples: Option<usize>,

    #[arg(short = 'g', long)]
    graph_output_file: Option<PathBuf>,

    #[arg(short = 'o', long)]
    traces_output_dir: Option<PathBuf>,

    #[arg(short = 'w', long)]
    display_walks: bool,

    #[arg(short = 'f', long)]
    trace_files: Option<Vec<PathBuf>>,
    
    #[arg(short = 'c', long)]
    custom_actions_file: Option<PathBuf>,
}


#[derive(Clone, Hash, PartialEq, Eq, Debug)]
enum ObjectAction {
    CreateObject,

    DeleteObject,

    CustomAction(String),
}


#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct ClusterAction {
    target_name: String,

    action_type: ObjectAction,
}


#[derive(Clone, Hash, PartialEq, Eq, Debug, Serialize)]
struct Node {
    objects: BTreeMap<String, dyn K8sObject>,
    timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DynamicObjectWrapper {
    dynamic_object: DynamicObject,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CustomActionDefinition {
    name: String,
    description: String,
    // The JSON path expression to target in the DynamicObject
    path: String,
    // The operation to apply (e.g., "add", "replace", "remove")
    operation: String,
    // Optional value to set (used for add/replace operations)
    value: Option<Value>,
    // Probability of this action being selected
    probability: f64,
}

struct DynamicAction {
    probability: f64,
    dynamic_action_type: DynamicActionType,
}

enum DynamicActionType {
    Create { applied: DynamicObject },
    Delete,
}

trait K8sObject: std::fmt::Debug {
    fn new_boxed() -> Box<Self>
    where
        Self: Sized;
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

    fn new() -> Self {
        Self { objects: BTreeMap::new(), timestamp: 0 }
    }

    fn create_object(&self, name: &str, candidate_objects: &BTreeMap<String, DynamicObjectWrapper>) -> Option<Self> {
        let object = candidate_objects.get(name)?;

        let mut next_state = self.clone();
        next_state.objects.insert(name.to_string(), object.clone());
        Some(next_state)
    }

    fn delete_object(&self, name: &str) -> Option<Self> {
        if self.objects.contains_key(name) {
            let mut next_state = self.clone();
            next_state.objects.remove(name);
            Some(next_state)
        } else {
            None
        }
    }

    fn apply_custom_action(&self, name: &str, action_name: &str, custom_actions: &[CustomActionDefinition]) -> Option<Self> {
        todo!()
    }

    fn perform_action(
        &self,
        ClusterAction { target_name: object_name, action_type }: ClusterAction,
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
        custom_actions: &[CustomActionDefinition],
    ) -> Option<Self> {
        let new_node = match action_type {
            ObjectAction::CreateObject => self.create_object(&object_name, candidate_objects),
            ObjectAction::DeleteObject => self.delete_object(&object_name),
            ObjectAction::CustomAction(action_name) => self.apply_custom_action(&object_name, &action_name, custom_actions),
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

    fn enumerate_actions(
        &self, 
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
        custom_actions: &[CustomActionDefinition],
    ) -> Vec<ClusterAction> {
        let mut actions = Vec::new();

        // Add create/delete actions
        for name in candidate_objects.keys() {
            if self.objects.contains_key(name) {
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: ObjectAction::DeleteObject,
                });
                
                // Add custom actions for existing objects
                for custom_action in custom_actions {
                    actions.push(ClusterAction {
                        target_name: name.clone(),
                        action_type: ObjectAction::CustomAction(custom_action.name.clone()),
                    });
                }
            } else {
                actions.push(ClusterAction {
                    target_name: name.clone(),
                    action_type: ObjectAction::CreateObject,
                });
            }
        }

        actions
    }

    fn valid_action_states(
        &self,
        candidate_objects: &BTreeMap<String, DynamicObjectWrapper>,
        custom_actions: &[CustomActionDefinition],
    ) -> Vec<(ClusterAction, Self)> {
        self.enumerate_actions(candidate_objects, custom_actions)
            .into_iter()
            .filter_map(|action| {
                self.perform_action(action.clone(), candidate_objects, custom_actions)
                    .map(|next_state| (action, next_state))
            })
            .collect()
    }
}


#[derive(Debug, Clone)]
struct Edge {
    action: ClusterAction,

    trace_event: TraceEvent,

    diff: Value,
}


type Walk = Vec<(Option<Edge>, Node)>;


struct ClusterGraph {
    candidate_objects: BTreeMap<String, DynamicObjectWrapper>,
    custom_actions: Vec<CustomActionDefinition>,
    graph: DiGraph<Node, Edge>,
}

impl ClusterGraph {
    fn new(
        candidate_objects: BTreeMap<String, DynamicObjectWrapper>,
        custom_actions: Vec<CustomActionDefinition>,
        starting_state: Vec<Node>,
        trace_length: u64,
    ) -> Self {
        let mut cluster_graph = Self { 
            candidate_objects, 
            custom_actions,
            graph: DiGraph::new() 
        };

        let mut node_to_index: HashMap<Node, NodeIndex> = HashMap::new();
        for node in &starting_state {
            let node_idx = cluster_graph.graph.add_node(node.clone());
            node_to_index.insert(node.clone(), node_idx);
        }


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

            node.valid_action_states(&cluster_graph.candidate_objects, &cluster_graph.custom_actions)
                .into_iter()
                .for_each(|(action, next_state)| {
                    let next_idx = *node_to_index.entry(next_state.clone()).or_insert_with(|| {
                        let node = cluster_graph.graph.add_node(next_state.clone());
                        bfs_queue.push_back((depth + 1, next_state.clone()));
                        node
                    });


                    let trace_event = gen_trace_event(*BASE_TS + depth as i64, &node, &next_state);

                    let diff = generate_diff(&node, &next_state);


                    cluster_graph
                        .graph
                        .update_edge(node_idx, next_idx, Edge { action, trace_event, diff });
                });
        }

        cluster_graph
    }

    fn generate_walks(&self, trace_length: u64) -> Vec<Walk> {
        let start_nodes: Vec<NodeIndex> = self.graph.node_indices().collect();
        let mut all_walks = Vec::new();


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


        for node_index in self.graph.node_indices() {
            let node = &self.graph[node_index];
            let label = node
                .objects
                .iter()
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
                action.target_name.replace('"', "\\\"")
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
                        match edge.action.action_type {
                            ObjectAction::CreateObject | ObjectAction::DeleteObject => CREATE_DELETE_ACTION_PROBABILITY,
                            ObjectAction::CustomAction(_) => CUSTOM_ACTION_PROBABILITY,
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

fn load_custom_actions(custom_actions_file: &Option<PathBuf>) -> Result<Vec<CustomActionDefinition>> {
    match custom_actions_file {
        Some(file_path) => {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);
            let custom_actions: Vec<CustomActionDefinition> = serde_json::from_reader(reader)?;
            Ok(custom_actions)
        }
        None => Ok(Vec::new()),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load custom actions if provided
    let custom_actions = load_custom_actions(&cli.custom_actions_file)?;

    let candidate_objects = generate_candidate_objects(cli.object_count);

    let mut starting_state = if let Some(trace_files) = &cli.trace_files {
        import_traces(trace_files)?
    } else {
        let target_name = candidate_objects
            .keys()
            .next()
            .expect("candidate_objects should not be empty")
            .clone();

        let a = Node::new();
        let b = a.create_object(&target_name, &candidate_objects).unwrap();

        vec![a, b]
    };


    let graph = ClusterGraph::new(candidate_objects, custom_actions, starting_state, cli.trace_length);


    if let Some(graph_output_file) = &cli.graph_output_file {
        export_graphviz(&graph, graph_output_file)?;
    }


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
            trace_store.create_or_update_obj(&obj, ts as i64, None);
        }

        for obj in trace_event.deleted_objs {
            trace_store.delete_obj(&obj, ts as i64);
        }
    }

    trace_store
}
