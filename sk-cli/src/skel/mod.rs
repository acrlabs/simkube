pub(super) mod ast;

use std::fs;

use pest::Parser;
use pest_derive::Parser;
use sk_core::prelude::*;

#[allow(dead_code)]
#[derive(Parser)]
#[grammar = "src/skel/skel.pest"]
struct SkelParser;

pub fn parse_skel_file(filename: &str) -> EmptyResult {
    let commands = fs::read_to_string(filename)?;
    let skel = SkelParser::parse(Rule::skel, &commands)?;

    for command in skel {
        println!("{command:?}");
    }

    Ok(())
}

#[cfg(test)]
pub mod tests;
