use std::collections::{
    HashMap,
    HashSet,
};

use anyhow::*;
use sk_core::prelude::*;
use sk_core::trace::PodSimData;
use sk_core::trace::index::{
    TraceIndex,
    TraceIndexEntry,
};
use tracing::*;

// The OwnersIndex stores relevant information for all resources tracked by the tracer, as well
// as simulation data for any pods that belong to those (types of) resources.  This data is:
//
//   - most recent hash of the owning resource spec field
//   - a vector of modification times for the owning resource spec
//   - a vector of relevant simulation data for pods owned by the resource
//
// It is important to note that fields in this index are updated independently and asychronously by
// the tracer.  Specifically, the dynamic object watcher updates the hash and mtimes, and the pod
// watcher updates the pod sim data.  Thus it is possible for there to be pod data without hash or
// mtime data, or vice versa, particularly if one of the watchers gets behind.
//
// Control to this resource is gated through the store by an Arc<Mutex<...>> so that we don't have
// both watchers trying to update the same entry at the same time.
//
// An object is marked as "deleted" if the hash value is set to None; since every update to the hash
// value will record a corresponding mtime in the mtimes vector, if you _need_ to tell the
// difference between a deleted object and a partially-filled object, you can check if hash is None
// and mtimes is non-empty.  So far that hasn't actually come up though :fingers-crossed:

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct OwnersIndexEntry {
    hash: Option<u64>,
    mtimes: Vec<i64>,
    pod_sim_data: Vec<PodSimData>,
}

#[derive(Debug, Default)]
pub(crate) struct OwnersIndex {
    m: HashMap<KubeResourceId, OwnersIndexEntry>,
    pod_index: HashMap<String, (KubeResourceId, usize)>,
}

impl OwnersIndex {
    pub fn contains(&self, owner_id: &KubeResourceId) -> bool {
        self.m.contains_key(owner_id)
    }

    pub fn get_hash(&self, owner_id: &KubeResourceId) -> Option<u64> {
        self.m.get(owner_id)?.hash
    }

    pub fn get_pod_lifecycles(&self, pod_ns_name: &str) -> Vec<PodLifecycleData> {
        if let Some((id, _)) = self.pod_index.get(pod_ns_name)
            && let Some(entry) = self.m.get(id)
        {
            entry.pod_sim_data.iter().cloned().map(|psd| psd.lifecycle).collect()
        } else {
            vec![]
        }
    }

    pub fn has_pod(&self, ns_name: &str) -> bool {
        self.pod_index.contains_key(ns_name)
    }

    pub fn store_object(&mut self, owner_id: KubeResourceId, hash: u64, mtime: i64) {
        let entry = self.m.entry(owner_id).or_default();
        entry.hash = Some(hash);

        // This "should" never happen
        assert!(entry.mtimes.last().is_none_or(|last_mtime| mtime >= *last_mtime));

        // TODO what if the mtimes are equal?
        entry.mtimes.push(mtime);
    }

    pub fn remove_object(&mut self, owner_id: &KubeResourceId) {
        if let Some(entry) = self.m.get_mut(owner_id) {
            entry.hash = None;
        }
    }

    pub fn store_new_pod_lifecycle(
        &mut self,
        pod_ns_name: &str,
        owner_id: &KubeResourceId,
        lifecycle: PodLifecycleData,
    ) -> EmptyResult {
        info!("inserting lifecycle data for {pod_ns_name} owned by {owner_id}: {lifecycle:?}");

        if self.has_pod(pod_ns_name) {
            bail!("pod {pod_ns_name} already exists in index");
        }

        let entry = self.m.entry(owner_id.clone()).or_default();

        // This "should" never happen
        assert!(
            entry
                .pod_sim_data
                .last()
                .is_none_or(|psd| lifecycle.start_ts() >= psd.lifecycle.start_ts())
        );

        entry.pod_sim_data.push(PodSimData::new(lifecycle));
        let idx = entry.pod_sim_data.len() - 1;
        self.pod_index.insert(pod_ns_name.into(), (owner_id.clone(), idx));
        Ok(())
    }

    pub fn update_pod_lifecycle(&mut self, pod_ns_name: &str, lifecycle: PodLifecycleData) -> EmptyResult {
        match self.pod_index.get(pod_ns_name) {
            None => bail!("pod {pod_ns_name} not present in index"),
            Some((owner_id, sequence_idx)) => {
                let owner_entry = self
                    .m
                    .get_mut(owner_id)
                    .ok_or(anyhow!("no owner entry for pod {pod_ns_name}"))?;
                let pod_entry = owner_entry
                    .pod_sim_data
                    .get_mut(*sequence_idx)
                    .ok_or(anyhow!("no sequence index {sequence_idx} for pod {pod_ns_name}"))?;

                info!("updating pod {pod_ns_name} owned by {owner_id}: {lifecycle:?}");
                pod_entry.lifecycle = lifecycle;
                Ok(())
            },
        }
    }

    // Given an set of "owning objects", get all the pod lifecycles between a given start and end
    // time that belong to one of those owning objects, grouped by the modification timestamps of
    // the owning resource
    pub fn aggregate_pod_sim_data(
        &self,
        start_ts: i64,
        end_ts: i64,
        owning_objects: &HashSet<KubeResourceId>,
    ) -> TraceIndex {
        let mut index = TraceIndex::new();
        for (owner_id, entry) in &self.m {
            // The filtering is a little complicated here; if the owning object isn't in the index,
            // we discard it.  Also, if none of the pods belonging to the owning object land
            // within the given time window, we want to discard it.  Otherwise, we want to filter
            // down the list of pods to the ones that fall between the given time window.
            if !owning_objects.contains(owner_id) {
                continue;
            }

            let Some(buckets) = bucket_pod_sim_data_by_mtime(start_ts, end_ts, entry) else {
                continue;
            };

            index.insert(owner_id.clone(), buckets);
        }
        index
    }
}

pub fn bucket_pod_sim_data_by_mtime(start_ts: i64, end_ts: i64, entry: &OwnersIndexEntry) -> Option<TraceIndexEntry> {
    let filtered_sim_data: Vec<PodSimData> = entry
        .pod_sim_data
        .iter()
        .cloned()
        .filter_map(|sim_data| {
            if sim_data.overlaps(start_ts, end_ts) {
                // The timing for any lifecycle that _contains_ the start time of the trace
                // will get truncated; this is necessary for the "bare pods" scenario, and
                // in any other scenario it can either be correct or incorrect to do so.
                // For the ease of the code right now, I'm just unilaterally truncating the
                // time, but if we need to make it more rigorous later we can do so.
                Some(sim_data.bound_start_ts(start_ts))
            } else {
                None
            }
        })
        .collect();

    if filtered_sim_data.is_empty() {
        return None;
    }

    let mut grouped_lifecycles = TraceIndexEntry::new();
    let mut rest: &[PodSimData] = &filtered_sim_data;
    for &mtime in entry.mtimes.iter().rev() {
        let idx = rest.partition_point(|psd| psd.lifecycle.start_ts().is_some_and(|ts| ts < mtime));
        let (head, tail) = rest.split_at(idx);
        if !tail.is_empty() {
            grouped_lifecycles.insert(mtime, tail.into());
        }
        rest = head;
    }
    Some(grouped_lifecycles)
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
impl OwnersIndex {
    pub fn len(&self) -> usize {
        self.m.values().filter(|e| e.hash.is_some()).count()
    }

    pub fn lifecycle_data_for<'a>(&'a self, owner_id: &KubeResourceId) -> Option<&'a OwnersIndexEntry> {
        self.m.get(owner_id)
    }

    pub fn new_from_parts(
        m: HashMap<KubeResourceId, OwnersIndexEntry>,
        pod_index: HashMap<String, (KubeResourceId, usize)>,
    ) -> OwnersIndex {
        Self { m, pod_index }
    }

    pub fn pod_index_entry(&self, pod_ns_name: &str) -> Option<&(KubeResourceId, usize)> {
        self.pod_index.get(pod_ns_name)
    }
}

#[cfg(test)]
impl OwnersIndexEntry {
    pub fn new_from_parts(hash: Option<u64>, mtimes: Vec<i64>, pod_sim_data: Vec<PodSimData>) -> OwnersIndexEntry {
        Self { hash, mtimes, pod_sim_data }
    }

    pub fn sim_data(&self) -> &[PodSimData] {
        &self.pod_sim_data
    }
}
