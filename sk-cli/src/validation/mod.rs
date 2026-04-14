mod rules;
mod validation_store;
mod validator;

use std::collections::BTreeMap;
use std::io::Write;

use clap::{
    Subcommand,
    ValueEnum,
    value_parser,
};
use sk_core::prelude::*;
use sk_store::ExportedTrace;

pub use self::validation_store::{
    Annotations,
    VALIDATORS,
    ValidationStore,
};
pub use self::validator::{
    ValidatorCode,
    ValidatorType,
};

const WIDTH: usize = 70;

#[derive(Clone, Subcommand)]
pub enum ValidateSubcommand {
    #[command(about = "explain a rule")]
    Check(CheckArgs),

    #[command(about = "explain a rule")]
    Explain(ExplainArgs),

    #[command(about = "print all validation rules")]
    Print(PrintArgs),
}

#[derive(clap::Args, Clone)]
pub struct CheckArgs {
    #[arg(long_help = "location of the input trace file")]
    pub trace_path: String,

    #[arg(long, long_help = "print sample SKEL code to fix the trace", default_value = "false")]
    generate_skel: bool,
}

#[derive(clap::Args, Clone)]
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

#[derive(clap::Args, Clone)]
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
            let trace = ExportedTrace::from_path(&args.trace_path).await?;
            let mut validators = VALIDATORS.lock().unwrap();
            let failed_checks = validators.validate_trace(&trace)?;
            write_summary(&mut std::io::stdout(), &args.trace_path, &validators, failed_checks, args.generate_skel)?;
        },
        ValidateSubcommand::Explain(args) => VALIDATORS.lock().unwrap().explain(&args.code)?,
        ValidateSubcommand::Print(args) => VALIDATORS.lock().unwrap().print(&args.format)?,
    }
    Ok(())
}

pub(super) fn write_summary(
    f: &mut impl Write,
    trace_path: &str,
    validators: &ValidationStore,
    failed_checks: BTreeMap<usize, Annotations>,
    generate_skel: bool,
) -> EmptyResult {
    let mut summary: BTreeMap<ValidatorCode, usize> = BTreeMap::new();
    for (_, annotations) in failed_checks {
        for (code, obj_indices) in annotations {
            *summary.entry(code).or_default() += obj_indices.len()
        }
    }

    let mut skel = String::new();

    writeln!(f, "Validation errors found:")?;
    writeln!(f, "{}", "-".repeat(WIDTH))?;
    for (code, count) in summary.iter() {
        if *count == 0 {
            continue;
        }
        let v = validators.lookup(code);
        let (name, skel_suggestion) = v.map(|v| (v.name, v.skel_suggestion)).unwrap_or(("<unknown>", ""));
        let left = format!("{name} ({code})");
        let right = format!("{count}");
        let mid_width = WIDTH.saturating_sub(left.len()).saturating_sub(right.len()).saturating_sub(2); // two chars for extra spaces
        writeln!(f, "{left} {} {right}", ".".repeat(mid_width))?;

        skel += skel_suggestion;
    }

    if generate_skel {
        writeln!(f, "{}", "-".repeat(WIDTH))?;
        writeln!(f, "# Auto-generated SKEL file to fix this trace;")?;
        writeln!(f, "# Run `skctl transform {trace_path} <SKELFILE>` to apply it\n")?;
        writeln!(f, "{skel}")?;
    }

    Ok(())
}

#[cfg(test)]
pub mod tests;
