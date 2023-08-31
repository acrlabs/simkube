#![allow(clippy::needless_return)]
use std::cmp::max;
use std::collections::{
    BTreeMap,
    HashMap,
};
use std::fs;
use std::time::Duration;

use clap::Parser;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::{
    DeleteParams,
    PostParams,
};
use kube::ResourceExt;
use simkube::prelude::*;
use simkube::util::add_common_fields;
use simkube::watchertracer::Tracer;
use tokio::time::sleep;
use tracing::*;

#[derive(Parser, Debug)]
struct Options {
    #[arg(long)]
    sim_name: String,

    #[arg(long)]
    sim_root: String,

    #[arg(long)]
    sim_namespace_prefix: String,

    #[arg(long)]
    trace_path: String,
}

fn make_virtual_namespace(
    sim_name: &str,
    ns_name: &str,
    sim_root: &SimulationRoot,
) -> SimKubeResult<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: metav1::ObjectMeta {
            name: Some(ns_name.into()),
            ..metav1::ObjectMeta::default()
        },
        ..corev1::Namespace::default()
    };
    add_common_fields(sim_name, sim_root, &mut ns)?;

    return Ok(ns);
}

#[tokio::main]
async fn main() -> SimKubeResult<()> {
    let args = Options::parse();
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    info!("Simulation driver starting");

    let trace_data = fs::read(args.trace_path)?;
    let tracer = Tracer::import(trace_data)?;

    let k8s_client = kube::Client::try_default().await?;
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(k8s_client.clone());
    let ns_api: kube::Api<corev1::Namespace> = kube::Api::all(k8s_client.clone());

    let root = roots_api.get(&args.sim_root).await?;

    let mut sim_ts = match tracer.iter().next() {
        Some((_, Some(ts))) => ts,
        _ => panic!("no trace data"),
    };

    let mut pod_apis: HashMap<String, kube::Api<corev1::Pod>> = HashMap::new();

    let mut trace_iter = tracer.iter();
    while let Some((evt, Some(next_ts))) = trace_iter.next() {
        for pod in evt.created_pods {
            let virtual_ns_name = format!("{}-{}", args.sim_namespace_prefix, pod.namespace().unwrap());
            if !pod_apis.contains_key(&virtual_ns_name) {
                let ns = make_virtual_namespace(&args.sim_name, &virtual_ns_name, &root)?;
                ns_api.create(&PostParams::default(), &ns).await?;
                pod_apis.insert(virtual_ns_name.clone(), kube::Api::namespaced(k8s_client.clone(), &virtual_ns_name));
            }

            let mut replay_pod = pod.clone();
            let selector: BTreeMap<String, String> = BTreeMap::from([("type".into(), "virtual".into())]);
            replay_pod.metadata.namespace = Some(virtual_ns_name.clone());
            replay_pod.metadata.resource_version = None;
            replay_pod.metadata.managed_fields = None;
            let spec = replay_pod.spec.as_mut().unwrap();
            spec.node_selector = Some(selector);
            spec.service_account = None;
            spec.service_account_name = None;
            replay_pod.status = None;

            add_common_fields(&args.sim_name, &root, &mut replay_pod)?;

            info!("creating pod {:?}", replay_pod);
            let pod_api = pod_apis.get(&virtual_ns_name).unwrap();
            pod_api.create(&PostParams::default(), &replay_pod).await?;
        }

        for pod in evt.deleted_pods {
            info!("deleting pod {}", pod.name_any());
            let virtual_ns_name = format!("{}-{}", args.sim_namespace_prefix, pod.namespace().unwrap());
            match pod_apis.get(&virtual_ns_name) {
                Some(pod_api) => {
                    pod_api.delete(&pod.name_any(), &DeleteParams::default()).await?;
                },
                None => warn!("could not find namespace"),
            }
        }

        let sleep_duration = max(0, next_ts - sim_ts);
        sim_ts = next_ts;

        info!("next event happens in {} seconds, sleeping", sleep_duration);
        sleep(Duration::from_secs(sleep_duration as u64)).await;
    }

    info!("trace over, cleaning up");
    roots_api.delete(&root.name_any(), &DeleteParams::default()).await?;
    info!("simulation complete!");

    Ok(())
}
