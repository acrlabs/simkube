use std::collections::BTreeMap;
use std::fmt; // BTreeMap sorts by key, HashMap doesn't sort

use anyhow::anyhow;
use serde::{
    Serialize,
    Serializer,
};
use sk_core::prelude::*;

use super::annotated_trace::{
    AnnotatedTrace,
    AnnotatedTraceEvent,
};
use super::{
    status_field_populated,
    PrintFormat,
};

#[derive(Eq, Hash, PartialEq, Serialize)]
pub enum ValidatorType {
    Warning,
    #[allow(dead_code)]
    Error,
}

impl fmt::Display for ValidatorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ValidatorType::Warning => 'W',
                ValidatorType::Error => 'E',
            }
        )
    }
}

pub trait Diagnostic {
    fn check_next_event(&mut self, evt: &mut AnnotatedTraceEvent) -> Vec<usize>;
    fn reset(&mut self);
}

#[derive(Serialize)]
pub struct Validator {
    #[serde(rename = "type")]
    pub type_: ValidatorType,
    pub name: &'static str,

    #[serde(serialize_with = "flatten_str")]
    pub help: &'static str,

    #[serde(skip)]
    pub diagnostic: Box<dyn Diagnostic>,
}

impl Validator {
    fn check_next_event(&mut self, a_event: &mut AnnotatedTraceEvent) -> Vec<usize> {
        self.diagnostic.check_next_event(a_event)
    }

    fn reset(&mut self) {
        self.diagnostic.reset()
    }

    fn help(&self) -> String {
        self.help.replace('\n', " ")
    }
}

#[derive(Serialize)]
pub struct ValidationStore {
    validators: BTreeMap<String, Validator>,
}

impl ValidationStore {
    pub(super) fn validate_trace(&mut self, trace: &mut AnnotatedTrace) {
        for validator in self.validators.values_mut() {
            validator.reset();
        }

        for evt in trace.events.iter_mut() {
            for (code, validator) in self.validators.iter_mut() {
                let mut affected_indices: Vec<_> =
                    validator.check_next_event(evt).into_iter().map(|i| (i, code.clone())).collect();
                trace
                    .summary
                    .entry(code.clone())
                    .and_modify(|e| *e += affected_indices.len())
                    .or_insert(affected_indices.len());

                // This needs to happen at the ends, since `append` consumes affected_indices' contents
                evt.annotations.append(&mut affected_indices);
            }
        }
    }

    pub(super) fn explain(&self, code: &str) -> EmptyResult {
        let v = self.lookup(code)?;
        println!("{} ({code})", v.name);
        println!("{:=<80}", "");
        println!("{}", self.lookup(code)?.help);
        Ok(())
    }

    pub(super) fn lookup<'a>(&'a self, code: &str) -> anyhow::Result<&'a Validator> {
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

    fn register(&mut self, v: Validator) {
        let code = format!("{}{:04}", v.type_, self.validators.len());
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

fn flatten_str<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&s.replace('\n', " "))
}
