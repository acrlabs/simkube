#![deny(rustdoc::broken_intra_doc_links)]

use petgraph::dot::Dot;
use sk_core::jsonutils::{ordered_eq, ordered_hash};
mod contraction_hierarchies;
mod output;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

use clap::Parser;
use contraction_hierarchies::{CHNode, Distance};
use kube::api::DynamicObject;
use kube::Resource;
use ordered_float::OrderedFloat;
use petgraph::prelude::*;
use serde::{Deserialize, Serialize};
use sk_core::k8s::GVK;
use sk_store::{ExportedTrace, TraceEvent};
use jaq_core::{load, Compiler, Ctx, FilterT, RcIter};
use jaq_json::{Val, Error as JaqError};
use serde_json::Value as JsonValue;

use crate::contraction_hierarchies::CHEdge;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    num_samples: usize,
    #[arg(short = 'l', long)]
    trace_length: u64,
    #[arg(short, long)]
    input_traces: Vec<PathBuf>,
    #[arg(short = 'g', long)]
    graph_output_file: Option<PathBuf>,
    #[arg(short = 'o', long)]
    traces_output_dir: PathBuf,
    #[arg(short = 'v', long)]
    verbose: bool,
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
        let gvk = GVK::from_dynamic_obj(&value).unwrap();
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
struct Node {
    object_type: ObjectType,
    objects: BTreeMap<ObjectKey, DynamicObjectNewType>,
    ts: i64,
}

impl Node {
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

        for obj in patch.applied_objs.iter() {
            next_node.objects.insert(ObjectKey::from(obj), obj.clone().into());
        }

        for obj in patch.deleted_objs.iter() {
            next_node.objects.remove(&ObjectKey::from(obj));
        }

        Ok(next_node)
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
    fn probability(&self) -> ordered_float::OrderedFloat<f64> {
        self.action.probability.into()
    }
}

pub trait NextStateFn {
    fn next_states(&self, node: &Node) -> Vec<Action>; // TODO either vec is empty or enforce probabilities sum to 1.0
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
        // TODO: fields should be placed in a deterministic order before hashing
        let json = serde_json::to_string(&self.dynamic_object).unwrap();
        json.hash(state);
    }
}

struct Simulation<F>
where
    F: Fn(&Node) -> Vec<Action>,
{
    state_graph: DiGraph<Node, Edge>,
    next_action_fn: F,
}

impl<F> Simulation<F>
where
    F: Fn(&Node) -> Vec<Action>,
{
    fn new(next_action_fn: F, input_traces: Vec<Vec<TraceEvent>>) -> Self {
        // TODO: rectify clone overuse

        let args = Cli::parse();

        let mut state_graph = DiGraph::new();

        let mut node_to_index: HashMap<Node, NodeIndex> = HashMap::new();

        // Import traces and add to state graph

        // TODO: extract into separate trace_import function
        for trace in input_traces {
            let mut trace = trace.into_iter();

            let Some(first_event) = trace.next() else {
                continue;
            };

            for deleted_object in first_event.deleted_objs.iter() {
                eprint!("Warning: ignoring deleted object in first event of trace: {:?}", deleted_object);
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
                let next_node = current_node.apply_patch(&event).unwrap();

                let edge = Edge {
                    object_type: ObjectType::Observed,
                    action: Action {
                        patch: event.into(),
                        probability: 1.0.into(),                           // TODO
                        message: Some("generated from trace".to_string()), // TODO: include file of origin, perhaps in an enum field of{Synthetic, Observed {file: "..."}}
                    },
                };

                // TODO: this is probably redundant, maybe we could track the node index instead
                let current_node_index = node_to_index
                    .entry(current_node.clone())
                    .or_insert(
                        state_graph.add_node(current_node.clone()), // TODO make sure this doesn't run when the entry is found
                    )
                    .clone();

                let next_node_index = node_to_index
                    .entry(next_node.clone())
                    .or_insert(
                        state_graph.add_node(next_node.clone()), // TODO make sure this doesn't run when the entry is found
                    )
                    .clone();

                state_graph.add_edge(current_node_index, next_node_index, edge);
                current_node = next_node;
            }
        }

        // Enumerate actions against observed state graph
        let starting_state = state_graph.node_indices();

        let mut bfs_queue: VecDeque<(u64, NodeIndex)> = VecDeque::new();
        for node in starting_state {
            bfs_queue.push_back((1, node));
        }
        let mut visited = HashSet::new();

        let mut first = true;

        while let Some((depth, node_idx)) = bfs_queue.pop_front() {
            if depth >= args.trace_length {
                continue;
            }

            let not_previously_seen = visited.insert(node_idx.clone());
            if !not_previously_seen {
                continue;
            }

            let current_node = &state_graph[node_idx].clone();

            for action in next_action_fn(&state_graph[node_idx]) {
                let next_node = current_node
                    .apply_patch(&action.patch)
                    .expect("next_action_fn: F should generate only valid actions.");

                let next_idx = *node_to_index
                    .entry(next_node.clone())
                    .or_insert(state_graph.add_node(next_node.clone()));

                bfs_queue.push_back((depth + 1, next_idx.clone()));

                let edge = Edge { object_type: ObjectType::Synthetic, action };

                state_graph.add_edge(node_idx, next_idx, edge); // TODO, is adding duplicate edges between the same nodes intended?
            }
        }

        println!("state_graph: {} nodes", state_graph.node_count());

        let graphable = state_graph.map(
            |i, n| {
                format!("{} -- {:?}", i.index(), n.object_type)
            },
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
        let state_graph_dot = Dot::new(&graphable);
        let mut file = File::create("state_graph.dot").unwrap();
        use std::io::Write;
        write!(file, "{}", format!("{}", state_graph_dot)).unwrap();

        // conduct CH
        let heuristic_graph = crate::contraction_hierarchies::HeuristicGraph::new(state_graph.clone());
        let contraction_order = heuristic_graph.contraction_order();
        let contraction_order = contraction_order[0..contraction_order.len()/2].to_vec();
        let core_graph_num = contraction_order.len() - 1;

        let mut ch = crate::contraction_hierarchies::CH::new(state_graph, contraction_order.into_iter());

        let core_graph = ch.core_graph(core_graph_num).unwrap();
        let contraction_hierarchy = ch.contraction_hierarchy().unwrap();
        // write core graph

        // write core graph dot to a file


        let graphable = core_graph.map(
            |i, n| {

                match n {
                    CHNode::Original { node } => format!("{} -- Original {:?}", i.index(), node.object_type),
                    CHNode::Contracted { node, iteration } => format!("{} -- Contracted {:?}", i.index(), node.object_type),
                }
            },
            |i, e| {

                match e {
                    CHEdge::Original { edge } => format!("{:?}", edge.action.message),
                    CHEdge::Shortcut { edges, nodes, iteration } => format!("{} -- Shortcut {:?}", i.index(), edges.iter().map(|e| e.action.message.clone()).collect::<Vec<_>>()),
                    CHEdge::Orphaned { edge, iteration } => format!("{} -- Orphaned {:?}", i.index(), edge.action.message),
                }

            },
        );

        let core_graph_dot = Dot::new(&graphable);
        let mut file = File::create("core_graph.dot").unwrap();
        write!(file, "{}", format!("{}", core_graph_dot)).unwrap();

        // sample traces

        todo!()
    }
}

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
            | "\((.num | tonumber) / 2)\(.unit)")
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

    let increment_replica_script = r#"
        [
        range(0; length) as $i |
        [ .[] | . ] |
        .[$i].spec.replicas |= . + 1
        ]
        "#;

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

    let objects_json = serde_json::to_value(
        node.objects.values().map(|d| d.dynamic_object.clone()).collect::<Vec<_>>()
    ).expect("Failed to serialize objects to JSON");

    action_scripts
        .into_iter()
        .flat_map(|(action_message, jq_script)| {
            let message = Some(action_message.to_string());
            println!("message: {:?}", message);

            // Parse and execute the jaq filter
            let program = load::File { code: jq_script, path: () };
            
            use load::{Arena, Loader};
            let arena = Arena::default();
            let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
            
            // Load the filter modules
            let modules = match loader.load(&arena, program) {
                Ok(modules) => modules,
                Err(err) => {
                    eprintln!("Failed to load jaq script for action '{}': {:?}", action_message, err);
                    return Vec::new();
                }
            };
            
            // Compile the filter
            let filter = match Compiler::default()
                .with_funs(jaq_std::funs().chain(jaq_json::funs()))
                .compile(modules) {
                    Ok(filter) => filter,
                    Err(err) => {
                        eprintln!("Failed to compile jaq script for action '{}': {:?}", action_message, err);
                        return Vec::new();
                    }
                };
            
            let inputs = RcIter::new(core::iter::empty());
            
            // Run the filter with our input
            let mut output = filter.run((Ctx::new([], &inputs), Val::from(objects_json.clone())));
            
            // Collect all results
            let mut results = Vec::new();
            while let Some(result) = output.next() {
                match result {
                    Ok(val) => {
                        match serde_json::from_value::<Vec<Vec<DynamicObject>>>(val.into()) {
                            Ok(dynamic_object_list) => {
                                results.push(dynamic_object_list);
                            },
                            Err(e) => {
                                eprintln!("Error deserializing jaq result for action '{}': {}. Input was {} objects", 
                                    action_message, e, node.objects.len());
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("Error running jaq filter for action '{}': {}", action_message, e);
                    }
                }
            }
            
            // If we have multiple results, just take the first one for simplicity
            let objects_list = results.into_iter().flat_map(|dynamic_object_list| {
                dynamic_object_list
                    .into_iter()
                    .map(|dynamic_object_list| {
                        dynamic_object_list
                            .into_iter()
                            .map(|obj| (ObjectKey::from(&obj), DynamicObjectNewType { dynamic_object: obj }))
                            .collect::<BTreeMap<_, _>>()
                    })
                    .collect::<Vec<_>>()
            }).collect::<Vec<_>>();

            objects_list
                .into_iter()
                .map(|objects| {
                    // Find created or updated objects
                    let applied_objs = objects
                        .iter()
                        .filter(|(key, new_value)| {
                            node.objects.get(key).map_or(true, |old_value| old_value != *new_value)
                        })
                        .map(|(_, obj)| obj.dynamic_object.clone())
                        .collect::<Vec<_>>();

                    // Find deleted objects
                    let deleted_objs = node
                        .objects
                        .iter()
                        .filter(|(key, _)| !objects.contains_key(key))
                        .map(|(_, obj)| obj.dynamic_object.clone())
                        .collect::<Vec<_>>();

                    Action {
                        patch: TraceEvent { ts: node.ts + 1, applied_objs, deleted_objs }.into(),
                        probability: 1.0.into(), // TODO
                        message: message.clone(),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let input_traces = args
        .input_traces
        .iter()
        .map(|path| {
            let file = File::open(path).unwrap();
            let trace: ExportedTrace = rmp_serde::from_read(&file).unwrap();
            trace.events()
        })
        .collect();

    let simulation = Simulation::new(next_action_fn, input_traces);

    Ok(())
}
