use anyhow::bail;
use clockabilly::prelude::*;
use kube::api::Patch;
use serde_json::json;
use sk_api::v1::SimulationSpec;
use sk_core::prelude::*;

const PAUSED_TIME_KEY: &str = "pausedTime";

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "name of the simulation to operate on")]
    pub name: String,
}

pub async fn pause_cmd(args: &Args, client: kube::Client) -> EmptyResult {
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    let maybe_sim = sim_api.get_opt(&args.name).await?;
    match maybe_sim {
        None => bail!("simulation not found: {}", args.name),
        Some(Simulation {
            spec: SimulationSpec { paused_time: Some(ts), .. }, ..
        }) => bail!("simulation {} is already paused at {}", &args.name, ts),
        _ => (),
    }

    println!("pausing simulation {}...", args.name);
    let now = UtcClock.now();
    let pause_patch = json!({
        "spec": {
            PAUSED_TIME_KEY: now,
    }});

    sim_api
        .patch(&args.name, &Default::default(), &Patch::Merge(pause_patch))
        .await?;

    Ok(())
}

pub async fn resume_cmd(args: &Args, client: kube::Client) -> EmptyResult {
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    let maybe_sim = sim_api.get_opt(&args.name).await?;
    match maybe_sim {
        None => bail!("simulation not found: {}", args.name),
        Some(Simulation { spec: SimulationSpec { paused_time: None, .. }, .. }) => {
            bail!("simulation {} is not paused", &args.name)
        },
        _ => (),
    }

    println!("resuming simulation {}...", args.name);
    let resume_patch = json!({
        "spec": {
            PAUSED_TIME_KEY: null,
    }});

    sim_api
        .patch(&args.name, &Default::default(), &Patch::Merge(resume_patch))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use clockabilly::DateTime;
    use httpmock::prelude::*;
    use sk_testutils::*;

    use super::*;

    #[rstest(tokio::test)]
    async fn test_pause_cmd_not_found() {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        fake_apiserver.handle_not_found(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));

        let err = pause_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap_err();

        assert_eq!(format!("simulation not found: {TEST_SIM_NAME}"), format!("{}", err.root_cause()));
        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    async fn test_pause_cmd_already_paused(mut test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        test_sim.spec.paused_time = DateTime::from_timestamp(0, 0);
        fake_apiserver.handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim);
        });

        let err = pause_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap_err();

        assert_starts_with!(format!("{}", err.root_cause()), format!("simulation {TEST_SIM_NAME} is already paused"));
        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    async fn test_pause_cmd_not_paused(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let mut test_sim_patched = test_sim.clone();
        test_sim_patched.spec.paused_time = DateTime::from_timestamp(0, 0);
        fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim);
        });
        fake_apiserver.handle(move |when, then| {
            when.method(PATCH)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"))
                .body_matches("\\{\"spec\":\\{\"pausedTime\":\".*\"\\}\\}");
            then.json_body_obj(&test_sim_patched);
        });

        pause_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap();
        fake_apiserver.assert();
    }


    #[rstest(tokio::test)]
    async fn test_resume_cmd_not_found() {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        fake_apiserver.handle_not_found(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));

        let err = resume_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap_err();

        assert_eq!(format!("simulation not found: {TEST_SIM_NAME}"), format!("{}", err.root_cause()));
        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    async fn test_resume_cmd_not_paused(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        fake_apiserver.handle(move |when, then| {
            when.path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim);
        });

        let err = resume_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap_err();

        assert_eq!(format!("simulation {TEST_SIM_NAME} is not paused"), format!("{}", err.root_cause()));
        fake_apiserver.assert();
    }

    #[rstest(tokio::test)]
    async fn test_resume_cmd_paused(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();
        let mut test_sim_patched = test_sim.clone();
        test_sim_patched.spec.paused_time = DateTime::from_timestamp(0, 0);
        fake_apiserver.handle(move |when, then| {
            when.method(GET)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"));
            then.json_body_obj(&test_sim_patched);
        });
        fake_apiserver.handle(move |when, then| {
            when.method(PATCH)
                .path(format!("/apis/simkube.io/v1/simulations/{TEST_SIM_NAME}"))
                .body_matches("\\{\"spec\":\\{\"pausedTime\":null\\}\\}");
            then.json_body_obj(&test_sim);
        });

        resume_cmd(&Args { name: TEST_SIM_NAME.into() }, client).await.unwrap();
        fake_apiserver.assert();
    }
}
