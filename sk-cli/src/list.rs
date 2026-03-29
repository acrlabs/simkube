use sk_core::prelude::*;

#[derive(clap::Args, Debug, Default)]
pub struct Args {}

fn fmt_dt(ts: Option<clockabilly::DateTime<clockabilly::Utc>>) -> String {
    ts.map(|t| t.to_string()).unwrap_or_else(|| "-".into())
}

fn fmt_state(sim: &Simulation) -> String {
    sim.status
        .as_ref()
        .and_then(|s| s.state.as_ref())
        .map(|s| format!("{s:?}").to_lowercase())
        .unwrap_or_else(|| "unknown".into())
}

fn fmt_completed(sim: &Simulation) -> String {
    let completed = sim.status.as_ref().and_then(|s| s.completed_runs).unwrap_or(0);
    let total = sim.spec.repetitions.unwrap_or(1);
    format!("{completed}/{total}")
}

pub async fn cmd(_: &Args, client: kube::Client) -> EmptyResult {
    let sim_api = kube::Api::<Simulation>::all(client);
    let mut sims = sim_api.list(&Default::default()).await?.items;

    if sims.is_empty() {
        println!("no simulations found");
        return Ok(());
    }

    sims.sort_by(|a, b| {
        a.metadata
            .name
            .as_deref()
            .unwrap_or("")
            .cmp(b.metadata.name.as_deref().unwrap_or(""))
    });

    println!("{:<32} {:<12} {:<24} {:<24} {:<12} {:<8}", "NAME", "STATE", "START", "END", "COMPLETED", "SPEED");

    for sim in sims {
        let name = sim.metadata.name.as_deref().unwrap_or("-");
        let start = fmt_dt(sim.status.as_ref().and_then(|s| s.start_time));
        let end = fmt_dt(sim.status.as_ref().and_then(|s| s.end_time));
        let completed = fmt_completed(&sim);
        let speed = sim.speed();

        println!("{:<32} {:<12} {:<24} {:<24} {:<12} {:<8}", name, fmt_state(&sim), start, end, completed, speed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sk_testutils::*;

    use super::*;

    #[rstest(tokio::test)]
    async fn test_list_cmd_ok(test_sim: Simulation) {
        let (mut fake_apiserver, client) = make_fake_apiserver();

        fake_apiserver.handle(move |when, then| {
            when.method(GET).path("/apis/simkube.io/v1/simulations");
            then.json_body_obj(&json!({
                "kind": "SimulationList",
                "apiVersion": "simkube.io/v1",
                "metadata": {},
                "items": [test_sim],
            }));
        });

        cmd(&Args::default(), client).await.unwrap();
        fake_apiserver.assert();
    }
}
