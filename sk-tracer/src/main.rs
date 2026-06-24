use std::ops::Deref;
use std::sync::Arc;

use clap::Parser;
use kube::Client;
use rocket::serde::json::Json;
use sk_api::v1::ExportRequest;
use sk_core::external_storage::SkObjectStore;
use sk_core::logging;
use sk_core::prelude::*;
use sk_tracer::errors::ExportResponseError;
use sk_tracer::export::export_helper;
use sk_tracer::manager::TraceManager;
use sk_tracer::store::TraceStore;
use tokio::sync::Mutex;
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

#[rocket::post("/export", data = "<req>")]
async fn export(
    req: Json<ExportRequest>,
    store: &rocket::State<Arc<Mutex<TraceStore>>>,
) -> Result<Vec<u8>, ExportResponseError> {
    info!("export called with {:?}", req);

    let object_store = SkObjectStore::new(&req.export_path)?;
    let res = export_helper(req.deref(), store.deref().clone(), &object_store).await;

    // anyhow::Error Debug implementation prints the entire chain of errors, but once this gets
    // sucked up into rocket it no longer knows anything about that, so here we print the full
    // error first before returning the result.
    if let Err(e) = res.as_ref() {
        error!("{:?}", e);
    }
    res.map_err(|e| e.into())
}

#[instrument(ret, err)]
async fn run(args: Options) -> EmptyResult {
    let config = TracerConfig::load(&args.config_file)?.normalize()?;
    let client = Client::try_default().await.expect("failed to create kube client");
    let manager = TraceManager::start(client, config).await?;
    let store = manager.get_store();

    let rkt_config = rocket::Config { port: args.server_port, ..Default::default() };
    let server = rocket::custom(&rkt_config).mount("/", rocket::routes![export]).manage(store);

    server.launch().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity);
    run(args).await
}
