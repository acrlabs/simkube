mod args;
mod delete;
mod export;
mod run;

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
    #[command(about = "delete a simulation")]
    Delete(args::Delete),

    #[command(about = "export simulation trace data")]
    Export(args::Export),

    #[command(about = "run a simulation")]
    Run(args::Run),
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();

    match &args.subcommand {
        Commands::Export(args) => export::cmd(args).await,
        Commands::Delete(args) => delete::cmd(args).await,
        Commands::Run(args) => run::cmd(args).await,
    }
}
