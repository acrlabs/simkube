mod cert_manager;
mod controller;
mod objects;
mod trace;

use std::sync::Arc;

use clap::Parser;
use futures::{
    future,
    StreamExt,
};
use kube::runtime::controller::Controller;
use kube::ResourceExt;
use simkube::prelude::*;
use thiserror::Error;
use tracing::*;

use crate::controller::{
    error_policy,
    reconcile,
};

#[derive(Clone, Debug, Parser)]
struct Options {
    #[arg(long)]
    driver_image: String,

    #[arg(long, default_value = DRIVER_ADMISSION_WEBHOOK_PORT)]
    driver_port: i32,

    #[arg(long)]
    sim_svc_account: String,

    // TODO: should support non-cert-manager for configuring certs as well
    #[arg(long)]
    use_cert_manager: bool,

    #[arg(long, default_value = "")]
    cert_manager_issuer: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[derive(Error, Debug)]
#[error(transparent)]
enum ReconcileError {
    AnyhowError(#[from] anyhow::Error),
    KubeApiError(#[from] kube::Error),
}

#[derive(Clone)]
struct SimulationContext {
    client: kube::Client,
    opts: Options,

    name: String,
    root: String,
    driver_ns: String,
    driver_name: String,
    driver_svc: String,
    webhook_name: String,
}

impl SimulationContext {
    fn new(client: kube::Client, opts: Options) -> SimulationContext {
        SimulationContext {
            client,
            opts,
            name: String::new(),
            root: String::new(),
            driver_ns: String::new(),
            driver_name: String::new(),
            driver_svc: String::new(),
            webhook_name: String::new(),
        }
    }

    fn new_with_sim(self: Arc<Self>, sim: &Simulation) -> SimulationContext {
        let mut new = (*self).clone();
        new.name = sim.name_any();
        new.root = format!("sk-{}-root", new.name);
        new.driver_name = format!("sk-{}-driver", new.name);
        new.driver_ns = sim.spec.driver_namespace.clone();
        new.driver_svc = format!("sk-{}-driver-svc", new.name);
        new.webhook_name = format!("sk-{}-mutatepods", new.name);

        new
    }
}

async fn run(opts: Options) -> EmptyResult {
    info!("Simulation controller starting");

    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    let ctrl = Controller::new(sim_api, Default::default())
        .run(reconcile, error_policy, Arc::new(SimulationContext::new(client, opts)))
        .for_each(|_| future::ready(()));

    tokio::select!(
        _ = ctrl => info!("controller exited")
    );

    info!("shutting down...");
    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity)?;
    run(args).await?;
    Ok(())
}
