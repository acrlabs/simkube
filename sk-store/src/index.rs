use std::collections::HashMap;
use std::mem::take;

use serde::{Deserialize, Serialize};
use sk_core::k8s::{format_gvk_name, GVK};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct TraceIndex {
    #[serde(flatten)]
    index: HashMap<GVK, HashMap<String, u64>>,
}

impl TraceIndex {
    pub fn new() -> TraceIndex {
        TraceIndex::default()
    }

    pub fn contains(&self, gvk: &GVK, ns_name: &str) -> bool {
        self.index.get(gvk).is_some_and(|gvk_hash| gvk_hash.contains_key(ns_name))
    }

    pub fn flattened_keys(&self) -> Vec<String> {
        self.index
            .iter()
            .flat_map(|(gvk, gvk_hash)| gvk_hash.keys().map(move |ns_name| format_gvk_name(gvk, ns_name)))
            .collect()
    }

    pub fn get(&self, gvk: &GVK, ns_name: &str) -> Option<u64> {
        self.index.get(gvk)?.get(ns_name).cloned()
    }

    pub fn insert(&mut self, gvk: GVK, ns_name: String, hash: u64) {
        self.index.entry(gvk).or_default().insert(ns_name, hash);
    }

    pub fn is_empty(&self) -> bool {
        self.index.values().all(|gvk_hash| gvk_hash.is_empty())
    }

    pub fn len(&self) -> usize {
        self.index.values().map(|gvk_hash| gvk_hash.len()).sum()
    }

    pub fn remove(&mut self, gvk: GVK, ns_name: &str) {
        self.index.entry(gvk).and_modify(|gvk_hash| {
            gvk_hash.remove(ns_name);
        });
    }

    pub fn take_gvk_index(&mut self, gvk: &GVK) -> HashMap<String, u64> {
        take(self.index.get_mut(gvk).unwrap_or(&mut HashMap::new()))
    }
}
