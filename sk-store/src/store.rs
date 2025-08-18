use std::collections::HashMap;

use anyhow::bail;
use sk_api::v1::ExportFilters;
use sk_core::jsonutils;
use sk_core::k8s::{
    DynamicApiSet,
    GVK,
    OwnersCache,
    PodExt,
    PodLifecycleData,
};
use sk_core::prelude::*;
use tracing::*;

use crate::config::TracerConfig;
use crate::event::{
    TraceAction,
    TraceEvent,
    append_event,
};
use crate::filter::filter_event;
use crate::index::TraceIndex;
use crate::pod_owners_map::PodOwnersMap;
use crate::trace::ExportedTrace;

pub struct TraceStore {
    pub(crate) config: TracerConfig,
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) pod_owners: PodOwnersMap,
    pub(crate) index: TraceIndex,

    owners_cache: OwnersCache,
}

// The TraceStore object is an in-memory store of a cluster trace.  It keeps track of all the
// configured Kubernetes objects, as well as lifecycle data for any pods that are owned by the
// tracked objects.  It also provides functionality for importing and exporting traces.
//
// Currently, the store just grows indefinitely, so will eventually run out of memory.  At some
// point in the future we plan to implement garbage collection so this isn't a problem.

impl TraceStore {
    pub fn new(config: TracerConfig, apiset: DynamicApiSet) -> TraceStore {
        TraceStore {
            config,
            events: vec![],
            pod_owners: PodOwnersMap::default(),
            index: TraceIndex::default(),

            owners_cache: OwnersCache::new(apiset),
        }
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
        let data = ExportedTrace {
            config: self.config.clone(),
            events,
            index,
            pod_lifecycles,
            ..Default::default()
        }
        .to_bytes()?;

        info!("Exported {} events", num_events);
        Ok(data)
    }

    pub(super) fn collect_events(
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

        for evt in self.events.iter() {
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

    // We use a swap-and-update operation for the index, which means that if we call
    // create_or_update_obj from a refresh event, the _new_ index won't have the hash data
    // available in it yet.  So here we have to pass in a maybe_old_hash which is the value from
    // the swapped-out data structure.  If this is called from an `Applied` event, we just pass in
    // `None` and look up the value in the current index (if the object didn't exist in the old
    // index either, we'll do a second lookup in the new index, but that should be pretty fast).
    pub(super) fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        let gvk = GVK::from_dynamic_obj(obj)?;

        let ns_name = obj.namespaced_name();
        let new_hash = jsonutils::hash_option(obj.data.get("spec"));
        let old_hash = self.index.get(&gvk, &ns_name);

        if Some(new_hash) != old_hash {
            append_event(&mut self.events, ts, obj, TraceAction::ObjectApplied);
        }
        self.index.insert(gvk, ns_name, new_hash);
        Ok(())
    }

    pub(super) fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        let gvk = GVK::from_dynamic_obj(obj)?;
        let ns_name = obj.namespaced_name();
        append_event(&mut self.events, ts, obj, TraceAction::ObjectDeleted);
        self.index.remove(gvk, &ns_name);
        Ok(())
    }

    // We assume that we are given a valid/correct lifecycle event here, so we will just
    // blindly store whatever we are given.  It's up to the caller (the pod watcher in this
    // case) to ensure that the lifecycle data isn't incorrect.
    pub(super) async fn record_pod_lifecycle(
        &mut self,
        ns_name: &str,
        maybe_pod: &Option<corev1::Pod>,
        lifecycle_data: &PodLifecycleData,
    ) -> EmptyResult {
        // If we've already stored data about this pod, we just update the existing entry
        // This assumes that the pod spec is immutable/can't change.  This is _largely_ true in
        // current Kubernetes, but it may not be true in the future with in-place resource updates
        // and so forth.  (We're specifically not including labels and annotations in the hash
        // because those _can_ change).
        if self.pod_owners.has_pod(ns_name) {
            self.pod_owners.update_pod_lifecycle(ns_name, lifecycle_data)?;
        } else if let Some(pod) = maybe_pod {
            // Otherwise, we need to check if any of the pod's owners are tracked by us
            let gvk = corev1::Pod::gvk();
            let owners = match (self.owners_cache.lookup_by_name(&gvk, ns_name), &maybe_pod) {
                (Some(o), _) => o.clone(),
                (None, Some(pod)) => self.owners_cache.compute_owners_for(&gvk, pod).await?,
                _ => bail!("could not determine owner chain for {}", ns_name),
            };
            for owner in owners {
                // Pods are guaranteed to have namespaces, so the unwrap is fine
                let owner_ns_name = format!("{}/{}", pod.namespace().unwrap(), owner.name);
                let owner_gvk = GVK::from_owner_ref(&owner)?;
                if !self.index.contains(&owner_gvk, &owner_ns_name) {
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
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use super::*;

    impl TraceStore {
        pub fn objs_at(&self, end_ts: i64, filter: &ExportFilters) -> Vec<String> {
            // To compute the list of tracked_objects at a particular timestamp, we _don't_ want to
            // keep the deleted objects around, so we set that parameter to `false`.
            let (_, index) = self.collect_events(0, end_ts, filter, false).expect("testing code");
            index.flattened_keys()
        }
    }
}
