mod annotated_trace;
mod rules;
mod summary;
mod validation_store;
mod validator;

use bytes::Bytes;
use clap::{
    Subcommand,
    ValueEnum,
    value_parser,
};
use sk_core::external_storage::{
    ObjectStoreWrapper,
    SkObjectStore,
};
use sk_core::prelude::*;

pub use self::annotated_trace::{
    AnnotatedTrace,
    AnnotatedTraceEvent,
    AnnotatedTracePatch,
    PatchLocations,
};
pub use self::validation_store::VALIDATORS;
pub use self::validator::{
    ValidatorCode,
    ValidatorType,
};

#[derive(Subcommand)]
pub enum ValidateSubcommand {
    #[command(about = "check a trace file")]
    Check(CheckArgs),

    #[command(about = "explain a rule")]
    Explain(ExplainArgs),

    #[command(about = "print all validation rules")]
    Print(PrintArgs),
}

#[derive(clap::Args)]
pub struct CheckArgs {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,

    #[arg(long, long_help = "fix all discovered issues")]
    pub fix: bool,

    #[arg(
        short,
        long,
        long_help = "output path for modified trace (REQUIRED if --fix is set)",
        required_if_eq("fix", "true")
    )]
    pub output: Option<String>,
}

#[derive(clap::Args)]
pub struct ExplainArgs {
    #[arg(long_help = "Error code to explain", value_parser = ValidatorCode::parse)]
    pub code: ValidatorCode,
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
    match subcommand {
        ValidateSubcommand::Check(args) => {
            let mut trace = AnnotatedTrace::new(&args.trace_path).await?;
            let summary = VALIDATORS.validate_trace(&mut trace, args.fix)?;
            if let Some(output_path) = &args.output {
                let trace_data = trace.export()?;
                let object_store = SkObjectStore::new(output_path)?;
                object_store.put(Bytes::from(trace_data)).await?;
            }
            println!("{summary}");
        },
        ValidateSubcommand::Explain(args) => VALIDATORS.explain(&args.code)?,
        ValidateSubcommand::Print(args) => VALIDATORS.print(&args.format)?,
    }
    Ok(())
}

#[cfg(test)]
pub use self::validation_store::ValidationStore;

#[cfg(test)]
pub mod tests;
