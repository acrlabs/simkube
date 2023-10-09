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

#[derive(Error, Debug)]
#[error(transparent)]
enum ReconcileError {
    AnyhowError(#[from] anyhow::Error),
    KubeApiError(#[from] kube::Error),
}

struct SimulationContext {
    k8s_client: kube::Client,
    opts: Options,
    sim_svc_account: String,
}

async fn run(args: Options) -> EmptyResult {
    info!("Simulation controller starting");

    let k8s_client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(k8s_client.clone());

    let ctrl = Controller::new(sim_api, Default::default())
        .run(
            reconcile,
            error_policy,
            Arc::new(SimulationContext {
                k8s_client,
                opts: args,

                // TODO don't hardcode these
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
    run(args).await?;
    Ok(())
}
