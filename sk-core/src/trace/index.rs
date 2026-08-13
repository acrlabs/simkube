use std::collections::{
    HashMap,
    HashSet,
};
use std::mem::take;

use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::{
    GVK,
    KubeResourceId,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TraceIndex {
    #[serde(flatten)]
    index: HashMap<GVK, HashMap<String, u64>>,
}

impl TraceIndex {
    pub fn new() -> TraceIndex {
        TraceIndex::default()
    }

    pub fn contains(&self, resource_id: &KubeResourceId) -> bool {
        self.index
            .get(&resource_id.gvk)
            .is_some_and(|gvk_hash| gvk_hash.contains_key(&resource_id.ns_name))
    }

    pub fn flattened_keys(&self) -> HashSet<KubeResourceId> {
        self.index
            .iter()
            .flat_map(|(gvk, gvk_hash)| {
                gvk_hash
                    .keys()
                    .map(move |ns_name| KubeResourceId::new(gvk.clone(), ns_name.clone()))
            })
            .collect()
    }

    pub fn get(&self, resource_id: &KubeResourceId) -> Option<u64> {
        self.index.get(&resource_id.gvk)?.get(&resource_id.ns_name).cloned()
    }

    pub fn insert(&mut self, resource_id: &KubeResourceId, hash: u64) {
        self.index
            .entry(resource_id.gvk.clone())
            .or_default()
            .insert(resource_id.ns_name.clone(), hash);
    }

    pub fn is_empty(&self) -> bool {
        self.index.values().all(|gvk_hash| gvk_hash.is_empty())
    }

    pub fn len(&self) -> usize {
        self.index.values().map(|gvk_hash| gvk_hash.len()).sum()
    }

    pub fn remove(&mut self, resource_id: &KubeResourceId) {
        self.index.entry(resource_id.gvk.clone()).and_modify(|gvk_hash| {
            gvk_hash.remove(&resource_id.ns_name);
        });
    }

    pub fn take_gvk_index(&mut self, gvk: &GVK) -> HashMap<String, u64> {
        take(self.index.get_mut(gvk).unwrap_or(&mut HashMap::new()))
    }
}
