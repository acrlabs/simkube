#![allow(clippy::needless_return)]
use std::fs;

use clap::Parser;
use simkube::watchertracer::Tracer;

#[derive(Parser, Debug)]
struct Options {
    #[arg(short, long)]
    trace_path: String,
}

fn main() {
    let args = Options::parse();
    let trace_data = fs::read(args.trace_path).expect("cannot read trace file");
    let tracer = Tracer::import(trace_data).expect("could not import trace");

    for (evt, ts) in tracer.iter() {
        println!("{:?}, {:?}", evt, ts);
    }
}
