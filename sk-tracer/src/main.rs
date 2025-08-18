mod errors;

use std::ops::Deref;
use std::sync::{
    Arc,
    Mutex,
};

use bytes::Bytes;
use clap::Parser;
use kube::Client;
use object_store::ObjectStoreScheme;
use rocket::serde::json::Json;
use sk_api::v1::ExportRequest;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::logging;
use sk_core::prelude::*;
use sk_store::{
    TraceManager,
    TraceStore,
    TracerConfig,
};
use tracing::*;

use crate::errors::ExportResponseError;

#[derive(Parser, Debug)]
struct Options {
    #[arg(short, long)]
    config_file: String,

    #[arg(long)]
    server_port: u16,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

async fn export_helper(
    req: &ExportRequest,
    store: Arc<Mutex<TraceStore>>,
    object_store: &(dyn ObjectStoreWrapper + Sync),
) -> anyhow::Result<Vec<u8>> {
    let trace_data = { store.lock().unwrap().export(req.start_ts, req.end_ts, &req.filters)? };

    match object_store.scheme() {
        // If we're writing to a cloud provider, we want to write from the location that the
        // tracer's running from, ostensibly to minimize transport costs.
        ObjectStoreScheme::AmazonS3 | ObjectStoreScheme::GoogleCloudStorage | ObjectStoreScheme::MicrosoftAzure => {
            object_store.put(Bytes::from(trace_data)).await?;
            Ok(vec![])
        },

        // On the other hand, if we're trying to write to local storage (or something else), it's
        // not going to do any good to write to local storage of the _tracer_, so we return all the
        // data and let the client do something with it.
        _ => Ok(trace_data),
    }
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
    let config = TracerConfig::load(&args.config_file)?;
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

#[cfg(test)]
mod tests;
