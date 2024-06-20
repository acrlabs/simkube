mod errors;

use std::ops::Deref;
use std::sync::{
    Arc,
    Mutex,
};

use bytes::Bytes;
use clap::Parser;
use kube::Client;
use object_store::PutPayload;
use reqwest::Url;
use rocket::serde::json::Json;
use simkube::api::v1::ExportRequest;
use simkube::k8s::ApiSet;
use simkube::prelude::*;
use simkube::store::external_storage::{
    object_store_for_scheme,
    ObjectStoreScheme,
};
use simkube::store::TraceStore;
use simkube::watch::{
    DynObjWatcher,
    PodWatcher,
};

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

async fn export_helper(req: &ExportRequest, store: &Arc<Mutex<TraceStore>>) -> anyhow::Result<Vec<u8>> {
    let trace_data = store.lock().unwrap().export(req.start_ts, req.end_ts, &req.filters)?;

    let url = Url::parse(&req.export_path)?;
    let (scheme, path) = ObjectStoreScheme::parse(&url)?;
    match scheme {
        // If we're writing to a cloud provider, we want to write from the location that the
        // tracer's running from, ostensibly to minimize transport costs.
        ObjectStoreScheme::AmazonS3 | ObjectStoreScheme::GoogleCloudStorage | ObjectStoreScheme::MicrosoftAzure => {
            let store = object_store_for_scheme(&scheme, &req.export_path)?;
            let payload = PutPayload::from_bytes(Bytes::from(trace_data));
            store.put(&path, payload).await?;
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
    let res = export_helper(req.deref(), store).await;

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
    let mut apiset = ApiSet::new(client.clone());

    let store = Arc::new(Mutex::new(TraceStore::new(config.clone())));
    let (dyn_obj_watcher, _) = DynObjWatcher::new(store.clone(), &mut apiset, &config.tracked_objects).await?;
    let (pod_watcher, _) = PodWatcher::new(client, store.clone(), apiset);

    let rkt_config = rocket::Config { port: args.server_port, ..Default::default() };
    let server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![export])
        .manage(store.clone());

    tokio::select! {
        res = tokio::spawn(dyn_obj_watcher.start()) => res.map_err(|e| e.into()),
        res = tokio::spawn(pod_watcher.start()) => res.map_err(|e| e.into()),
        res = tokio::spawn(server.launch()) => match res {
            Ok(r) => r.map(|_| ()).map_err(|err| err.into()),
            Err(err) => Err(err.into()),
        },
    }
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity);
    run(args).await
}
