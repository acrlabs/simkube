use std::fs;

use anyhow::anyhow;
use bytes::Bytes;
use reqwest::Url;
use simkube::prelude::*;
use simkube::store::storage::{
    get_scheme,
    Scheme,
};

use crate::args;

pub async fn cmd(args: &args::Export) -> EmptyResult {
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
