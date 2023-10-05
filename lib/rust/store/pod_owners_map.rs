use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::errors::*;
use crate::k8s::PodLifecycleData;

#[derive(Default)]
pub(super) struct PodOwnersMap {
    m: HashMap<String, HashMap<u64, Vec<PodLifecycleData>>>,
    index: HashMap<String, (String, u64, usize)>,
}

impl PodOwnersMap {
    pub(super) fn has_pod(&self, ns_name: &str) -> bool {
        self.index.contains_key(ns_name)
    }

    pub(super) fn lifecycle_data_for(&self, owner_ns_name: &str, pod_hash: &u64) -> Option<Vec<PodLifecycleData>> {
        Some(self.m.get(owner_ns_name)?.get(pod_hash)?.clone())
    }

    pub(super) fn store_new_pod_lifecycle(
        &mut self,
        ns_name: &str,
        owner_ns_name: &str,
        hash: u64,
        lifecycle_data: PodLifecycleData,
    ) {
        let idx = match self.m.entry(owner_ns_name.into()) {
            Entry::Vacant(e) => {
                e.insert(HashMap::from([(hash, vec![lifecycle_data])]));
                0
            },
            Entry::Occupied(mut e) => {
                let pod_sequence = e.get_mut().get_mut(&hash).unwrap();
                pod_sequence.push(lifecycle_data);
                pod_sequence.len() - 1
            },
        };
        self.index.insert(ns_name.into(), (owner_ns_name.into(), hash, idx));
    }

    pub(super) fn update_pod_lifecycle(&mut self, ns_name: &str, lifecycle_data: PodLifecycleData) -> EmptyResult {
        match self.index.get(ns_name) {
            None => bail!("pod {} not present in index", ns_name),
            Some((owner_ns_name, hash, sequence_idx)) => {
                let owner_entry = self
                    .m
                    .get_mut(owner_ns_name)
                    .ok_or(anyhow!("no owner entry for pod {}", ns_name))?;
                let pods =
                    owner_entry
                        .get_mut(hash)
                        .ok_or(anyhow!("no entry for pod {} matching hash {}", ns_name, hash))?;
                let pod_entry = pods.get_mut(*sequence_idx).ok_or(anyhow!(
                    "no sequence index {} for pod {} matching hash {}",
                    sequence_idx,
                    ns_name,
                    hash
                ))?;
                *pod_entry = lifecycle_data;
                Ok(())
            },
        }
    }
}

#[cfg(test)]
impl PodOwnersMap {
    pub(super) fn new_from_parts(
        m: HashMap<String, HashMap<u64, Vec<PodLifecycleData>>>,
        index: HashMap<String, (String, u64, usize)>,
    ) -> PodOwnersMap {
        PodOwnersMap { m, index }
    }
}
