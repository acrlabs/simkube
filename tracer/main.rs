use std::sync::{
    Arc,
    Mutex,
};

use clap::Parser;
use kube::Client;
use rocket::serde::json::Json;
use serde::Deserialize;
use simkube::prelude::*;
use simkube::trace::{
    TraceFilter,
    Tracer,
};
use simkube::watchertracer::new_watcher_tracer;
use tracing::*;

#[derive(Parser, Debug)]
struct Options {
    #[arg(short, long)]
    config_file: String,

    #[arg(long)]
    server_port: u16,

    #[arg(short, long, default_value = "warn")]
    verbosity: String,
}

#[derive(Deserialize, Debug)]
struct ExportRequest {
    start_ts: i64,
    end_ts: i64,
    filter: TraceFilter,
}

#[rocket::post("/export", data = "<req>")]
async fn export(req: Json<ExportRequest>, tracer: &rocket::State<Arc<Mutex<Tracer>>>) -> Result<Vec<u8>, String> {
    debug!("export called with {:?}", req);
    tracer
        .lock()
        .unwrap()
        .export(req.start_ts, req.end_ts, &req.filter)
        .map_err(|e| format!("{:?}", e))
}

async fn run(args: &Options) -> EmptyResult {
    info!("Reading tracer configuration from {}", &args.config_file);
    let config = TracerConfig::load(&args.config_file)?;

    let client = Client::try_default().await.expect("failed to create kube client");
    let (mut watcher, tracer) = new_watcher_tracer(&config, client.clone()).await?;

    let rkt_config = rocket::Config { port: args.server_port, ..Default::default() };
    let server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![export])
        .manage(tracer.clone());

    tokio::select!(
        _ = watcher.start() => warn!("watcher finished"),
        _ = server.launch() => warn!("server failed"),
    );

    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity)?;
    run(&args).await?;
    Ok(())
}
