mod mutation;
mod runner;

use std::fs;
use std::net::{
    IpAddr,
    Ipv4Addr,
};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use rocket::config::TlsConfig;
use simkube::k8s::{
    ApiSet,
    OwnersCache,
};
use simkube::prelude::*;
use simkube::store::{
    TraceStorable,
    TraceStore,
};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::*;

use crate::mutation::MutationData;
use crate::runner::TraceRunner;

#[derive(Clone, Debug, Parser)]
struct Options {
    #[arg(long)]
    sim_name: String,

    #[arg(long)]
    sim_root: String,

    #[arg(long)]
    virtual_ns_prefix: String,

    #[arg(long, default_value = DRIVER_ADMISSION_WEBHOOK_PORT)]
    admission_webhook_port: u16,

    #[arg(long)]
    cert_path: String,

    #[arg(long)]
    key_path: String,

    // This must be passed in as an arg instead of read from the simulation spec
    // because the location the trace is mounted in the pod will be different than
    // the location specified in the spec
    #[arg(long)]
    trace_mount_path: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[derive(Clone)]
pub struct DriverContext {
    name: String,
    sim_root: String,
    virtual_ns_prefix: String,
    owners_cache: Arc<Mutex<OwnersCache>>,
    store: Arc<dyn TraceStorable + Send + Sync>,
}

#[instrument(ret, err)]
async fn run(opts: Options) -> EmptyResult {
    let client = kube::Client::try_default().await?;
    let sim_api: kube::Api<Simulation> = kube::Api::all(client.clone());
    let sim = sim_api.get(&opts.sim_name).await?;

    let trace_data = fs::read(opts.trace_mount_path)?;
    let store = Arc::new(TraceStore::import(trace_data, &sim.spec.duration)?);

    let apiset = ApiSet::new(client.clone());
    let owners_cache = Arc::new(Mutex::new(OwnersCache::new(apiset)));
    let ctx = DriverContext {
        name: opts.sim_name.clone(),
        sim_root: opts.sim_root.clone(),
        virtual_ns_prefix: opts.virtual_ns_prefix.clone(),
        owners_cache,
        store,
    };

    let rkt_config = rocket::Config {
        address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: opts.admission_webhook_port,
        tls: Some(TlsConfig::from_paths(&opts.cert_path, &opts.key_path)),
        ..Default::default()
    };
    let server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![mutation::handler])
        .manage(MutationData::new())
        .manage(ctx.clone());

    let server_task = tokio::spawn(server.launch());

    // Give the mutation handler a bit of time to come online before starting the sim
    sleep(Duration::from_secs(5)).await;

    let runner = TraceRunner::new(ctx.clone()).await?;

    tokio::select! {
        res = server_task => Err(anyhow!("server terminated: {res:#?}")),
        res = tokio::spawn(runner.run()) => {
            match res {
                Ok(r) => r,
                Err(err) => Err(err.into()),
            }
        },
    }
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&format!("{},rocket=warn", args.verbosity));
    run(args).await
}

#[cfg(test)]
mod tests;
