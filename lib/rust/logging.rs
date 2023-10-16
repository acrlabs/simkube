use std::str::FromStr;

use tracing::*;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::errors::*;

pub fn setup(verbosity: &str) -> EmptyResult {
    let level = Level::from_str(verbosity)?;
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_target(false)
        .compact()
        .init();
    Ok(())
}
