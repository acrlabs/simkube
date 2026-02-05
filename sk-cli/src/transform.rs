use std::time::Duration;

use bytes::Bytes;
use clockabilly::Local;
use clockabilly::prelude::*;
use humantime::format_duration;
use metrics::{
    Key,
    gauge,
};
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;
use sk_store::ExportedTrace;

use crate::skel::apply_skel_file;
use crate::skel::metrics::*;

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
    println!("\nApplying all transformations from {} to {}...", args.skel_file, args.input);
    let object_store = SkObjectStore::new(&args.input)?;
    let trace_data = object_store.get().await?.to_vec();
    let trace = ExportedTrace::import(trace_data, None)?;

    let clock = UtcClock::new();
    let start_time = clock.now();
    let transformed_trace = apply_skel_file(&trace, &args.skel_file)?;
    let transformed_trace_data = transformed_trace.to_bytes()?;
    let end_time = clock.now();

    let eval_time_gauge = gauge!(TOTAL_EVALUATION_TIME_GAUGE);
    eval_time_gauge.set((end_time.timestamp_millis() - start_time.timestamp_millis()) as f64);

    let transform_time_str = start_time.with_timezone(&Local).format("%Y%m%d%H%M%S");
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| format!("{}.{}", &args.input, transform_time_str));
    let object_store = SkObjectStore::new(&output_path)?;
    object_store.put(Bytes::from(transformed_trace_data)).await?;

    println!("Transformed trace output written to {output_path}.\n");
    Ok(())
}

pub fn output_stats(metrics: &MemoryRecorder) -> EmptyResult {
    let duration = Duration::from_millis(metrics.get_gauge(&Key::from_name(TOTAL_EVALUATION_TIME_GAUGE))? as u64);

    println!("Summary of changes:");
    println!("{}", "-".repeat(80));
    println!("  Trace events matched: {}", metrics.get_counter(&Key::from_name(EVENT_MATCHED_COUNTER))?);
    println!("  Trace resources modified: {}", metrics.get_counter(&Key::from_name(RESOURCE_MODIFIED_COUNTER))?);
    println!("  Total evaluation time: {}", format_duration(duration));
    println!("{}", "-".repeat(80));
    Ok(())
}
