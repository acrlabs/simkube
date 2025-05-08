use sk_core::prelude::*;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long_help = "name of the simulation to delete")]
    pub name: String,
}

pub async fn cmd(args: &Args, client: kube::Client) -> EmptyResult {
    println!("deleting simulation {}...", args.name);
    let sim_api = kube::Api::<Simulation>::all(client.clone());

    sim_api.delete(&args.name, &Default::default()).await?;

    Ok(())
}
