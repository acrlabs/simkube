mod completions;
mod crd;
mod delete;
mod export;
mod run;
mod snapshot;

use clap::{
    CommandFactory,
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
struct SkCommandRoot {
    #[command(subcommand)]
    subcommand: SkSubcommand,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum SkSubcommand {
    #[command(about = "generate shell completions for skctl")]
    Completions(completions::Args),

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
    let args = SkCommandRoot::parse();

    match &args.subcommand {
        SkSubcommand::Completions(args) => completions::cmd(args, SkCommandRoot::command()),
        SkSubcommand::Crd => crd::cmd(),
        SkSubcommand::Export(args) => export::cmd(args).await,
        SkSubcommand::Delete(args) => delete::cmd(args).await,
        SkSubcommand::Run(args) => run::cmd(args).await,
        SkSubcommand::Snapshot(args) => snapshot::cmd(args).await,
    }
}

#[cfg(test)]
mod tests;
