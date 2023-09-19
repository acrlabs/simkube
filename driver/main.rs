use std::cmp::max;
use std::collections::hash_map::Entry;
use std::collections::{
    BTreeMap,
    HashMap,
};
use std::fs;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::{
    DynamicObject,
    Patch,
    PatchParams,
};
use kube::ResourceExt;
use serde_json::json;
use simkube::jsonutils;
use simkube::k8s::{
    add_common_fields,
    get_api_resource,
    prefixed_ns,
    GVK,
};
use simkube::prelude::*;
use simkube::trace::Tracer;
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

fn build_virtual_ns(sim_name: &str, ns_name: &str, sim_root: &SimulationRoot) -> anyhow::Result<corev1::Namespace> {
    let mut ns = corev1::Namespace {
        metadata: metav1::ObjectMeta {
            name: Some(ns_name.into()),
            labels: Some(BTreeMap::from([(VIRTUAL_LABEL_KEY.into(), "true".into())])),
            ..Default::default()
        },
        ..Default::default()
    };
    add_common_fields(sim_name, sim_root, &mut ns)?;

    Ok(ns)
}

fn build_virtual_obj(
    obj: &DynamicObject,
    vns_name: &str,
    sim_name: &str,
    root: &SimulationRoot,
    config: &TracerConfig,
) -> anyhow::Result<DynamicObject> {
    let mut vobj = obj.clone();
    vobj.metadata.namespace = Some(vns_name.into());
    vobj.labels_mut().insert(VIRTUAL_LABEL_KEY.into(), "true".into());

    let gvk = GVK::from_dynamic_obj(obj)?;
    let psp = &config.tracked_objects[&gvk].pod_spec_path;

    jsonutils::patch_ext::add(psp, "nodeSelector", &json!({"type": "virtual"}), &mut vobj.data, true)?;
    jsonutils::patch_ext::add(psp, "tolerations", &json!([]), &mut vobj.data, false)?;
    jsonutils::patch_ext::add(
        &format!("{}/tolerations", psp),
        "-",
        &json!({"key": "simkube.io/virtual-node", "value": "true"}),
        &mut vobj.data,
        true,
    )?;
    jsonutils::patch_ext::remove(psp, "status", &mut vobj.data)?;
    add_common_fields(sim_name, root, &mut vobj)?;

    Ok(vobj)
}

async fn run(args: &Options) -> EmptyResult {
    info!("Simulation driver starting");

    let trace_data = fs::read(&args.trace_path)?;
    let tracer = Tracer::import(trace_data)?;

    let k8s_client = kube::Client::try_default().await?;
    let roots_api: kube::Api<SimulationRoot> = kube::Api::all(k8s_client.clone());
    let ns_api: kube::Api<corev1::Namespace> = kube::Api::all(k8s_client.clone());
    let mut obj_apis: HashMap<(GVK, String), kube::Api<DynamicObject>> = HashMap::new();

    let root = roots_api.get(&args.sim_root).await?;

    let mut sim_ts = tracer.start_ts().ok_or(anyhow!("no trace data"))?;
    for (evt, next_ts) in tracer.iter() {
        for obj in evt.applied_objs {
            let gvk = GVK::from_dynamic_obj(&obj)?;
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            let obj_api = match obj_apis.entry((gvk.clone(), vns_name.clone())) {
                Entry::Vacant(e) => {
                    let vns = build_virtual_ns(&args.sim_name, &vns_name, &root)?;
                    ns_api.create(&Default::default(), &vns).await?;
                    let (ar, _) = get_api_resource(&gvk, &k8s_client).await?;
                    e.insert(kube::Api::namespaced_with(k8s_client.clone(), &vns_name, &ar))
                },
                Entry::Occupied(e) => e.into_mut(),
            };

            let vobj = build_virtual_obj(&obj, &vns_name, &args.sim_name, &root, tracer.config())?;

            info!("applying object {:?}", vobj);
            obj_api
                .patch(&vobj.name_any(), &PatchParams::apply("simkube"), &Patch::Apply(&vobj))
                .await?;
        }

        for obj in evt.deleted_objs {
            info!("deleting pod {}", obj.name_any());
            let gvk = GVK::from_dynamic_obj(&obj)?;
            let vns_name = prefixed_ns(&args.sim_namespace_prefix, &obj);
            match obj_apis.get(&(gvk, vns_name)) {
                Some(obj_api) => _ = obj_api.delete(&obj.name_any(), &Default::default()).await?,
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

#[tokio::main]
async fn main() {
    let args = Options::parse();
    tracing_subscriber::fmt().with_max_level(Level::DEBUG).init();
    if let Err(e) = run(&args).await {
        error!("{e}");
        std::process::exit(1);
    }
}
