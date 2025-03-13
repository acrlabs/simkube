use std::fmt;
use std::str::from_utf8;
use std::sync::{Arc, RwLock};

use anyhow::bail;
use json_patch_ext::prelude::*;
use serde::{Serialize, Serializer};

use super::annotated_trace::AnnotatedTraceEvent;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ValidatorType {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ValidatorCode(pub ValidatorType, pub usize);

impl ValidatorCode {
    pub fn parse(s: &str) -> anyhow::Result<ValidatorCode> {
        if s.is_empty() {
            bail!("empty string");
        }

        let chars = s.as_bytes();
        let t = match chars[0] {
            b'W' => ValidatorType::Warning,
            b'E' => ValidatorType::Error,
            _ => bail!("unknown type"),
        };
        let id = from_utf8(&chars[1..])?.parse::<usize>()?;
        Ok(ValidatorCode(t, id))
    }
}

impl fmt::Display for ValidatorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{:04}",
            match self.0 {
                ValidatorType::Warning => 'W',
                ValidatorType::Error => 'E',
            },
            self.1,
        )
    }
}

pub trait Diagnostic {
    fn check_next_event(&mut self, evt: &mut AnnotatedTraceEvent) -> Vec<usize>;
    fn fixes(&self) -> Vec<PatchOperation>;
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
    pub diagnostic: Arc<RwLock<dyn Diagnostic + Send + Sync>>,
}

impl Validator {
    pub fn check_next_event(&self, a_event: &mut AnnotatedTraceEvent) -> Vec<usize> {
        self.diagnostic.write().unwrap().check_next_event(a_event)
    }

    pub fn fixes(&self) -> Vec<PatchOperation> {
        self.diagnostic.read().unwrap().fixes()
    }

    pub fn reset(&self) {
        self.diagnostic.write().unwrap().reset()
    }

    pub fn help(&self) -> String {
        self.help.replace('\n', " ")
    }
}

fn flatten_str<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&s.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn test_parse_validator_code() {
        assert_eq!(ValidatorCode::parse("E0001").unwrap(), ValidatorCode(ValidatorType::Error, 1));
        assert_eq!(ValidatorCode::parse("W0001").unwrap(), ValidatorCode(ValidatorType::Warning, 1));
        assert_err!(ValidatorCode::parse("asdf"));
    }
}
