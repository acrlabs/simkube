use std::collections::BTreeMap;

use serde_json::Value;

pub(super) type MatchContext = BTreeMap<String, MatchContextEntry>;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct MatchContextEntry {
    pointers: Vec<String>,
    values: Vec<Value>,
}

impl MatchContextEntry {
    pub(super) fn new() -> Self {
        MatchContextEntry { pointers: vec![], values: vec![] }
    }

    pub(super) fn insert(&mut self, pointer: String, value: Value) {
        self.pointers.push(pointer);
        self.values.push(value);
    }

    pub(super) fn len(&self) -> usize {
        self.pointers.len()
    }

    pub(super) fn pointers(&self) -> &Vec<String> {
        &self.pointers
    }

    pub(super) fn values(&self) -> &Vec<Value> {
        &self.values
    }
}

#[cfg(test)]
impl MatchContextEntry {
    pub(super) fn new_from_parts(pointers: Vec<String>, values: Vec<Value>) -> Self {
        MatchContextEntry { pointers, values }
    }
}
