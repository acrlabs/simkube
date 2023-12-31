mod cert_manager;
mod controller;
mod objects;
mod trace;

use std::ops::Deref;
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
use crate::objects::*;

#[derive(Clone, Debug, Parser)]
struct Options {
    #[arg(long)]
    driver_image: String,

    #[arg(long, default_value = DRIVER_ADMISSION_WEBHOOK_PORT)]
    driver_port: i32,

    // TODO: should support non-cert-manager for configuring certs as well
    #[arg(long)]
    use_cert_manager: bool,

    #[arg(long, default_value = "")]
    cert_manager_issuer: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

// This is sortof a stupid hack, because anyhow::Error doesn't derive from
// std::error::Error, but the reconcile functions require you to return a
// result that derives from std::error::Error.  So we just wrap the anyhow,
// and then implement deref for it so we can get back to the underlying error
// wherever we actually care.
#[derive(Debug, Error)]
#[error(transparent)]
struct AnyhowError(#[from] anyhow::Error);

impl Deref for AnyhowError {
    type Target = anyhow::Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
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

#[instrument(ret, err)]
async fn run(opts: Options) -> EmptyResult {
    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    let ctrl = Controller::new(sim_api, Default::default())
        .run(reconcile, error_policy, Arc::new(SimulationContext::new(client, opts)))
        .for_each(|_| future::ready(()));

    ctrl.await;
    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity);
    run(args).await
}
