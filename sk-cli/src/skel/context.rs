use std::collections::{
    BTreeMap,
    btree_map,
};

use serde_json::Value;

#[derive(Debug)]
pub(super) struct MatchContext {
    // Duplicate the obj we're operating on for read-only purposes (looking
    // up values of pointers); mainly we're using this to minimize stuff we're
    // passing around to functions, we may get rid of it later if it's too confusing.
    obj: Value,
    entries: BTreeMap<String, MatchContextEntry>,
}

impl MatchContext {
    pub(super) fn new(obj: Value) -> Self {
        MatchContext { obj, entries: BTreeMap::new() }
    }

    pub(super) fn insert(&mut self, var: String, entry: MatchContextEntry) {
        self.entries.insert(var, entry);
    }

    #[allow(dead_code)] // used in tests
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn iter(&self) -> btree_map::Iter<'_, String, MatchContextEntry> {
        self.entries.iter()
    }

    pub(super) fn obj(&self) -> &Value {
        &self.obj
    }
}

impl std::ops::Index<&str> for MatchContext {
    type Output = MatchContextEntry;

    fn index(&self, index: &str) -> &Self::Output {
        &self.entries[index]
    }
}

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

    #[allow(dead_code)] // used in tests
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
