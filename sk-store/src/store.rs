use std::collections::HashMap;
use std::sync::Arc;

use anyhow::bail;
use kube::Resource;
use sk_api::v1::ExportFilters;
use sk_core::jsonutils;
use sk_core::k8s::{
    DynamicApiSet,
    GVK,
    OwnersCache,
    PodExt,
    PodLifecycleData,
    format_gvk_name,
};
use sk_core::prelude::*;
use tokio::sync::Mutex;
use tracing::*;

use crate::config::TracerConfig;
use crate::event::{
    TraceAction,
    TraceEvent,
    append_event,
};
use crate::index::TraceIndex;
use crate::pod_owners_map::PodOwnersMap;
use crate::trace::ExportedTrace;

pub struct TraceStore {
    pub(crate) config: TracerConfig,
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) pod_owners: PodOwnersMap,
    pub(crate) index: TraceIndex,

    owners_cache: Arc<Mutex<OwnersCache>>,
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

            owners_cache: Arc::new(Mutex::new(OwnersCache::new(apiset))),
        }
    }

    pub async fn export(&self, start_ts: i64, end_ts: i64, filter: &ExportFilters) -> anyhow::Result<Vec<u8>> {
        info!("Exporting objs between {start_ts} and {end_ts} with filters: {filter:?}");

        // First, we collect all the events in our trace that match our configured filters.  This
        // will return an index of objects that we collected, and we set the keep_deleted flag =
        // true so that in the second step, we keep pod data around even if the owning object was
        // deleted before the trace ends.
        let (events, index) = self.collect_events(start_ts, end_ts, filter, true).await?;
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

    pub(super) async fn collect_events(
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

            let mut filtered_applied_objs = vec![];
            let mut filtered_deleted_objs = vec![];

            for obj in &evt.applied_objs {
                let gvk = GVK::from_dynamic_obj(obj)?;
                let ns_name = obj.namespaced_name();

                if object_matches_filter(obj, filter)
                    || self.is_owned_by_tracked_object(&gvk, &ns_name, obj, &index).await?
                {
                    debug!("applied obj {} filtered out", format_gvk_name(&gvk, &ns_name));
                    continue;
                }

                if evt.ts < start_ts {
                    flattened_objects.insert(ns_name.clone(), obj.clone());
                } else {
                    filtered_applied_objs.push(obj.clone());
                }
                let hash = jsonutils::hash_option(obj.data.get("spec"));
                index.insert(gvk, ns_name, hash);
            }

            for obj in &evt.deleted_objs {
                let gvk = GVK::from_dynamic_obj(obj)?;
                let ns_name = obj.namespaced_name();

                if object_matches_filter(obj, filter)
                    || self.is_owned_by_tracked_object(&gvk, &ns_name, obj, &index).await?
                {
                    debug!("deleted obj {} filtered out", format_gvk_name(&gvk, &ns_name));
                    continue;
                }

                if evt.ts < start_ts {
                    flattened_objects.remove(&ns_name);
                } else {
                    filtered_deleted_objs.push(obj.clone());
                }

                if !keep_deleted {
                    index.remove(gvk, &ns_name);
                }
            }

            // We can't filter on evt.ts >= start_ts earlier because we need to
            // track all of the objects that existed before start_ts; the second
            // boolean condition ensures that only non-empty events are added to the
            // exported trace (either objects applied or deleted).
            if evt.ts >= start_ts && !(filtered_applied_objs.is_empty() && filtered_deleted_objs.is_empty()) {
                events.push(TraceEvent {
                    ts: evt.ts,
                    applied_objs: filtered_applied_objs,
                    deleted_objs: filtered_deleted_objs,
                });
            }
        }

        // events[0] is the empty event we inserted at the beginning, so we're guaranteed not to
        // overwrite anything here.
        events[0].applied_objs = flattened_objects.into_values().collect();
        Ok((events, index))
    }

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
            let owners = self
                .owners_cache
                .lock()
                .await
                .lookup_by_name_or_obj(&gvk, ns_name, maybe_pod.as_ref())
                .await?;
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

    async fn is_owned_by_tracked_object(
        &self,
        gvk: &GVK,
        ns_name: &str,
        obj: &(impl Resource + Sync),
        // We specifically DO NOT use self.index here, because the index at time t_n
        // probably has ~little relation to whatever the index looked like at the
        // time we're performing the export.
        index: &TraceIndex,
    ) -> anyhow::Result<bool> {
        // If any of the owners of this object are exported, we don't want to also
        // export this object; in the simulation replay, it would result in duplicate
        // objects being created
        let owners = self
            .owners_cache
            .lock()
            .await
            .lookup_by_name_or_obj(gvk, ns_name, Some(obj))
            .await?;
        for owner in owners {
            // TODO right now we only look up _namespaced_ owners, not cluster-scoped; in
            // principle, it's possible to get the cluster-scoped owners, since the owner
            // cache knows what they are, but passing that information back up to us is
            // sortof annoying and I don't want to bother right now.
            let owner_ns_name = format!("{}/{}", obj.namespace().unwrap(), owner.name);
            let owner_gvk = GVK::from_owner_ref(&owner)?;
            if index.contains(&owner_gvk, &owner_ns_name) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn object_matches_filter(obj: &DynamicObject, f: &ExportFilters) -> bool {
    obj.metadata
        .namespace
        .as_ref()
        .is_some_and(|ns| f.excluded_namespaces.contains(ns))
        || obj
            .metadata
            .owner_references
            .as_ref()
            .is_some_and(|owners| owners.iter().any(|owner| &owner.kind == "DaemonSet"))
        // TODO: maybe don't call unwrap here?  Right now we panic if the user specifies
        // an invalid label selector.  Or, maybe it doesn't matter once we write the CLI
        // tool.
        || f.excluded_labels.iter().any(|sel| obj.matches(sel).unwrap())
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use super::*;

    impl TraceStore {
        // This is really stupid to have async, it's a consequence of collect_events now
        // querying ownership information.... probably should fix this at some point
        pub async fn objs_at(&self, end_ts: i64, filter: &ExportFilters) -> Vec<String> {
            // To compute the list of tracked_objects at a particular timestamp, we _don't_ want to
            // keep the deleted objects around, so we set that parameter to `false`.
            let (_, index) = self.collect_events(0, end_ts, filter, false).await.expect("testing code");
            index.flattened_keys()
        }
    }
}
