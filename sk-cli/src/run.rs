use clap::crate_version;
use clockabilly::prelude::*;
use serde::Serialize;
use sk_api::prometheus::PrometheusRemoteWrite;
use sk_api::v1::{
    SimulationDriverConfig,
    SimulationMetricsConfig,
    SimulationSpec,
};
use sk_core::hooks::merge_hooks;
use sk_core::prelude::*;

const DRIVER_IMAGE: &str = "quay.io/appliedcomputing/sk-driver";

#[derive(clap::Args, Debug, Serialize)]
#[command(disable_help_flag = true, disable_version_flag = true)]
pub struct Args {
    #[arg(long_help = "name of the simulation to run")]
    pub name: String,

    #[arg(
        short = 'D',
        long,
        long_help = "duration of the simulation",
        allow_hyphen_values = true
    )]
    pub duration: Option<String>,

    #[arg(
        long = "speed",
        short = 's',
        long_help = "Time multiplier for the simulation",
        default_value = "1"
    )]
    pub speed: f64,

    #[arg(
        short = 'N',
        long,
        long_help = "number of repetitions of the simulation to run",
        default_value = "1"
    )]
    pub repetitions: i32,

    #[arg(long, long_help = "start the simulation in the \"paused\" state")]
    pub start_paused: bool,

    #[arg(
        long,
        long_help = "list of simulation hook files to apply in order",
        default_value = "config/hooks/default.yml:config/hooks/autoscaler.yml",
        value_delimiter = ':'
    )]
    pub hooks: Option<Vec<String>>,

    #[arg(
        short = 'I',
        long,
        long_help = "name of the docker image to use for sk-driver",
        default_value = format!("{DRIVER_IMAGE}:v{}", crate_version!()),
        help_heading = "Driver"
    )]
    pub driver_image: String,

    #[arg(
        long,
        long_help = "admission webhook port for sk-driver",
        default_value = DRIVER_ADMISSION_WEBHOOK_PORT,
        help_heading = "Driver"
    )]
    pub driver_port: i32,

    #[arg(
        short = 'f',
        long,
        long_help = "location of the trace file for sk-driver to read",
        default_value = "file:///data/trace",
        help_heading = "Driver"
    )]
    pub trace_path: String,

    #[arg(
        long,
        long_help = "namespace to launch sk-driver in",
        default_value = "simkube",
        help_heading = "Driver"
    )]
    pub driver_namespace: String,

    #[arg(
        long,
        long_help = "additional secrets to mount in the driver pod",
        value_delimiter = ',',
        help_heading = "Driver"
    )]
    pub driver_secrets: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "don't mount trace volume to the driver pod",
        help_heading = "Driver"
    )]
    pub skip_local_volume_mount: bool,

    #[arg(
        long,
        long_help = "driver log verbosity",
        default_value = "info",
        help_heading = "Driver"
    )]
    pub driver_verbosity: String,

    #[arg(
        long,
        long_help = "namespace prefix for simulation objects",
        default_value = "virtual",
        help_heading = "Driver"
    )]
    pub virtual_ns_prefix: String,

    #[arg(
        long,
        long_help = "don't spawn Prometheus pod before running sim",
        help_heading = "Metrics"
    )]
    pub disable_metrics: bool,

    #[arg(
        long,
        long_help = "namespace to launch monitoring utilities in",
        default_value = DEFAULT_METRICS_NS,
        help_heading = "Metrics",
    )]
    pub metrics_namespace: String,

    #[arg(
        long,
        long_help = "service account with monitoring permissions",
        default_value = DEFAULT_METRICS_SVC_ACCOUNT,
        help_heading = "Metrics",
    )]
    pub metrics_service_account: String,

    #[arg(
        long,
        long_help = "comma-separated list of namespaces containing pod monitor configs",
        value_delimiter = ',',
        default_value = "monitoring-hd",
        help_heading = "Metrics"
    )]
    pub metrics_pod_monitor_namespaces: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of pod monitor config names\n\
            (if empty, uses all pod monitor configs in metrics_pod_monitor_namespaces)",
        value_delimiter = ',',
        help_heading = "Metrics"
    )]
    pub metrics_pod_monitor_names: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of namespaces containing service monitor configs",
        value_delimiter = ',',
        default_value = "monitoring-hd",
        help_heading = "Metrics"
    )]
    pub metrics_service_monitor_namespaces: Option<Vec<String>>,

    #[arg(
        long,
        long_help = "comma-separated list of service monitor config names\n\
            (if empty, uses all pod monitor configs in metrics_service_monitor_namespaces)",
        value_delimiter = ',',
        help_heading = "Metrics"
    )]
    pub metrics_service_monitor_names: Option<Vec<String>>,

    #[arg(long, long_help = "number of prometheus shards to run", help_heading = "Metrics")]
    pub prometheus_shards: Option<i32>,

    #[arg(
        long,
        long_help = "address for remote write endpoint",
        default_value = "http://prom2parquet-svc:1234/receive",
        help_heading = "Metrics"
    )]
    pub remote_write_endpoint: Option<String>,

    // We override help and version here so that it shows up in its own help group at the bottom
    // See https://github.com/clap-rs/clap/issues/4367 and https://github.com/clap-rs/clap/issues/4831
    // for more details.
    #[serde(skip)]
    #[arg(short, long, long_help="Print help (see a summary with '-h')", action = clap::ArgAction::Help, help_heading = "Help")]
    pub help: (),

    #[serde(skip)]
    #[arg(short='V', long, long_help="Print version", action = clap::ArgAction::Version, help_heading = "Help")]
    pub version: (),
}

pub async fn cmd(args: &Args, client: kube::Client) -> EmptyResult {
    println!("running simulation with configuration:\n\n---\n{}", serde_yaml::to_string(args)?);

    let metrics_config = (!args.disable_metrics).then_some(SimulationMetricsConfig {
        namespace: Some(args.metrics_namespace.clone()),
        service_account: Some(args.metrics_service_account.clone()),
        pod_monitor_namespaces: args.metrics_pod_monitor_namespaces.clone(),
        pod_monitor_names: args.metrics_pod_monitor_names.clone(),
        service_monitor_namespaces: args.metrics_service_monitor_namespaces.clone(),
        service_monitor_names: args.metrics_service_monitor_names.clone(),
        prometheus_shards: args.prometheus_shards,
        remote_write_configs: args
            .remote_write_endpoint
            .clone()
            .map_or(vec![], |url| vec![PrometheusRemoteWrite { url, ..Default::default() }]),
    });

    let hooks = merge_hooks(&args.hooks)?;
    let driver_args = vec!["--verbosity".into(), args.driver_verbosity.clone()];

    let paused_time = if args.start_paused { Some(UtcClock.now()) } else { None };
    let mut sim = Simulation::new(
        &args.name,
        SimulationSpec {
            driver: SimulationDriverConfig {
                args: Some(driver_args),
                image: args.driver_image.clone(),
                namespace: args.driver_namespace.clone(),
                port: args.driver_port,
                secrets: args.driver_secrets.clone(),
                trace_path: args.trace_path.clone(),
                virtual_ns_prefix: args.virtual_ns_prefix.clone(),
            },
            duration: args.duration.clone(),
            hooks,
            metrics: metrics_config,
            paused_time,
            repetitions: Some(args.repetitions),
            speed: Some(args.speed),
        },
    );
    if args.skip_local_volume_mount {
        sim.annotations_mut()
            .insert(SKIP_LOCAL_VOLUME_MOUNT_ANNOTATION_KEY.into(), "true".into());
    }

    let sim_api = kube::Api::<Simulation>::all(client.clone());
    sim_api.create(&Default::default(), &sim).await?;

    Ok(())
}
