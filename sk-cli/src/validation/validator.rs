use std::fmt;
use std::str::from_utf8;

use anyhow::bail;
use serde::{
    Serialize,
    Serializer,
};
use sk_core::prelude::*;
use sk_store::TraceEvent;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ValidatorType {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

impl Serialize for ValidatorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // reuse the display impl for serializing
        serializer.serialize_str(&format!("{self}"))
    }
}

pub trait Diagnostic {
    fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>>;
}

#[derive(Serialize)]
pub struct Validator {
    #[serde(rename = "type")]
    pub type_: ValidatorType,
    pub name: &'static str,

    #[serde(serialize_with = "flatten_str")]
    pub help: &'static str,

    pub skel_suggestion: &'static str,

    #[serde(skip)]
    pub diagnostic: Box<dyn Diagnostic + Send + Sync>,
}

impl Validator {
    pub fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        self.diagnostic.check_next_event(event, config)
    }

    pub fn help(&self) -> String {
        self.help.replace('\n', " ")
    }
}

impl fmt::Debug for Validator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Validator")
            .field("type", &self.type_)
            .field("name", &self.name)
            .finish()
    }
}

fn flatten_str<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&s.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use sk_testutils::*;

    use super::*;

    #[rstest]
    fn test_parse_validator_code() {
        assert_eq!(ValidatorCode::parse("E0001").unwrap(), ValidatorCode(ValidatorType::Error, 1));
        assert_eq!(ValidatorCode::parse("W0001").unwrap(), ValidatorCode(ValidatorType::Warning, 1));
        assert_err!(ValidatorCode::parse("asdf"));
    }
}
