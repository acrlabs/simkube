use std::fs;

use anyhow::anyhow;
use bytes::Bytes;
use reqwest::Url;
use simkube::prelude::*;
use simkube::store::storage::{
    get_scheme,
    Scheme,
};
use simkube::time::duration_to_ts;

#[derive(clap::Args)]
pub struct Args {
    #[arg(
        short,
        long,
        long_help = "trace export start timestamp; can be a relative duration\n\
             or absolute timestamp; durations are computed relative\n\
             to the specified end time, _not_ the current time",
        default_value = "-30m",
        value_parser = duration_to_ts,
        allow_hyphen_values = true,
    )]
    pub start_time: i64,

    #[arg(
        short = 't',
        long,
        long_help = "end time; can be a relative or absolute timestamp",
        default_value = "now",
        value_parser = duration_to_ts,
        allow_hyphen_values = true,
    )]
    pub end_time: i64,

    #[arg(
        long,
        long_help = "namespaces to exclude from the trace",
        value_delimiter = ',',
        default_value = "cert-manager,kube-system,local-path-storage,monitoring,simkube"
    )]
    pub excluded_namespaces: Vec<String>,

    #[arg(
        long,
        long_help = "sk-tracer server address",
        default_value = "http://localhost:7777"
    )]
    pub tracer_address: String,

    #[arg(
        short,
        long,
        long_help = "location to save exported trace",
        default_value = "file:///tmp/kind-node-data"
    )]
    pub output: Url,
}

pub async fn cmd(args: &Args) -> EmptyResult {
    let filters = ExportFilters::new(args.excluded_namespaces.clone(), vec![], true);
    let req = ExportRequest::new(args.start_time, args.end_time, filters);
    let endpoint = format!("{}/export", args.tracer_address);

    println!("exporting trace data");
    println!("start_ts = {}, end_ts = {}", args.start_time, args.end_time);
    println!("using filters:\n\texcluded_namespaces: {:?}\n\texcluded_labels: none", args.excluded_namespaces);
    println!("making request to {}", endpoint);

    let client = reqwest::Client::new();
    let res = client.post(endpoint).json(&req).send().await?;

    write_output(&res.bytes().await?, &args.output)
}

fn write_output(data: &Bytes, output_url: &Url) -> EmptyResult {
    match get_scheme(output_url)? {
        Scheme::Local => {
            let mut fp = output_url
                .to_file_path()
                .map_err(|_| anyhow!("could not compute export path: {}", output_url))?;
            fs::create_dir_all(&fp)?;
            fp.push("trace");
            fs::write(&fp, data)?;
            println!("trace successfully written to {}", fp.to_str().unwrap());
        },
        Scheme::AmazonS3 => unimplemented!(),
    }


    Ok(())
}
