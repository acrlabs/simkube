use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't

use anyhow::anyhow;
use serde::Serialize;
use sk_core::prelude::*;

use super::annotated_trace::AnnotatedTrace;
use super::validator::{
    Validator,
    ValidatorCode,
};
use super::{
    status_field_populated,
    PrintFormat,
};

#[derive(Serialize)]
pub struct ValidationStore {
    pub(super) validators: BTreeMap<ValidatorCode, Validator>,
}

impl ValidationStore {
    pub fn validate_trace(&mut self, trace: &mut AnnotatedTrace) {
        for validator in self.validators.values_mut() {
            validator.reset();
        }

        trace.validate(&mut self.validators);
    }

    pub(super) fn explain(&self, code: &ValidatorCode) -> EmptyResult {
        let v = self.lookup(code)?;
        println!("{} ({code})", v.name);
        println!("{:=<80}", "");
        println!("{}", self.lookup(code)?.help);
        Ok(())
    }

    pub(super) fn lookup<'a>(&'a self, code: &ValidatorCode) -> anyhow::Result<&'a Validator> {
        self.validators.get(code).ok_or(anyhow!("code not found: {code}"))
    }

    pub(super) fn print(&self, format: &PrintFormat) -> EmptyResult {
        match format {
            PrintFormat::Json => print!("{}", serde_json::to_string(self)?),
            PrintFormat::List => self.print_list()?,
            PrintFormat::Table => self.print_table()?,
            PrintFormat::Yaml => print!("{}", serde_yaml::to_string(self)?),
        }

        Ok(())
    }

    pub(super) fn register(&mut self, v: Validator) {
        let code = ValidatorCode(v.type_, self.validators.len());
        self.register_with_code(code, v);
    }

    pub(super) fn register_with_code(&mut self, code: ValidatorCode, v: Validator) {
        self.validators.insert(code, v);
    }

    fn print_list(&self) -> EmptyResult {
        for (code, validator) in self.validators.iter() {
            println!("{} ({}): {}", code, validator.name, validator.help());
        }
        Ok(())
    }

    fn print_table(&self) -> EmptyResult {
        println!("| code | name | description |");
        println!("|---|---|---|");
        for (code, validator) in self.validators.iter() {
            println!("| {} | {} | {} |", code, validator.name, validator.help());
        }
        Ok(())
    }
}

impl Default for ValidationStore {
    fn default() -> Self {
        let mut store = ValidationStore { validators: BTreeMap::new() };

        store.register(status_field_populated::validator());

        store
    }
}
