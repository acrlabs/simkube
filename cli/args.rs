use clap::Args;
use reqwest::Url;
use simkube::prelude::*;
use simkube::time;

#[derive(Args)]
pub struct Delete {
    #[arg(long_help = "name of the simulation to run", long)]
    pub name: String,
}

#[derive(Args)]
pub struct Export {
    #[arg(
        long_help = "trace export start timestamp; can be a relative duration\n\t\
                         or absolute timestamp; durations are computed relative\n\t\
                         to the specified end time, _not_ the current time",
        long,
        default_value = "-30m",
        value_parser = time::parse,
        allow_hyphen_values = true,
    )]
    pub start_time: i64,

    #[arg(
        long_help = "end time; can be a relative or absolute timestamp",
        long,
        default_value = "now",
        value_parser = time::parse,
        allow_hyphen_values = true,
    )]
    pub end_time: i64,

    #[arg(
        long_help = "namespaces to exclude from the trace",
        long,
        value_delimiter = ',',
        default_value = "cert-manager,kube-system,local-path-storage,monitoring,simkube"
    )]
    pub excluded_namespaces: Vec<String>,

    #[arg(
        long_help = "sk-tracer server address",
        long,
        default_value = "http://localhost:7777"
    )]
    pub tracer_address: String,

    #[arg(
        long_help = "location to save exported trace",
        long,
        default_value = "file:///tmp/kind-node-data"
    )]
    pub output: Url,
}

#[derive(Args)]
pub struct Run {
    #[arg(long_help = "name of the simulation to run", long)]
    pub name: String,

    #[arg(long_help = "namespace to launch sk-driver in", long, default_value = "simkube")]
    pub driver_namespace: String,

    #[arg(
        long_help = "namespaced name of configmap containing PromQL queries to execute at the end of the simulation",
        long,
        default_value = "simkube/queries"
    )]
    pub metric_query_configmap: String,

    #[arg(
        long_help = "namespace to launch monitoring utilities in",
        long,
        default_value = DEFAULT_MONITORING_NS,
    )]
    pub monitoring_namespace: String,

    #[arg(
        long_help = "service account with monitoring permissions",
        long,
        default_value = DEFAULT_PROM_SVC_ACCOUNT,
    )]
    pub prometheus_service_account: String,

    #[arg(
        long_help = "location of the trace file for sk-driver to read",
        long,
        default_value = "file:///data/trace"
    )]
    pub trace_file: String,
}
