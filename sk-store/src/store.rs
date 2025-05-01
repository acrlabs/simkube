use std::collections::HashMap;

use anyhow::bail;
use clockabilly::prelude::*;
use sk_api::v1::ExportFilters;
use sk_core::jsonutils;
use sk_core::k8s::{
    build_deletable,
    PodExt,
    PodLifecycleData,
    GVK,
};
use sk_core::prelude::*;
use sk_core::time::duration_to_ts_from;
use thiserror::Error;
use tracing::*;

use crate::config::TracerConfig;
use crate::filter::filter_event;
use crate::pod_owners_map::PodOwnersMap;
use crate::{
    ExportedTrace,
    TraceAction,
    TraceEvent,
    TraceEventList,
    TraceIndex,
    TraceIterator,
    TraceStorable,
    CURRENT_TRACE_FORMAT_VERSION,
};


#[derive(Debug, Error)]
pub enum TraceStoreError {
    #[error(
        "could not parse trace file\n\nIf this trace file is older than version 2, \
        it is only parseable by SimKube <= 1.1.1.  Please see the release notes for details."
    )]
    ParseFailed(#[from] rmp_serde::decode::Error),
}

#[derive(Clone, Default)]
pub struct TraceStore {
    pub(crate) config: TracerConfig,
    pub(crate) events: TraceEventList,
    pub(crate) pod_owners: PodOwnersMap,
    pub(crate) index: TraceIndex,
}

// The TraceStore object is an in-memory store of a cluster trace.  It keeps track of all the
// configured Kubernetes objects, as well as lifecycle data for any pods that are owned by the
// tracked objects.  It also provides functionality for importing and exporting traces.
//
// Currently, the store just grows indefinitely, so will eventually run out of memory.  At some
// point in the future we plan to implement garbage collection so this isn't a problem.

impl TraceStore {
    pub fn new(config: TracerConfig) -> TraceStore {
        TraceStore { config, ..Default::default() }
    }

    pub fn clone_with_events(&self, events: TraceEventList) -> TraceStore {
        let mut store = self.clone();
        store.events = events;
        store
    }

    pub fn export(&self, start_ts: i64, end_ts: i64, filter: &ExportFilters) -> anyhow::Result<Vec<u8>> {
        info!("Exporting objs between {start_ts} and {end_ts} with filters: {filter:?}");

        // First, we collect all the events in our trace that match our configured filters.  This
        // will return an index of objects that we collected, and we set the keep_deleted flag =
        // true so that in the second step, we keep pod data around even if the owning object was
        // deleted before the trace ends.
        let (events, index) = self.collect_events(start_ts, end_ts, filter, true)?;
        let num_events = events.len();

        // Collect all pod lifecycle data that is a) between the start and end times, and b) is
        // owned by some object contained in the trace
        let pod_lifecycles = self.pod_owners.filter(start_ts, end_ts, &index);
        let data = rmp_serde::to_vec_named(&ExportedTrace {
            version: CURRENT_TRACE_FORMAT_VERSION,
            config: self.config.clone(),
            events,
            index,
            pod_lifecycles,
        })?;

        info!("Exported {} events", num_events);
        Ok(data)
    }

    pub fn export_all(&self) -> anyhow::Result<Vec<u8>> {
        let (Some(start_ts), Some(end_ts)) = (self.start_ts(), self.end_ts()) else {
            return Ok(vec![]);
        };
        self.export(start_ts, end_ts, &ExportFilters::default())
    }

    pub fn import(data: Vec<u8>, maybe_duration: &Option<String>) -> anyhow::Result<TraceStore> {
        let exported_trace = rmp_serde::from_slice::<ExportedTrace>(&data).map_err(TraceStoreError::ParseFailed)?;
        Self::from_exported_trace(exported_trace, maybe_duration)
    }

    // Note that _importing_ data into a trace store is lossy -- we don't store (or import) all of
    // the metadata necessary to pick up a trace and continue.  Instead, we just re-import enough
    // information to be able to run a simulation off the trace store.
    pub fn from_exported_trace(
        mut exported_trace: ExportedTrace,
        maybe_duration: &Option<String>,
    ) -> anyhow::Result<TraceStore> {
        if exported_trace.version != CURRENT_TRACE_FORMAT_VERSION {
            bail!("unsupported trace version: {}", exported_trace.version);
        }

        let trace_start_ts = exported_trace
            .events
            .first()
            .unwrap_or(&TraceEvent { ts: UtcClock.now_ts(), ..Default::default() })
            .ts;
        let mut trace_end_ts = exported_trace
            .events
            .last()
            .unwrap_or(&TraceEvent { ts: UtcClock.now_ts(), ..Default::default() })
            .ts;
        if let Some(trace_duration_str) = maybe_duration {
            trace_end_ts = duration_to_ts_from(trace_start_ts, trace_duration_str)?;
            exported_trace.events.retain(|evt| evt.ts < trace_end_ts);

            // Add an empty event to the very end to make sure the driver doesn't shut down early
            exported_trace
                .events
                .push(TraceEvent { ts: trace_end_ts, ..Default::default() });
        }

        info!("Imported {} events between {trace_start_ts} and {trace_end_ts}", exported_trace.events.len());
        Ok(TraceStore {
            config: exported_trace.config,
            events: exported_trace.events.into(),
            index: exported_trace.index,
            pod_owners: PodOwnersMap::new_from_parts(exported_trace.pod_lifecycles, HashMap::new()),
        })
    }

    pub(crate) fn collect_events(
        &self,
        start_ts: i64,
        end_ts: i64,
        filter: &ExportFilters,
        keep_deleted: bool,
    ) -> anyhow::Result<(Vec<TraceEvent>, TraceIndex)> {
        // TODO this is not a huge inefficiency but it is a little annoying to have
        // an empty event at the start_ts if there aren't any events that happened
        // before the start_ts
        let mut events = vec![TraceEvent { ts: start_ts, ..Default::default() }];

        // flattened_objects is a list of everything that happened before start_ts but is
        // still present at start_ts -- i.e., it is our starting configuration.
        let mut flattened_objects = HashMap::new();
        let mut index = TraceIndex::new();

        for (evt, _) in self.iter() {
            // trace should be end-exclusive, so we use >= here: anything that is at the
            // end_ts or greater gets discarded.  The event list is stored in
            // monotonically-increasing order so we are safe to break here.
            if evt.ts >= end_ts {
                break;
            }

            if let Some(new_evt) = filter_event(evt, filter) {
                for obj in &new_evt.applied_objs {
                    let gvk = GVK::from_dynamic_obj(obj)?;
                    let ns_name = obj.namespaced_name();
                    if new_evt.ts < start_ts {
                        flattened_objects.insert(ns_name.clone(), obj.clone());
                    }
                    let hash = jsonutils::hash_option(obj.data.get("spec"));
                    index.insert(gvk, ns_name, hash);
                }

                for obj in &evt.deleted_objs {
                    let gvk = GVK::from_dynamic_obj(obj)?;
                    let ns_name = obj.namespaced_name();
                    if new_evt.ts < start_ts {
                        flattened_objects.remove(&ns_name);
                    }
                    if !keep_deleted {
                        index.remove(gvk, &ns_name);
                    }
                }

                if new_evt.ts >= start_ts {
                    events.push(new_evt.clone());
                }
            }
        }

        // events[0] is the empty event we inserted at the beginning, so we're guaranteed not to
        // overwrite anything here.
        events[0].applied_objs = flattened_objects.into_values().collect();
        Ok((events, index))
    }
}

impl TraceStorable for TraceStore {
    // We use a swap-and-update operation for the index, which means that if we call
    // create_or_update_obj from a refresh event, the _new_ index won't have the hash data
    // available in it yet.  So here we have to pass in a maybe_old_hash which is the value from
    // the swapped-out data structure.  If this is called from an `Applied` event, we just pass in
    // `None` and look up the value in the current index (if the object didn't exist in the old
    // index either, we'll do a second lookup in the new index, but that should be pretty fast).
    fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64, maybe_old_hash: Option<u64>) -> EmptyResult {
        let gvk = GVK::from_dynamic_obj(obj)?;

        let ns_name = obj.namespaced_name();
        let new_hash = jsonutils::hash_option(obj.data.get("spec"));
        let old_hash = maybe_old_hash.or_else(|| self.index.get(&gvk, &ns_name));

        if Some(new_hash) != old_hash {
            self.events.append(ts, obj, TraceAction::ObjectApplied);
        }
        self.index.insert(gvk, ns_name, new_hash);
        Ok(())
    }

    fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        let gvk = GVK::from_dynamic_obj(obj)?;
        let ns_name = obj.namespaced_name();
        self.events.append(ts, obj, TraceAction::ObjectDeleted);
        self.index.remove(gvk, &ns_name);
        Ok(())
    }

    fn update_all_objs_for_gvk(&mut self, gvk: &GVK, objs: &[DynamicObject], ts: i64) -> EmptyResult {
        let mut old_gvk_index = self.index.take_gvk_index(gvk);
        for obj in objs {
            let ns_name = obj.namespaced_name();
            let old_hash = old_gvk_index.remove(&ns_name);
            self.create_or_update_obj(obj, ts, old_hash)?;
        }

        for ns_name in old_gvk_index.keys() {
            self.delete_obj(&build_deletable(gvk, ns_name), ts)?;
        }
        Ok(())
    }

    fn lookup_pod_lifecycle(
        &self,
        owner_gvk: &GVK,
        owner_ns_name: &str,
        pod_hash: u64,
        seq: usize,
    ) -> PodLifecycleData {
        let maybe_lifecycle_data = self.pod_owners.lifecycle_data_for(owner_gvk, owner_ns_name, pod_hash);
        match maybe_lifecycle_data {
            Some(data) => data[seq % data.len()].clone(),
            _ => PodLifecycleData::Empty,
        }
    }

    // We assume that we are given a valid/correct lifecycle event here, so we will just
    // blindly store whatever we are given.  It's up to the caller (the pod watcher in this
    // case) to ensure that the lifecycle data isn't incorrect.
    fn record_pod_lifecycle(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<corev1::Pod>,
        owners: Vec<metav1::OwnerReference>,
        lifecycle_data: &PodLifecycleData,
    ) -> EmptyResult {
        // If we've already stored data about this pod, we just update the existing entry
        // This assumes that the pod spec is immutable/can't change.  This is _largely_ true in
        // current Kubernetes, but it may not be true in the future with in-place resource updates
        // and so forth.  (We're specifically not including labels and annotations in the hash
        // because those _can_ change).
        if self.pod_owners.has_pod(ns_name) {
            self.pod_owners.update_pod_lifecycle(ns_name, lifecycle_data)?;
        } else if let Some(pod) = &maybe_pod {
            // Otherwise, we need to check if any of the pod's owners are tracked by us
            for owner in &owners {
                // Pods are guaranteed to have namespaces, so the unwrap is fine
                let owner_ns_name = format!("{}/{}", pod.namespace().unwrap(), owner.name);
                let owner_gvk = GVK::from_owner_ref(owner)?;
                if !self.has_obj(&owner_gvk, &owner_ns_name) {
                    continue;
                }

                if !self.config.track_lifecycle_for(&owner_gvk) {
                    continue;
                }

                // We compute a hash of the podspec, because some types of owning objects may have
                // multiple different types of running pods, and we want to track the lifecycle
                // data for these separately.  (For example, a volcanojob takes in a list of pod
                // templates that each have their own replica counts)
                //
                // TODO - it's possible that hashing _everything_ may be too much.  Are there types
                // of data that are unique to each pod that won't materially impact the behaviour?
                // This does occur for example with coredns's volume mounts.  We may need to filter
                // more things out from this and/or allow users to specify what is filtered out.
                let hash = jsonutils::hash(&serde_json::to_value(pod.stable_spec()?)?);
                self.pod_owners
                    .store_new_pod_lifecycle(ns_name, &owner_gvk, &owner_ns_name, hash, lifecycle_data);
                break;
            }
        } else {
            bail!("no pod ownership data found for {}, cannot store", ns_name);
        }

        Ok(())
    }

    fn config(&self) -> &TracerConfig {
        &self.config
    }

    fn has_obj(&self, gvk: &GVK, ns_name: &str) -> bool {
        self.index.contains(gvk, ns_name)
    }

    fn start_ts(&self) -> Option<i64> {
        self.events.front().map(|evt| evt.ts)
    }

    fn end_ts(&self) -> Option<i64> {
        self.events.back().map(|evt| evt.ts)
    }

    fn iter(&self) -> TraceIterator<'_> {
        TraceIterator { events: &self.events, idx: 0 }
    }
}

// Our iterator implementation iterates over all the events in timeseries order.  It returns the
// current event, and the timestamp of the _next_ event.
impl<'a> Iterator for TraceIterator<'a> {
    type Item = (&'a TraceEvent, Option<i64>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.events.is_empty() {
            return None;
        }

        let ret = match self.idx {
            i if i < self.events.len() - 1 => Some((&self.events[i], Some(self.events[i + 1].ts))),
            i if i == self.events.len() - 1 => Some((&self.events[i], None)),
            _ => None,
        };

        self.idx += 1;
        ret
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use super::*;

    impl TraceStore {
        pub fn sorted_objs_at(&self, end_ts: i64, filter: &ExportFilters) -> Vec<String> {
            // To compute the list of tracked_objects at a particular timestamp, we _don't_ want to
            // keep the deleted objects around, so we set that parameter to `false`.
            let (_, index) = self.collect_events(0, end_ts, filter, false).expect("testing code");
            let mut res = index.flattened_keys();
            res.sort();
            res
        }
    }
}
