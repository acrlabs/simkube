use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::errors::*;
use crate::k8s::PodLifecycleData;
use crate::prelude::*;

// The PodOwnersMap tracks lifecycle data for all pods that are owned by some object that we care
// about (e.g., if we are tracking Deployments, the owners map will track the lifecycle data for
// all pods that are owned by a Deployment).
//
// For simulation purposes, we make the assumption that pods launched _in the same order_ will
// exhibit the same lifecycle characteristics.  This assumption isn't going to be universally true,
// of course, but I think it's a reasonable proxy.  We could also do things in the simulator driver
// like "compute the mean/stdev" of pods, and launch new pods with lifecycle data randomly sampled
// from that distribution.
//
// Some Kubernetes objects (particularly custom resources) can have multiple "pod types" that
// belong to them.  For example, VolcanoJobs take in a list of different pod templates that should
// all be launched as part of the same job.  We differentiate by hash of the pod spec (this also
// lets us track changes to deployments that cause a rolling update, for example).
//
// Consequentially, the actual data structure that we use here ends up being fairly complex.  It
// looks like this:
//
// - Owning Object: pod_spec_hash1:
//     - start_ts1 end_ts1
//     - start_ts2
//     - start_ts3 end_ts3
//   pod_spec_hash2:
//     - start_tsA end_tsA
//
// Each owning object keeps track of all the pods that we've seen that belong to it.  These are
// stored nested by hash, and then each hash keeps a vector of pod lifecycle data.  The order
// events appear in the pod lifecycle vector corresponds to the order in which pods were launched.
//
// To be able to efficiently handle updates, we also keep an index which maps pod name to the
// owning object, hash, and vector position containing its lifecycle data.
//
// TODO: possible improvement?  Since we're tracking the start_ts for every object, maybe we can
// just discard the ordering information and store them in a slightly less complicated data
// structure?  Then we could just sort by start_ts in the simulation driver if we wanted them in
// order.  I am not sure if there's any actual improvements to be had here, though.

pub type PodLifecyclesMap = HashMap<u64, Vec<PodLifecycleData>>;

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

    pub(super) fn lifecycle_data_for<'a>(
        &'a self,
        owner_ns_name: &str,
        pod_hash: u64,
    ) -> Option<&'a Vec<PodLifecycleData>> {
        self.m.get(owner_ns_name)?.get(&pod_hash)
    }

    pub(super) fn store_new_pod_lifecycle(
        &mut self,
        ns_name: &str,
        owner_ns_name: &str,
        hash: u64,
        lifecycle_data: &PodLifecycleData,
    ) {
        let idx = match self.m.entry(owner_ns_name.into()) {
            Entry::Vacant(e) => {
                e.insert([(hash, vec![lifecycle_data.clone()])].into());
                0
            },
            Entry::Occupied(mut e) => {
                let pod_sequence = e.get_mut().entry(hash).or_insert(vec![]);
                pod_sequence.push(lifecycle_data.clone());
                pod_sequence.len() - 1
            },
        };

        info!("inserting pod {ns_name} owned by {owner_ns_name} with hash {hash}: {lifecycle_data:?}");
        self.index.insert(ns_name.into(), (owner_ns_name.into(), hash, idx));
    }

    pub(super) fn update_pod_lifecycle(&mut self, ns_name: &str, lifecycle_data: &PodLifecycleData) -> EmptyResult {
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

                info!("updating pod {ns_name} owned by {owner_ns_name} with hash {hash}: {lifecycle_data:?}");
                *pod_entry = lifecycle_data.clone();
                Ok(())
            },
        }
    }

    // Given an index of "owning objects", get a list of all the pods between a given start and end
    // time that belong to one of those owning objects.
    pub(super) fn filter(
        &self,
        start_ts: i64,
        end_ts: i64,
        index: &HashMap<String, u64>,
    ) -> HashMap<String, PodLifecyclesMap> {
        self.m
            .iter()
            // The filtering is a little complicated here; if the owning object isn't in the index,
            // we discard it.  Also, if none of the pods belonging to the owning object land
            // within the given time window, we want to discard it.  Otherwise, we want to filter
            // down the list of pods to the ones that fall between the given time window.
            .filter_map(|(owner, lifecycles_map)| {
                if !index.contains_key(owner) {
                    return None;
                }

                // Note the question mark here, doing a bunch of heavy lifting
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
    pub(super) fn pod_owner_meta(&self, ns_name: &str) -> Option<&(String, u64, usize)> {
        self.index.get(ns_name)
    }
}
