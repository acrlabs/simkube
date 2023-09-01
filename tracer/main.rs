#![allow(clippy::needless_return)]

use std::sync::{
    Arc,
    Mutex,
};

use clap::Parser;
use kube::Client;
use rocket::data::ByteUnit;
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

#[rocket::post("/export", data = "<data>")]
async fn export(data: rocket::Data<'_>, tracer: &rocket::State<Arc<Mutex<Tracer>>>) -> Result<Vec<u8>, String> {
    // We don't use rocket::Json because we want to provide default values
    // which that wrapper doesn't seem to support
    let body = match data.open(ByteUnit::kB).into_string().await {
        Ok(b) => b,
        Err(e) => return Err(format!("{:?}", e)),
    };
    let cfg = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => return Err(format!("{:?}", e)),
    };
    info!("exporting with config: {:?}", cfg);
    tracer.lock().unwrap().export(&cfg).map_err(|e| format!("{:?}", e))
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args = Options::parse();

    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let client = Client::try_default().await.expect("failed to create kube client");
    let (mut watcher, tracer) = new_watcher_tracer(client.clone());

    let config = rocket::Config { port: args.server_port, ..Default::default() };
    let server = rocket::custom(&config).mount("/", rocket::routes![export]).manage(tracer.clone());

    tokio::select!(
        _ = watcher.start() => warn!("watcher finished"),
        _ = server.launch() => warn!("server failed"),
    );

    return Ok(());
}
