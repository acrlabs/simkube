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
use kube::api::DynamicObject;
use kube::ResourceExt;
use simkube::prelude::*;
use simkube::util::{
    add_common_fields,
    prefixed_ns,
};
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

fn build_virtual_ns(sim_name: &str, ns_name: &str, sim_root: &SimulationRoot) -> SimKubeResult<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: metav1::ObjectMeta {
            name: Some(ns_name.into()),
            labels: Some(BTreeMap::from([(VIRTUAL_LABEL_KEY.into(), "true".into())])),
            ..Default::default()
        },
        ..Default::default()
    };
    add_common_fields(sim_name, sim_root, &mut ns)?;

    return Ok(ns);
}

fn build_virtual_obj(
    obj: &DynamicObject,
    vns_name: &str,
    _sim_name: &str,
    _root: &SimulationRoot,
) -> SimKubeResult<DynamicObject> {
    let mut vobj = obj.clone();
    let _selector: BTreeMap<String, String> = BTreeMap::from([("type".into(), "virtual".into())]);
    vobj.metadata.namespace = Some(vns_name.into());
    vobj.metadata
        .labels
        .get_or_insert(BTreeMap::new())
        .insert(VIRTUAL_LABEL_KEY.into(), "true".into());


    // let spec = vobj.spec.as_mut().unwrap();
    // spec.node_selector = Some(selector);
    // spec.tolerations.get_or_insert(vec![]).push(corev1::Toleration {
    //     key: Some("simkube.io/virtual-node".into()),
    //     value: Some("true".into()),
    //     ..Default::default()
    // });
    // vobj.status = None;

    // add_common_fields(sim_name, root, &mut vobj)?;
    Ok(vobj)
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
    let mut pod_apis: HashMap<String, kube::Api<corev1::Pod>> = HashMap::new();

    let root = roots_api.get(&args.sim_root).await?;

    let mut sim_ts = tracer.start_ts().expect("no trace data");
    for (evt, next_ts) in tracer.iter() {
        for obj in evt.created_objs {
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            if !pod_apis.contains_key(&vns_name) {
                let vns = build_virtual_ns(&args.sim_name, &vns_name, &root)?;
                ns_api.create(&Default::default(), &vns).await?;
                pod_apis.insert(vns_name.clone(), kube::Api::namespaced(k8s_client.clone(), &vns_name));
            }

            let vobj = build_virtual_obj(&obj, &vns_name, &args.sim_name, &root)?;

            info!("creating object {:?}", vobj);
            // let pod_api = pod_apis.get(&vns_name).unwrap();
            // pod_api.create(&Default::default(), &vobj).await?;
        }

        for obj in evt.deleted_objs {
            info!("deleting pod {}", obj.name_any());
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            match pod_apis.get(&vns_name) {
                Some(pod_api) => _ = pod_api.delete(&obj.name_any(), &Default::default()).await?,
                None => warn!("could not find namespace"),
            }
        }

        if let Some(ts) = next_ts {
            let sleep_duration = max(0, ts - sim_ts);
            sim_ts = ts;
            info!("next event happens in {} seconds, sleeping", sleep_duration);
            sleep(Duration::from_secs(sleep_duration as u64)).await;
        }
    }

    info!("trace over, cleaning up");
    roots_api.delete(&root.name_any(), &Default::default()).await?;
    info!("simulation complete!");

    Ok(())
}
