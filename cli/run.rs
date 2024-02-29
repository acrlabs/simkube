use simkube::prelude::*;

use crate::args;

pub async fn cmd(args: &args::Run) -> EmptyResult {
    println!("running simulation {}...", args.name);
    let sim = Simulation::new(
        &args.name,
        SimulationSpec {
            driver_namespace: args.driver_namespace.clone(),
            metrics_config: Some(SimulationMetricsConfig {
                namespace: Some(args.metrics_namespace.clone()),
                service_account: Some(args.metrics_service_account.clone()),
                ..Default::default()
            }),
            duration: args.duration.clone(),
            trace_path: args.trace_file.clone(),
        },
    );
    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    sim_api.create(&Default::default(), &sim).await?;

    Ok(())
}
