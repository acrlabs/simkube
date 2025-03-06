use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't

use anyhow::anyhow;
use lazy_static::lazy_static;
use serde::Serialize;
use sk_core::prelude::*;

use super::rules::*;
use super::summary::ValidationSummary;
use super::validator::{
    Validator,
    ValidatorCode,
};
use super::{
    AnnotatedTrace,
    PrintFormat,
};

#[derive(Serialize)]
pub struct ValidationStore {
    pub(super) validators: BTreeMap<ValidatorCode, Validator>,
}

impl ValidationStore {
    pub fn validate_trace(&self, trace: &mut AnnotatedTrace, fix: bool) -> anyhow::Result<ValidationSummary> {
        for validator in self.validators.values() {
            validator.reset();
        }

        let mut summary = ValidationSummary::default();
        let mut summary_populated = false;
        loop {
            let s = trace.validate(&self.validators)?;
            if !summary_populated {
                summary.annotations = s;
                summary_populated = true;
            }

            if !fix {
                break;
            }

            let Some(next_annotation) = trace.get_next_annotation() else {
                break;
            };

            let Some(patch) = next_annotation.patches.first().cloned() else {
                println!("no fix available for {}; continuing", next_annotation.code);
                break;
            };
            summary.patches += trace.apply_patch(patch)?;
        }

        Ok(summary)
    }

    pub(super) fn explain(&self, code: &ValidatorCode) -> EmptyResult {
        let v = self.lookup(code)?;
        println!("{} ({code})", v.name);
        println!("{:=<80}", "");
        println!("{}", v.help);
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

    fn new() -> ValidationStore {
        let mut store = ValidationStore { validators: BTreeMap::new() };

        store.register(status_field_populated::validator());
        store.register(missing_resources::service_account_validator());
        store.register(missing_resources::secret_envvar_validator());

        store
    }

    fn register(&mut self, v: Validator) {
        let code = ValidatorCode(v.type_, self.validators.len());
        self.register_with_code(code, v);
    }

    fn register_with_code(&mut self, code: ValidatorCode, v: Validator) {
        self.validators.insert(code, v);
    }
}

lazy_static! {
    pub static ref VALIDATORS: ValidationStore = ValidationStore::new();
}
