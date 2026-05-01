use anyhow::bail;
use clockabilly::{DateTime, Utc};
use serde::Serialize;
use sk_api::v1::{SimulationDriverConfig, SimulationHooksConfig, SimulationMetricsConfig, SimulationState};
use sk_core::prelude::*;

#[derive(clap::Args, Debug)]
pub struct Args {
    #[arg(long_help = "name of the simulation to inspect")]
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SimulationInfo {
    name: String,
    state: Option<SimulationState>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    completed_runs: Option<u64>,
    total_runs: Option<i32>,
    speed: f64,
    paused_time: Option<DateTime<Utc>>,
    driver: SimulationDriverConfig,
    metrics: Option<SimulationMetricsConfig>,
    hooks: Option<SimulationHooksConfig>,
}

pub async fn cmd(args: &Args, client: kube::Client) -> EmptyResult {
    let sim_api = kube::Api::<Simulation>::all(client);
    let sim = sim_api.get_opt(&args.name).await?;

    let sim = match sim {
        Some(sim) => sim,
        None => bail!("simulation not found: {}", args.name),
    };

    let status = sim.status.clone().unwrap_or_default();

    let info = SimulationInfo {
        name: sim.metadata.name.clone().unwrap_or_else(|| args.name.clone()),
        state: status.state,
        start_time: status.start_time,
        end_time: status.end_time,
        completed_runs: status.completed_runs,
        total_runs: sim.spec.repetitions,
        speed: sim.speed(),
        paused_time: sim.spec.paused_time,
        driver: sim.spec.driver,
        metrics: sim.spec.metrics,
        hooks: sim.spec.hooks,
    };

    println!("{}", serde_yaml::to_string(&info)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use sk_testutils::*;

    use super::*;

    #[rstest(tokio::test)]
    async fn test_info_cmd_not_found() {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        fake_apiserver.handle_not_found(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));

        let err = cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap_err();

        assert_eq!(format!("simulation not found: {TEST_SIM_NAME}"), format!("{}", err.root_cause()));

        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    async fn test_info_cmd_ok(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();

        fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim);
        });

        cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap();

        fake_apiserver.assert();
    }
}
