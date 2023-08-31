#![allow(clippy::needless_return)]
mod controller;
mod trace;

use std::sync::Arc;

use futures::{
    future,
    StreamExt,
};
use kube::runtime::controller::Controller;
use kube::runtime::watcher;
use simkube::prelude::*;
use tracing::*;

use crate::controller::{
    error_policy,
    reconcile,
    SimulationContext,
};

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    info!("Simulation controller starting");

    let k8s_client = kube::Client::try_default().await.expect("failed to create kube client");
    let sim_api = kube::Api::<Simulation>::all(k8s_client.clone());
    let sim_root_api = kube::Api::<SimulationRoot>::all(k8s_client.clone());

    let ctrl = Controller::new(sim_api, watcher::Config::default())
        .owns(sim_root_api, watcher::Config::default())
        .run(reconcile, error_policy, Arc::new(SimulationContext { k8s_client }))
        .for_each(|_| future::ready(()));

    tokio::select!(
        _ = ctrl => info!("controller exited")
    );

    info!("shutting down...");
    return Ok(());
}
