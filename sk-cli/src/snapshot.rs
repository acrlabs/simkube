use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

use clockabilly::{Clockable, UtcClock};
use sk_api::v1::ExportFilters;
use sk_core::k8s::ApiSet;
use sk_core::prelude::*;
use sk_store::watchers::{DynObjHandler, ObjWatcher, PodHandler};
use sk_store::{TraceStore, TracerConfig};
use tokio::task::JoinSet;

#[derive(clap::Args)]
pub struct Args {
    #[arg(short, long, long_help = "config file specifying resources to snapshot")]
    pub config_file: String,

    #[arg(
        long,
        long_help = "namespaces to exclude from the snapshot",
        value_delimiter = ',',
        default_value = "cert-manager,kube-system,local-path-storage,monitoring,simkube"
    )]
    pub excluded_namespaces: Vec<String>,

    #[arg(
        short,
        long,
        long_help = "location to save exported trace",
        default_value = "trace.out"
    )]
    pub output: String,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    println!("Reading config from {}...", args.config_file);
    let config = TracerConfig::load(&args.config_file)?;

    println!("Connecting to kubernetes cluster...");
    let client = kube::Client::try_default().await?;
    let mut apiset = ApiSet::new(client.clone());

    println!("Loading snapshot into store...");
    let store = Arc::new(Mutex::new(TraceStore::new(config.clone())));
    let mut js = JoinSet::new();
    let mut do_ready_rxs = vec![];
    for gvk in config.tracked_objects.keys() {
        let (dyn_obj_handler, dyn_obj_stream) = DynObjHandler::new_with_stream(gvk, &mut apiset).await?;
        let (dyn_obj_watcher, do_ready_rx) = ObjWatcher::new(dyn_obj_handler, dyn_obj_stream, store.clone());
        do_ready_rxs.push(do_ready_rx);
        js.spawn(dyn_obj_watcher.start());
    }

    let (pod_handler, pod_stream) = PodHandler::new_with_stream(client, apiset);
    let (pod_watcher, pod_ready_rx) = ObjWatcher::new(pod_handler, pod_stream, store.clone());
    js.spawn(pod_watcher.start());

    // the receivers block until they get a message, so don't actually care about the value
    for do_ready_rx in do_ready_rxs {
        let _ = do_ready_rx.recv();
    }
    let _ = pod_ready_rx.recv();

    js.shutdown().await;

    println!("Exporting snapshot data from store...");
    let filters = ExportFilters::new(args.excluded_namespaces.clone(), vec![]);
    let start_ts = UtcClock.now_ts();
    let end_ts = start_ts + 1;
    let data = store.lock().unwrap().export(start_ts, end_ts, &filters)?;

    println!("Writing trace file: {}", args.output);
    let mut file = File::create(&args.output)?;
    file.write_all(&data)?;

    println!("Done!");
    Ok(())
}
