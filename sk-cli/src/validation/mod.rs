mod annotated_trace;
mod status_field_populated;
mod validation_store;
mod validator;

use clap::{
    value_parser,
    Subcommand,
    ValueEnum,
};
use sk_core::prelude::*;

pub use self::annotated_trace::AnnotatedTrace;
pub use self::validation_store::ValidationStore;

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
}

#[derive(clap::Args)]
pub struct ExplainArgs {
    #[arg(long_help = "Error code to explain")]
    pub code: String,
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
    let mut validators = ValidationStore::default();
    match subcommand {
        ValidateSubcommand::Check(args) => {
            let mut trace = AnnotatedTrace::new(&args.trace_path).await?;
            validators.validate_trace(&mut trace);
            print_summary(&trace, &validators)?;
        },
        ValidateSubcommand::Explain(args) => validators.explain(&args.code)?,
        ValidateSubcommand::Print(args) => validators.print(&args.format)?,
    }
    Ok(())
}

fn print_summary(trace: &AnnotatedTrace, validators: &ValidationStore) -> EmptyResult {
    for (code, count) in trace.summary_iter() {
        let name = validators.lookup(code)?.name;
        println!("{name} ({code}): {count:.>30}");
    }
    Ok(())
}

#[cfg(test)]
pub mod tests;
