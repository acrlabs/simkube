use std::fmt;

use serde::{
    Serialize,
    Serializer,
};

use super::annotated_trace::AnnotatedTraceEvent;

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
    pub fn check_next_event(&mut self, a_event: &mut AnnotatedTraceEvent) -> Vec<usize> {
        self.diagnostic.check_next_event(a_event)
    }

    pub fn reset(&mut self) {
        self.diagnostic.reset()
    }

    pub fn help(&self) -> String {
        self.help.replace('\n', " ")
    }
}

fn flatten_str<S: Serializer>(s: &str, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&s.replace('\n', " "))
}
