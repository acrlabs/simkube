use std::collections::{
    HashMap,
    VecDeque,
};
use std::path::PathBuf;

use clap::Parser;
use kube::api::DynamicObject;
use petgraph::algo::all_simple_paths;
use petgraph::prelude::*;
use sk_core::jsonutils;
use sk_core::k8s::GVK;
use sk_core::prelude::*;
use sk_store::{
    PodLifecyclesMap,
    TraceEvent,
    TracerConfig,
};

use serde_json::json;
use sk_store::TrackedObjectConfig;

/// This tool generates synthetic traces of length <trace_length> on a minimal deployment for a given set of replica
/// counts, starting and ending at the first replica count.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// output to stdout
    #[arg(short, long, default_value = "false")]
    display: bool,

    /// comma separated list of replica counts. Ex: "1,2,3,4"
    /// Walks start and end at the first replica count.
    #[arg(short, long, value_name = "REPLICA_COUNT")]
    replica_counts: String,

    /// trace length (>= 3)
    #[arg(short, long, value_name = "TRACE_LENGTH")]
    trace_length: u64,

    // output dir
    #[arg(short, long, value_name = "OUTPUT_DIR")]
    output_dir: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let trace_length = cli.trace_length;
    if trace_length < 3 {
        panic!("trace length must be >= 3");
    }
    let replica_counts = cli.replica_counts;

    let mut graph = DiGraph::<DynamicObject, ()>::new();

    let replica_counts = replica_counts.split(',').map(|s| s.trim().parse().unwrap()).collect::<Vec<_>>();

    let nodes = replica_counts.into_iter().map(create_deployment).collect::<Vec<_>>();

    // generate complete graph
    for node in nodes.iter() {
        graph.add_node(node.clone());
    }
    for i in 0..nodes.len() {
        for j in 0..nodes.len() {
            if i != j {
                graph.add_edge(NodeIndex::new(i), NodeIndex::new(j), ()); // <- here is where the weights are added
            }
        }
    }

    // enumerate all cycles with no revisits other than the first node.
    let walks = (1..nodes.len()).flat_map(|i| {
        let start = NodeIndex::new(0);
        let end = NodeIndex::new(i);
        let intermediate_nodes = (trace_length - 3) as usize;
        all_simple_paths(&graph, start, end, intermediate_nodes, Some(intermediate_nodes)).map(|walk: Vec<NodeIndex>| {
            eprintln!("walk from start to i: {:?}", walk);
            walk.into_iter()
                .map(|i| graph[i].clone())
                .chain(std::iter::once(graph[NodeIndex::new(0)].clone())) // return to start
                .collect()
        })
    });

    // ensure path exists
    if let Some(file) = &cli.output_dir {
        std::fs::create_dir_all(file).unwrap();
        // print directory
        println!("output directory: {:?}", file);
    }

    for (i, walk) in walks.into_iter().enumerate() {
        let data = generate_synthetic_trace(walk);

        let json_pretty = serde_json::to_string_pretty(&data).unwrap();

        if cli.display {
            println!("walk {}:\n{}", i, json_pretty);
        }

        if let Some(file) = &cli.output_dir {
            let data = rmp_serde::to_vec(&data).unwrap();
            let path = file.join(format!("trace-{}.mp", i));
            println!("writing to file: {:?}", path);
            std::fs::write(path, data).unwrap();
        }
    }
}


fn create_deployment(replica_count: u32) -> DynamicObject {
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
                "replicas": replica_count,
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

pub fn generate_synthetic_trace(
    deployments: Vec<DynamicObject>,
) -> (TracerConfig, VecDeque<TraceEvent>, HashMap<String, u64>, HashMap<String, PodLifecyclesMap>) {
    let mut events = VecDeque::new();
    let mut index = HashMap::new();
    let pod_lifecycles = HashMap::new();

    let base_ts = 1728334068;

    // Create TracerConfig
    let config = TracerConfig {
        tracked_objects: HashMap::from([(
            GVK::new("apps", "v1", "Deployment"),
            TrackedObjectConfig {
                track_lifecycle: true,
                pod_spec_template_path: "/spec/template".into(),
            },
        )]),
    };

    // Create Pod object (not currently used)
    let _pod = DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some("default".into()),
            name: Some("min-dep-hash".into()),
            owner_references: Some(vec![metav1::OwnerReference {
                api_version: "apps/v1".into(),
                kind: "Deployment".into(),
                name: "min-dep".into(),
                uid: "3f0f59d0-6a54-11ec-9d4e-0242ac130002".into(),
                ..Default::default()
            }]),
            ..Default::default()
        },
        types: Some(kube::api::TypeMeta {
            kind: "Pod".to_string(),
            api_version: "v1".to_string(),
        }),
        data: json!({
            "spec": {
                "containers": [{
                    "name": "minimal-container",
                    "image": "nginx:latest"
                }]
            }
        }),
    };


    // Not really sure whether the pod is created as an event or not
    // events.push_back(TraceEvent {
    //     ts: base_ts,
    //     applied_objs: vec![pod.clone()],
    //     deleted_objs: vec![],
    // });

    let mut ts = base_ts;
    // Create a deployment for each replica count
    for deployment in deployments {
        let deployment_hash = jsonutils::hash_option(deployment.data.get("spec"));
        index.insert(deployment.metadata.name.clone().unwrap(), deployment_hash);
        // trace event
        events.push_back(TraceEvent {
            ts,
            applied_objs: vec![deployment],
            deleted_objs: vec![],
        });
        ts += 5;
    }

    (config, events, index, pod_lifecycles)
}
