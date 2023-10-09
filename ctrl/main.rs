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
use simkube::prelude::*;
use thiserror::Error;
use tracing::*;

use crate::controller::{
    error_policy,
    reconcile,
};

#[derive(Parser, Debug)]
struct Options {
    #[arg(long)]
    driver_image: String,

    #[arg(long, default_value = "8888")]
    driver_port: i32,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[derive(Error, Debug)]
#[error(transparent)]
enum ReconcileError {
    AnyhowError(#[from] anyhow::Error),
    KubeApiError(#[from] kube::Error),
}

struct SimulationContext {
    k8s_client: kube::Client,
    driver_image: String,
    driver_port: i32,
    sim_svc_account: String,
}

async fn run(args: &Options) -> EmptyResult {
    info!("Simulation controller starting");

    let k8s_client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(k8s_client.clone());
    let sim_root_api = kube::Api::<SimulationRoot>::all(k8s_client.clone());

    let ctrl = Controller::new(sim_api, Default::default())
        .owns(sim_root_api, Default::default())
        .run(
            reconcile,
            error_policy,
            Arc::new(SimulationContext {
                k8s_client,
                driver_image: args.driver_image.clone(),
                driver_port: args.driver_port,
                // TODO don't hardcode this
                sim_svc_account: "sk-ctrl-service-account-c8688aad".into(),
            }),
        )
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
    run(&args).await?;
    Ok(())
}
