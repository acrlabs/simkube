mod mutation;
mod runner;

use std::fs;
use std::net::{
    IpAddr,
    Ipv4Addr,
};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use rocket::config::TlsConfig;
use simkube::k8s::{
    ApiSet,
    OwnersCache,
};
use simkube::prelude::*;
use simkube::store::TraceStore;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::*;

use crate::runner::TraceRunner;

#[derive(Parser, Debug)]
struct Options {
    #[arg(long)]
    sim_name: String,

    #[arg(long)]
    sim_root: String,

    #[arg(long)]
    sim_namespace_prefix: String,

    #[arg(long, default_value = DRIVER_ADMISSION_WEBHOOK_PORT)]
    admission_webhook_port: u16,

    #[arg(long)]
    cert_path: String,

    #[arg(long)]
    key_path: String,

    #[arg(long)]
    trace_path: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

pub struct DriverContext {
    sim_name: String,
    sim_root_name: String,
    owners_cache: Arc<Mutex<OwnersCache>>,
    store: Arc<TraceStore>,
}

async fn run(args: &Options) -> EmptyResult {
    info!("Simulation driver starting");

    let client = kube::Client::try_default().await?;

    let trace_data = fs::read(&args.trace_path)?;
    let apiset = ApiSet::new(client.clone());
    let store = Arc::new(TraceStore::import(trace_data)?);
    let ctx = DriverContext {
        sim_name: args.sim_name.clone(),
        sim_root_name: args.sim_root.clone(),
        owners_cache: Arc::new(Mutex::new(OwnersCache::new(apiset))),
        store: store.clone(),
    };

    let rkt_config = rocket::Config {
        address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: args.admission_webhook_port,
        tls: Some(TlsConfig::from_paths(&args.cert_path, &args.key_path)),
        ..Default::default()
    };
    let server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![mutation::handler])
        .manage(ctx);

    let server_task = tokio::spawn(server.launch());

    // Give the mutation handler a bit of time to come online before starting the sim:w
    sleep(Duration::from_secs(5)).await;

    let runner = TraceRunner::new(&args.sim_name, &args.sim_root, store.clone(), &args.sim_namespace_prefix).await?;

    tokio::select! {
        _ = server_task => warn!("server terminated"),
        res = tokio::spawn(runner.run()) => info!("simulation runner completed: {:?}", res),
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
