#![cfg_attr(coverage, feature(coverage_attribute))]
mod completions;
mod crd;
mod delete;
mod export;
mod pauseresume;
mod run;
mod snapshot;
mod validation;
mod xray;

use clap::{
    crate_version,
    CommandFactory,
    Parser,
    Subcommand,
};
use sk_core::logging;
use sk_core::prelude::*;

use crate::validation::ValidateSubcommand;

#[derive(Parser)]
#[command(
    about = "command-line app for running simulations with SimKube",
    version,
    propagate_version = true
)]
struct SkCommandRoot {
    #[command(subcommand)]
    subcommand: SkSubcommand,

    #[arg(short, long, default_value = "warn")]
    verbosity: String,
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

    #[command(about = "pause a running simulation")]
    Pause(pauseresume::Args),

    #[command(about = "resume a paused simulation")]
    Resume(pauseresume::Args),

    #[command(about = "run a simulation")]
    Run(run::Args),

    #[command(about = "take a point-in-time snapshot of a cluster (does not require sk-tracer to be running)")]
    Snapshot(snapshot::Args),

    #[command(subcommand)]
    Validate(ValidateSubcommand),

    #[command(about = "simkube version")]
    Version,

    #[command(about = "explore or prepare trace data for simulation")]
    Xray(xray::Args),
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = SkCommandRoot::parse();
    logging::setup_for_cli(&args.verbosity);

    // Not every subcommand needs a kube client and might actually fail (in CI or whatever)
    // if it can't find a kubeconfig, so that's why we don't construct the client outside
    // of the match; also it may be a teensy-eensy bit more performant but honestly probably
    // not noticeable.
    match &args.subcommand {
        SkSubcommand::Completions(args) => completions::cmd(args, SkCommandRoot::command()),
        SkSubcommand::Crd => crd::cmd(),
        SkSubcommand::Export(args) => export::cmd(args).await,
        SkSubcommand::Delete(args) => {
            let client = kube::Client::try_default().await?;
            delete::cmd(args, client).await
        },
        SkSubcommand::Pause(args) => {
            let client = kube::Client::try_default().await?;
            pauseresume::pause_cmd(args, client).await
        },
        SkSubcommand::Resume(args) => {
            let client = kube::Client::try_default().await?;
            pauseresume::resume_cmd(args, client).await
        },
        SkSubcommand::Run(args) => {
            let client = kube::Client::try_default().await?;
            run::cmd(args, client).await
        },
        SkSubcommand::Snapshot(args) => {
            let client = kube::Client::try_default().await?;
            snapshot::cmd(args, client).await
        },
        SkSubcommand::Validate(subcommand) => validation::cmd(subcommand).await,
        SkSubcommand::Version => {
            println!("skctl {}", crate_version!());
            Ok(())
        },
        SkSubcommand::Xray(args) => xray::cmd(args).await,
    }
}
