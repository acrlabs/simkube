use std::collections::BTreeMap;
use std::sync::{
    LazyLock,
    Mutex,
};

use anyhow::anyhow;
use assertables::assert_all;
use serde::Serialize;
use sk_core::prelude::*;
use sk_store::ExportedTrace;

use super::PrintFormat;
use super::rules::*;
use super::validator::{
    Validator,
    ValidatorCode,
};

pub type Annotations = BTreeMap<ValidatorCode, Vec<usize>>;

#[derive(Serialize)]
pub struct ValidationStore {
    pub(super) validators: BTreeMap<ValidatorCode, Validator>,
}

impl ValidationStore {
    pub fn validate_trace(&mut self, trace: &ExportedTrace) -> anyhow::Result<BTreeMap<usize, Annotations>> {
        let mut annotations: BTreeMap<usize, Annotations> = BTreeMap::new();

        for (i, (event, _)) in trace.iter().enumerate() {
            for (code, validator) in self.validators.iter_mut() {
                let mut failed_obj_indices = validator.check_next_event(event, &trace.config)?;
                annotations
                    .entry(i)
                    .or_default()
                    .entry(*code)
                    .or_default()
                    .append(&mut failed_obj_indices);
            }
        }

        Ok(annotations)
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
        store.register(missing_resources::configmap_envvar_validator());
        store.register(missing_resources::secret_volume_validator());
        store.register(missing_resources::configmap_volume_validator());

        store
    }

    fn register(&mut self, v: Validator) {
        // Runtime smoke test to make sure we don't have multiple validators with the same name
        assert_all!(self.validators.iter(), |(_, other): (&ValidatorCode, &Validator)| other.name != v.name);

        let code = ValidatorCode(v.type_, self.validators.len());
        self.register_with_code(code, v);
    }

    fn register_with_code(&mut self, code: ValidatorCode, v: Validator) {
        self.validators.insert(code, v);
    }
}

pub static VALIDATORS: LazyLock<Mutex<ValidationStore>> = LazyLock::new(|| Mutex::new(ValidationStore::new()));
