use simkube::prelude::*;

use crate::args;

pub async fn cmd(args: &args::Delete) -> EmptyResult {
    println!("deleting simulation {}...", args.name);
    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    sim_api.delete(&args.name, &Default::default()).await?;

    Ok(())
}
