mod status_field_populated;
mod trace;
mod validation_store;

use clap::{
    value_parser,
    Subcommand,
    ValueEnum,
};
use sk_core::prelude::*;

pub use self::trace::AnnotatedTrace;
pub use self::validation_store::ValidationStore;

#[derive(Subcommand)]
pub enum ValidateSubcommand {
    #[command(about = "check a trace file")]
    Check(CheckArgs),

    #[command(about = "print all validation rules")]
    Print(PrintArgs),
}

#[derive(clap::Args)]
pub struct CheckArgs {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,
}

#[derive(Clone, ValueEnum)]
pub enum PrintFormat {
    Json,
    List,
    Table,
    Yaml,
}

#[derive(clap::Args)]
pub struct PrintArgs {
    #[arg(
        short,
        long,
        long_help = "format to display the validation rules",
        default_value = "list",
        value_parser = value_parser!(PrintFormat),
    )]
    pub format: PrintFormat,
}

pub async fn cmd(subcommand: &ValidateSubcommand) -> EmptyResult {
    let mut store = ValidationStore::default();
    match subcommand {
        ValidateSubcommand::Check(args) => {
            let mut trace = AnnotatedTrace::new(&args.trace_path).await?;
            store.validate_trace(&mut trace);
            for evt in trace.events.iter() {
                println!("{:?}", evt.annotations);
            }
        },
        ValidateSubcommand::Print(args) => {
            store.print(&args.format)?;
        },
    }
    Ok(())
}
