#![allow(clippy::needless_return)]

use std::sync::{
    Arc,
    Mutex,
};

use clap::Parser;
use kube::Client;
use simkube::watchertracer::{
    new_watcher_tracer,
    Tracer,
};
use tracing::*;

#[derive(Parser, Debug)]
struct Options {
    #[arg(short, long)]
    server_port: u16,
}

#[rocket::get("/export?<start>&<end>")]
async fn export(start: i64, end: i64, tracer: &rocket::State<Arc<Mutex<Tracer>>>) -> Result<Vec<u8>, String> {
    tracer.lock().unwrap().export(start, end).map_err(|e| format!("{:?}", e))
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args = Options::parse();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let client = Client::try_default().await.expect("failed to create kube client");
    let (mut watcher, tracer) = new_watcher_tracer(client.clone());

    let config = rocket::Config {
        port: args.server_port,
        ..rocket::Config::default()
    };
    let server = rocket::custom(&config).mount("/", rocket::routes![export]).manage(tracer.clone());

    tokio::select!(
        _ = watcher.start() => warn!("watcher finished"),
        _ = server.launch() => warn!("server failed"),
    );

    return Ok(());
}
