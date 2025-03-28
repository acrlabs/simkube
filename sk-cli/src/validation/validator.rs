use std::fmt;
use std::str::from_utf8;
use std::sync::{
    Arc,
    RwLock,
};

use anyhow::bail;
use serde::{
    Serialize,
    Serializer,
};
use sk_store::TracerConfig;

use super::annotated_trace::{
    AnnotatedTraceEvent,
    AnnotatedTracePatch,
};

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

// The CheckResult from a single validator for a specific event is a vector of (object index, patch
// list) tuples; the object index follows the "applied objects, then deleted objects" convention,
// and the patch list is the list of potential fixes for this specific issue.
//
// This makes it easier for a particular validator to find _multiple_ issues with a single object;
// for example, a pod could have multiple missing config-map volumes.  In this setting, validator
// can just stick multiple tuples referencing the same index in the list.  They'll get all
// aggregated together in the next layer up.
pub type CheckResult = anyhow::Result<Vec<(usize, Vec<AnnotatedTracePatch>)>>;

pub trait Diagnostic {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent, config: &TracerConfig) -> CheckResult;
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
    pub fn check_next_event(&self, event: &mut AnnotatedTraceEvent, config: &TracerConfig) -> CheckResult {
        self.diagnostic.write().unwrap().check_next_event(event, config)
    }

    pub fn reset(&self) {
        self.diagnostic.write().unwrap().reset()
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
    use rstest::*;

    use super::*;

    #[rstest]
    fn test_parse_validator_code() {
        assert_eq!(ValidatorCode::parse("E0001").unwrap(), ValidatorCode(ValidatorType::Error, 1));
        assert_eq!(ValidatorCode::parse("W0001").unwrap(), ValidatorCode(ValidatorType::Warning, 1));
        assert_err!(ValidatorCode::parse("asdf"));
    }
}
