use std::sync::{
    Arc,
    Mutex,
};

use clap::Parser;
use kube::Client;
use rocket::serde::json::Json;
use serde::Deserialize;
use simkube::k8s::ApiSet;
use simkube::prelude::*;
use simkube::store::{
    TraceFilter,
    TraceStore,
};
use simkube::watch::{
    DynObjWatcher,
    PodWatcher,
};
use tracing::*;

#[derive(Parser, Debug)]
struct Options {
    #[arg(short, long)]
    config_file: String,

    #[arg(long)]
    server_port: u16,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[derive(Deserialize, Debug)]
struct ExportRequest {
    start_ts: i64,
    end_ts: i64,
    filter: TraceFilter,
}

#[rocket::post("/export", data = "<req>")]
async fn export(req: Json<ExportRequest>, store: &rocket::State<Arc<Mutex<TraceStore>>>) -> Result<Vec<u8>, String> {
    debug!("export called with {:?}", req);
    store
        .lock()
        .unwrap()
        .export(req.start_ts, req.end_ts, &req.filter)
        .map_err(|e| format!("{:?}", e))
}

async fn run(args: &Options) -> EmptyResult {
    info!("Reading tracer configuration from {}", &args.config_file);
    let config = TracerConfig::load(&args.config_file)?;

    let client = Client::try_default().await.expect("failed to create kube client");
    let mut apiset = ApiSet::new(client.clone());

    let store = TraceStore::new(config.clone());
    let dyn_obj_watcher = DynObjWatcher::new(store.clone(), &mut apiset, &config.tracked_objects).await?;
    let pod_watcher = PodWatcher::new(store.clone(), apiset);

    let rkt_config = rocket::Config { port: args.server_port, ..Default::default() };
    let server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![export])
        .manage(store.clone());

    tokio::select! {
        _ = tokio::spawn(dyn_obj_watcher.start()) => warn!("object watcher terminated"),
        _ = tokio::spawn(pod_watcher.start()) => warn!("pod watcher terminated"),
        _ = tokio::spawn(server.launch()) => warn!("server terminated"),
    };

    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity)?;
    run(&args).await?;
    Ok(())
}
