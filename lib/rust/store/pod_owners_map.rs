use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::errors::*;
use crate::k8s::PodLifecycleData;

pub(super) type PodLifecyclesMap = HashMap<u64, Vec<PodLifecycleData>>;

#[derive(Default)]
pub(super) struct PodOwnersMap {
    m: HashMap<String, PodLifecyclesMap>,
    index: HashMap<String, (String, u64, usize)>,
}

impl PodOwnersMap {
    pub(super) fn new_from_parts(
        m: HashMap<String, PodLifecyclesMap>,
        index: HashMap<String, (String, u64, usize)>,
    ) -> PodOwnersMap {
        PodOwnersMap { m, index }
    }

    pub(super) fn has_pod(&self, ns_name: &str) -> bool {
        self.index.contains_key(ns_name)
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
                e.insert([(hash, vec![lifecycle_data])].into());
                0
            },
            Entry::Occupied(mut e) => {
                let pod_sequence = e.get_mut().entry(hash).or_insert(vec![]);
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

    pub(super) fn filter(
        &self,
        start_ts: i64,
        end_ts: i64,
        index: &HashMap<String, u64>,
    ) -> HashMap<String, PodLifecyclesMap> {
        self.m
            .iter()
            .filter_map(|(owner, lifecycles_map)| {
                if !index.contains_key(owner) {
                    return None;
                }

                Some((owner.clone(), filter_lifecycles_map(start_ts, end_ts, lifecycles_map)?))
            })
            .collect()
    }
}

pub(super) fn filter_lifecycles_map(
    start_ts: i64,
    end_ts: i64,
    lifecycles_map: &PodLifecyclesMap,
) -> Option<PodLifecyclesMap> {
    let filtered_map: PodLifecyclesMap = lifecycles_map
        .iter()
        .filter_map(|(hash, lifecycles)| {
            let new_lifecycles: Vec<_> = lifecycles.iter().filter(|l| l.overlaps(start_ts, end_ts)).cloned().collect();
            if new_lifecycles.is_empty() {
                return None;
            }
            Some((*hash, new_lifecycles))
        })
        .collect();

    if filtered_map.is_empty() {
        return None;
    }
    Some(filtered_map)
}

#[cfg(test)]
impl PodOwnersMap {
    pub(super) fn lifecycle_data_for(&self, owner_ns_name: &str, pod_hash: &u64) -> Option<Vec<PodLifecycleData>> {
        Some(self.m.get(owner_ns_name)?.get(pod_hash)?.clone())
    }

    pub(super) fn pod_owner_meta(&self, ns_name: &str) -> Option<&(String, u64, usize)> {
        self.index.get(ns_name)
    }
}
