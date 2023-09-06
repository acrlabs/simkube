#![allow(clippy::needless_return)]
mod controller;
mod trace;

use std::sync::Arc;

use clap::Parser;
use futures::{
    future,
    StreamExt,
};
use kube::runtime::controller::Controller;
use simkube::prelude::*;
use tracing::*;

use crate::controller::{
    error_policy,
    reconcile,
    SimulationContext,
};

#[derive(Parser, Debug)]
struct Options {
    #[arg(long)]
    driver_image: String,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args = Options::parse();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    info!("Simulation controller starting");

    let k8s_client = kube::Client::try_default().await.expect("failed to create kube client");
    let sim_api = kube::Api::<Simulation>::all(k8s_client.clone());
    let sim_root_api = kube::Api::<SimulationRoot>::all(k8s_client.clone());

    let ctrl = Controller::new(sim_api, Default::default())
        .owns(sim_root_api, Default::default())
        .run(reconcile, error_policy, Arc::new(SimulationContext { k8s_client, driver_image: args.driver_image }))
        .for_each(|_| future::ready(()));

    tokio::select!(
        _ = ctrl => info!("controller exited")
    );

    info!("shutting down...");
    return Ok(());
}
