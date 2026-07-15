use std::collections::HashMap;

use sk_core::k8s::GVK;

#[derive(Clone, Debug, Default)]
pub struct TraceIndex {
    index: HashMap<GVK, HashMap<String, u64>>,
}

impl TraceIndex {
    pub fn contains(&self, gvk: &GVK, ns_name: &str) -> bool {
        self.index.get(gvk).is_some_and(|gvk_hash| gvk_hash.contains_key(ns_name))
    }

    pub fn get(&self, gvk: &GVK, ns_name: &str) -> Option<u64> {
        self.index.get(gvk)?.get(ns_name).cloned()
    }

    pub fn insert(&mut self, gvk: GVK, ns_name: String, hash: u64) {
        self.index.entry(gvk).or_default().insert(ns_name, hash);
    }

    pub fn remove(&mut self, gvk: GVK, ns_name: &str) {
        self.index.entry(gvk).and_modify(|gvk_hash| {
            gvk_hash.remove(ns_name);
        });
    }
}

#[cfg(test)]
impl TraceIndex {
    pub fn len(&self) -> usize {
        self.index.values().map(|gvk_hash| gvk_hash.len()).sum()
    }
}
