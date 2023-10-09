use std::cmp::max;
use std::fs;
use std::time::Duration;

use anyhow::anyhow;
use clap::Parser;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::{
    DynamicObject,
    Patch,
    PatchParams,
};
use kube::ResourceExt;
use serde_json::json;
use simkube::jsonutils;
use simkube::k8s::{
    add_common_metadata,
    build_global_object_meta,
    prefixed_ns,
    ApiSet,
    GVK,
};
use simkube::macros::*;
use simkube::prelude::*;
use simkube::store::TraceStore;
use tokio::time::sleep;
use tracing::*;

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
    trace_path: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

#[rocket::post("/")]
async fn mutate() {
    info!("got a message! wooo!");
}

async fn run(args: &Options) -> EmptyResult {
    info!("Simulation driver starting");

    let rkt_config = rocket::Config {
        port: args.admission_webhook_port,
        ..Default::default()
    };
    let server = rocket::custom(&rkt_config).mount("/", rocket::routes![mutate]);

    tokio::select! {
        _ = tokio::spawn(server.launch()) => warn!("server terminated"),
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
