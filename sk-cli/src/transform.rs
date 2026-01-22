use bytes::Bytes;
use chrono::prelude::*;
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;
use sk_store::ExportedTrace;

use crate::skel::apply_skel_file;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "trace file to transform")]
    pub input: String,

    #[arg(long_help = "SKEL file to apply to the trace")]
    pub skel_file: String,

    #[arg(short, long, long_help = "output (transformed) trace file name")]
    pub output: Option<String>,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    println!("Applying all transformations from {} to {}...", args.skel_file, args.input);
    let object_store = SkObjectStore::new(&args.input)?;
    let trace_data = object_store.get().await?.to_vec();
    let trace = ExportedTrace::import(trace_data, None)?;

    let transformed_trace = apply_skel_file(&trace, &args.skel_file)?;
    let transformed_trace_data = transformed_trace.to_bytes()?;

    let transform_time_str = Local::now().format("%Y%m%d%H%M%S");
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| format!("{}.{}", &args.input, transform_time_str));
    let object_store = SkObjectStore::new(&output_path)?;
    object_store.put(Bytes::from(transformed_trace_data)).await?;

    println!("Transformed trace output written to {output_path}.");
    Ok(())
}
