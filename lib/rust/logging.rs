use std::str::FromStr;

use tracing::*;

use crate::errors::*;

pub fn setup(verbosity: &str) -> EmptyResult {
    let level = Level::from_str(verbosity)?;
    tracing_subscriber::fmt().with_max_level(level).init();
    Ok(())
}
