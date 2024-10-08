use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't sort
use std::fmt;
mod status_field_populated;

use crate::TraceEvent;

type DiagnosticFunction = Box<dyn Fn(&[TraceEvent]) -> Vec<(usize, usize)>>;

#[derive(Eq, Hash, PartialEq)]
pub enum ValidatorType {
    Warning,
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

pub struct Validator {
    pub type_: ValidatorType,
    pub name: String,
    pub help: String,
    pub check: DiagnosticFunction,
}

pub struct ValidationStore {
    validators: BTreeMap<String, Validator>,
}

#[allow(clippy::new_without_default)]
impl ValidationStore {
    pub fn new() -> Self {
        let mut store = ValidationStore { validators: BTreeMap::new() };

        store.register(status_field_populated::validator());

        store
    }

    fn register(&mut self, v: Validator) {
        let code = format!("{}{:04}", v.type_, self.validators.len());
        self.validators.insert(code, v);
    }
}

impl fmt::Display for ValidationStore {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "| code | name | description |")?;
        writeln!(f, "|---|---|---|")?;
        for (code, validator) in self.validators.iter() {
            writeln!(f, "| {} | {} | {} |", code, validator.name, validator.help.replace('\n', " "))?;
        }
        Ok(())
    }
}
