use std::fs::File;
use std::io::Write;
use std::sync::{
    Arc,
    Mutex,
};

use clockabilly::prelude::*;
use sk_api::v1::ExportFilters;
use sk_core::prelude::*;
use sk_store::{
    TraceManager,
    TraceStore,
    TracerConfig,
};

#[derive(clap::Args)]
pub struct Args {
    #[arg(short, long, long_help = "config file specifying resources to snapshot")]
    pub config_file: String,

    #[arg(
        long,
        long_help = "namespaces to exclude from the snapshot",
        value_delimiter = ',',
        default_value = "cert-manager,kube-system,local-path-storage,monitoring,simkube"
    )]
    pub excluded_namespaces: Vec<String>,

    #[arg(
        short,
        long,
        long_help = "location to save exported trace",
        default_value = "trace.out"
    )]
    pub output: String,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    println!("Reading config from {}...", args.config_file);
    let config = TracerConfig::load(&args.config_file)?;

    println!("Taking snapshot from Kubernetes cluster...");
    let store = Arc::new(Mutex::new(TraceStore::new(config.clone())));
    let mut manager = TraceManager::new(config, store.clone());
    manager.start().await?;
    manager.wait_ready().await;
    manager.shutdown().await;

    println!("Exporting snapshot data from store...");
    let filters = ExportFilters::new(args.excluded_namespaces.clone(), vec![]);
    let start_ts = UtcClock.now_ts();
    let end_ts = start_ts + 1;
    let data = store.lock().unwrap().export(start_ts, end_ts, &filters)?;

    println!("Writing trace file: {}", args.output);
    let mut file = File::create(&args.output)?;
    file.write_all(&data)?;

    println!("Done!");
    Ok(())
}
