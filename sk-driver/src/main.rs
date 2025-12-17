mod mutation;
mod runner;
mod util;

use std::env;
use std::net::{
    IpAddr,
    Ipv4Addr,
};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use clockabilly::UtcClock;
use rocket::config::TlsConfig;
use sk_core::errors::*;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::k8s::{
    DynamicApiSet,
    OwnersCache,
};
use sk_core::prelude::*;
use sk_core::{
    hooks,
    logging,
};
use sk_store::ExportedTrace;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::*;

use crate::mutation::MutationData;
use crate::runner::run_trace;
use crate::util::wait_if_paused;

#[derive(Clone, Debug, Parser)]
struct Options {
    #[arg(long)]
    sim_name: String,

    // Needed so the driver can update the lease
    #[arg(long)]
    controller_ns: String,

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
    trace_path: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[derive(Clone)]
pub struct DriverContext {
    name: String,
    sim_name: String,
    root_name: String,
    ctrl_ns: String,
    virtual_ns_prefix: String,
    owners_cache: Arc<Mutex<OwnersCache>>,
    trace: Arc<ExportedTrace>,
}

#[instrument(ret, err)]
async fn run(opts: Options) -> EmptyResult {
    let name = env::var(DRIVER_NAME_ENV_VAR)?;

    let client = kube::Client::try_default().await?;
    let sim_api: kube::Api<Simulation> = kube::Api::all(client.clone());

    let sim = sim_api.get(&opts.sim_name).await?;
    wait_if_paused(client.clone(), &sim.name_any(), UtcClock::boxed()).await?;
    let root_name = format!("{name}-root");

    let object_store = SkObjectStore::new(&opts.trace_path)?;
    let trace_data = object_store.get().await?.to_vec();
    let trace = Arc::new(ExportedTrace::import(trace_data, sim.spec.duration.as_ref())?);

    let apiset = DynamicApiSet::new(client.clone());
    let owners_cache = Arc::new(Mutex::new(OwnersCache::new(apiset)));
    let ctx = DriverContext {
        name,
        sim_name: opts.sim_name.clone(),
        root_name,
        ctrl_ns: opts.controller_ns.clone(),
        virtual_ns_prefix: sim.spec.driver.virtual_ns_prefix.clone(),
        owners_cache,
        trace,
    };

    let rkt_config = rocket::Config {
        address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port: opts.admission_webhook_port,
        tls: Some(TlsConfig::from_paths(&opts.cert_path, &opts.key_path)),
        ..Default::default()
    };
    let mutation_server = rocket::custom(&rkt_config)
        .mount("/", rocket::routes![mutation::handler])
        .manage(MutationData::new())
        .manage(ctx.clone())
        .manage(sim.clone());
    let mutation_task = tokio::spawn(mutation_server.launch());
    // Give the mutation handler and stage controller a bit of time to come online
    // before starting the sim
    sleep(Duration::from_secs(5)).await;

    hooks::execute(&sim, hooks::Type::PreRun).await?;

    let runner_task = tokio::spawn(run_trace(ctx.clone(), client, sim.clone()));
    tokio::select! {
        res = mutation_task => Err(anyhow!("mutation server terminated: {res:#?}")),
        res = runner_task => {
            match res {
                Ok(r) => r,
                Err(err) => Err(err.into()),
            }
        },
    }?;

    hooks::execute(&sim, hooks::Type::PostRun).await
}

#[tokio::main]
async fn main() {
    let args = Options::parse();
    logging::setup(&format!("{},rocket=warn", args.verbosity));
    if let Err(err) = run(args).await {
        skerr!(err, "driver failed");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
