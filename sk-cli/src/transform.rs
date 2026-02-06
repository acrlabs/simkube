use std::sync::mpsc;
use std::time::Duration;

use bytes::Bytes;
use clockabilly::Local;
use clockabilly::prelude::*;
use humantime::format_duration;
use kdam::{
    Animation,
    BarExt,
    Colour,
    Spinner,
    tqdm,
};
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

const SPINNER_REFRESH_RATE: u64 = 50;
const SPINNER_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

    let progress_spinner = Spinner::new(SPINNER_DOTS, SPINNER_REFRESH_RATE as f32, 1.0);
    let mut progress_bar = tqdm!(
        total = trace.len(),
        animation = Animation::Tqdm,
        bar_format = format!("  {{spinner}} {{animation}} {{count}}/{{total}}"),
        colour = Colour::gradient(&["#5A56E0", "#EE6FF8"]),
        ncols = 80,
        spinner = progress_spinner
    );

    let (tx, rx) = mpsc::channel();

    let clock = UtcClock::new();
    let start_time = clock.now();
    let skel_filename = args.skel_file.clone();
    let transform_task = tokio::spawn(async move { apply_skel_file(&trace, &skel_filename, tx).await });
    loop {
        // don't really care about errors on progress bar updates right now
        let _ = progress_bar.refresh();
        tokio::time::sleep(Duration::from_millis(SPINNER_REFRESH_RATE)).await;
        match rx.try_recv() {
            Ok(_) => {
                let _ = progress_bar.update(1);
            },
            Err(mpsc::TryRecvError::Disconnected) => break,
            _ => (),
        }
    }
    let end_time = clock.now();

    let transformed_trace_data = match transform_task.await? {
        Ok(data) => data.to_bytes()?,
        Err(err) => {
            let _ = progress_bar.clear();
            return Err(err);
        },
    };
    let _ = progress_bar.set_bar_format(" ✅ {animation} {count}/{total}");
    let _ = progress_bar.refresh();

    println!("\n");

    let eval_time_gauge = gauge!(TOTAL_EVALUATION_TIME_GAUGE);
    eval_time_gauge.set((end_time.timestamp_millis() - start_time.timestamp_millis()) as f64);

    let transform_time_str = start_time.with_timezone(&Local).format("%Y%m%d%H%M%S");
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| format!("{}.{}", &args.input, transform_time_str));
    let object_store = SkObjectStore::new(&output_path)?;
    object_store.put(Bytes::from(transformed_trace_data)).await?;

    println!("All done!  Transformed trace written to {output_path}.\n");
    Ok(())
}

pub fn output_stats(metrics: &MemoryRecorder) -> EmptyResult {
    let duration = Duration::from_millis(metrics.get_gauge(&Key::from_name(TOTAL_EVALUATION_TIME_GAUGE))? as u64);

    println!("Summary of changes:");
    println!("{}", "-".repeat(80));
    println!("  Trace events matched: {}", metrics.get_counter(&Key::from_name(EVENT_MATCHED_COUNTER))?);
    println!("  Trace resources modified: {}", metrics.get_counter(&Key::from_name(RESOURCE_MODIFIED_COUNTER))?);
    println!("  Total evaluation time: {}", format_duration(duration));
    println!("{}\n", "-".repeat(80));
    Ok(())
}
