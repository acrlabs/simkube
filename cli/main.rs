mod crd;
mod delete;
mod export;
mod run;
mod snapshot;

use clap::{
    Parser,
    Subcommand,
};
use simkube::prelude::*;

#[derive(Parser)]
#[command(
    about = "command-line app for running simulations with SimKube",
    version,
    propagate_version = true
)]
struct Options {
    #[command(subcommand)]
    subcommand: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "print SimKube CRDs")]
    Crd,

    #[command(about = "delete a simulation")]
    Delete(delete::Args),

    #[command(about = "export simulation trace data")]
    Export(export::Args),

    #[command(about = "run a simulation")]
    Run(run::Args),

    #[command(about = "take a point-in-time snapshot of a cluster (does not require sk-tracer to be running)")]
    Snapshot(snapshot::Args),
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();

    match &args.subcommand {
        Commands::Crd => crd::cmd(),
        Commands::Export(args) => export::cmd(args).await,
        Commands::Delete(args) => delete::cmd(args).await,
        Commands::Run(args) => run::cmd(args).await,
        Commands::Snapshot(args) => snapshot::cmd(args).await,
    }
}
