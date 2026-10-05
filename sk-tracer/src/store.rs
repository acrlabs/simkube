use std::collections::{
    HashMap,
    HashSet,
};
use std::sync::Arc;

use anyhow::bail;
use kube::Resource;
use prometheus_parse::{
    Sample,
    Value,
};
use sk_api::v1::ExportFilters;
use sk_core::k8s::{
    DynamicApiSet,
    OwnersCache,
    build_pod_self_owner_reference,
};
use sk_core::prelude::*;
use sk_core::trace::{
    MetricType,
    PodMetricsData,
};
use sk_skel::{
    parse_skel_commands,
    process_event,
};
use tokio::sync::Mutex;
use tracing::*;

use crate::owners_index::OwnersIndex;
use crate::util::hash_dynamic_object;

pub struct TraceStore {
    pub(crate) config: TracerConfig,
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) owners_index: OwnersIndex,

    // This is behind an Arc<Mutex<...>> for interior mutability reasons, even though the parent
    // TraceStore is _also_ behind an Arc<Mutex<...>>... Sigh.
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
            owners_index: OwnersIndex::default(),
            owners_cache: Arc::new(Mutex::new(OwnersCache::new(apiset))),
        }
    }

    pub async fn export(
        &self,
        start_ts: i64,
        end_ts: i64,
        filter: &ExportFilters,
        maybe_skel_file: Option<&str>,
    ) -> anyhow::Result<Vec<u8>> {
        info!("Exporting objs between {start_ts} and {end_ts} with filters: {filter:?}");

        // First, we collect all the events in our trace that match our configured filters.  This
        // will return an index of objects that we collected, and we set the keep_deleted flag =
        // true so that in the second step, we keep pod data around even if the owning object was
        // deleted before the trace ends.
        let (events, objects) = self.collect_events(start_ts, end_ts, filter, true, maybe_skel_file).await?;

        // Collect all pod lifecycle data that is a) between the start and end times, and b) is
        // owned by some object contained in the trace
        let index = self.owners_index.aggregate_pod_sim_data(start_ts, end_ts, &objects);
        let data = Trace {
            config: self.config.clone(),
            events,
            index,
            ..Default::default()
        }
        .to_bytes()?;

        Ok(data)
    }

    pub(crate) async fn collect_events(
        &self,
        start_ts: i64,
        end_ts: i64,
        filter: &ExportFilters,
        keep_deleted: bool,
        maybe_skel_str: Option<&str>,
    ) -> anyhow::Result<(Vec<TraceEvent>, HashSet<KubeResourceId>)> {
        // TODO this is not a huge inefficiency but it is a little annoying to have
        // an empty event at the start_ts if there aren't any events that happened
        // before the start_ts
        let mut events = vec![TraceEvent { ts: start_ts, ..Default::default() }];

        // flattened_objects is a list of everything that happened before start_ts but is
        // still present at start_ts -- i.e., it is our starting configuration.
        let mut flattened_objects = HashMap::new();
        let parsed_commands =
            if let Some(skel_str) = maybe_skel_str { parse_skel_commands(skel_str, start_ts)? } else { vec![] };
        let mut all_objects = HashSet::new();

        for evt in self.events.iter() {
            // trace should be end-exclusive, so we use >= here: anything that is at the
            // end_ts or greater gets discarded.  The event list is stored in
            // monotonically-increasing order so we are safe to break here.
            if evt.ts >= end_ts {
                break;
            }

            // process event with skel commands
            let transformed_evt;
            let evt = if !parsed_commands.is_empty() {
                let mut current = evt.clone();
                for cmd in &parsed_commands {
                    current = process_event(cmd, current)?;
                }
                transformed_evt = current;
                &transformed_evt
            } else {
                evt
            };

            let mut filtered_applied_objs = vec![];
            let mut filtered_deleted_objs = vec![];

            for obj in &evt.applied_objs {
                let resource_id = obj.resource_id();

                if object_matches_filter(obj, filter)
                    || self.is_owned_by_tracked_object(&resource_id, obj, &all_objects).await?
                {
                    debug!("applied obj {resource_id} filtered out");
                    continue;
                }

                if evt.ts < start_ts && !self.obj_is_finished_pod(obj, start_ts)? {
                    flattened_objects.insert(resource_id.ns_name.clone(), obj.clone());
                } else {
                    filtered_applied_objs.push(obj.clone());
                }
                all_objects.insert(resource_id);
            }

            for obj in &evt.deleted_objs {
                let resource_id = obj.resource_id();

                if object_matches_filter(obj, filter)
                    || self.is_owned_by_tracked_object(&resource_id, obj, &all_objects).await?
                {
                    debug!("deleted obj {resource_id} filtered out");
                    continue;
                }

                if evt.ts < start_ts {
                    flattened_objects.remove(&resource_id.ns_name);
                } else {
                    filtered_deleted_objs.push(obj.clone());
                }

                if !keep_deleted {
                    all_objects.remove(&resource_id);
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
        Ok((events, all_objects))
    }

    pub(super) fn create_or_update_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        let resource_id = obj.resource_id();
        if self.config.skip_owned_for(&resource_id.gvk) && !obj.owner_references().is_empty() {
            return Ok(());
        }

        let new_hash = hash_dynamic_object(obj);
        let old_hash = self.owners_index.get_hash(&resource_id);

        if Some(new_hash) != old_hash {
            append_event(&mut self.events, ts, obj, TraceAction::ObjectApplied);
            self.owners_index.store_object(resource_id, new_hash, ts);
        }
        Ok(())
    }

    pub(super) fn delete_obj(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        // We don't check for skip_owned here, in principle if the object made it past the
        // insertion check, it won't have magically received an ownerref in the interim.  And even
        // if it somehow magically did, I think maybe we still want to delete it?
        let resource_id = obj.resource_id();
        append_event(&mut self.events, ts, obj, TraceAction::ObjectDeleted);
        self.owners_index.remove_object(&resource_id);
        Ok(())
    }

    pub(super) async fn store_pod_owners(&mut self, maybe_pod: &Option<corev1::Pod>) -> Vec<metav1::OwnerReference> {
        let Some(pod) = maybe_pod else {
            return vec![];
        };

        // TODO (SK-254) we may still want to do this if the pod is owned but we are choosing
        // to not track the owner for whatever reason
        if pod.owner_references().is_empty() {
            // If we have a bare pod, then we make the pod its own owner, which is a
            // little weird and not, like, technically correct, but will work fine for our
            // purposes; the bare pods are tracked in the index, so this will pass all the
            // checks below.
            vec![build_pod_self_owner_reference(pod.name_any())]
        } else {
            // If it's not a bare pod, then we look up the owners in the cache.
            self.owners_cache
                .lock()
                .await
                .lookup_by_name_or_obj(&pod.resource_id(), Some(pod))
                .await
        }
    }

    // We assume that we are given a valid/correct lifecycle event here, so we will just
    // blindly store whatever we are given.  It's up to the caller (the pod watcher in this
    // case) to ensure that the lifecycle data isn't incorrect.
    pub(super) async fn record_pod_lifecycle(
        &mut self,
        pod_ns_name: &str,
        owners: Vec<metav1::OwnerReference>,
        lifecycle_data: PodLifecycleData,
    ) -> EmptyResult {
        // If we've already stored data about this pod, we just update the existing entry
        // This assumes that the pod spec is immutable/can't change.  This is _largely_ true in
        // current Kubernetes, but it may not be true in the future with in-place resource updates
        // and so forth.  (We're specifically not including labels and annotations in the hash
        // because those _can_ change).
        if self.owners_index.has_pod(pod_ns_name) {
            self.owners_index.update_pod_lifecycle(pod_ns_name, lifecycle_data)?;
            return Ok(());
        }

        // Pods are guaranteed to have namespaces, so the unwrap is fine
        let (pod_namespace, _) = pod_ns_name.split_once("/").unwrap();
        for owner in owners {
            let owner_id = KubeResourceId::from_owner_ref(&owner, pod_namespace.into())?;
            // We don't look up if the owners_index has the specific owner_id here, because it may
            // not have gotten populated by a different async task yet.  If the owner_id doesn't
            // exist in the index here, the `store_new_pod_lifecycle` call below will create it.

            if !self.config.track_lifecycle_for(&owner_id.gvk) {
                continue;
            }

            self.owners_index
                .store_new_pod_lifecycle(pod_ns_name, &owner_id, lifecycle_data)?;
            break;
        }

        Ok(())
    }

    pub(super) async fn record_pod_utilization_metrics(&mut self, samples: Vec<Sample>) -> EmptyResult {
        let mut metrics: HashMap<(KubeResourceId, String), PodMetricsData> = HashMap::new();
        for sample in samples {
            // If the metric doesn't have a pod name and namespace, we're not interested
            let Some(pod_name) = sample.labels.get("pod") else {
                continue;
            };
            let Some(pod_namespace) = sample.labels.get("namespace") else {
                continue;
            };
            let container = sample.labels.get("container").unwrap_or_default();

            let pod_ns_name = format!("{pod_namespace}/{pod_name}");
            let pod_id = KubeResourceId::new(POD_GVK.clone(), pod_ns_name.clone());
            let owners = {
                // Drop the lock after this lookup
                self.owners_cache
                    .lock()
                    .await
                    .lookup_by_name_or_obj(&pod_id, None::<&corev1::Pod>) // typed none makes rustc happy
                    .await
            };

            // If no pod owner is present in the owner's cache, we will not record a metric for this
            // data point.  We do not have access to any ownership data _here_ because that's not
            // included in the metric labels, and the only way to _get_ that access otherwise would
            // be through an apiserver query to get the pods.  This could potentially lead to a lot
            // of apiserver requests at tracer startup, which is doubly stupid because we're _also_
            // establishing a pod watch as a separate task.
            //
            // So the implications here are "we might drop a few datapoints while either the tracer
            // is starting up, or (possibly) when a pod first appears on a node", but at least in
            // the short term that seems like a better outcome than hammering the k8s apiserver.
            for owner in owners {
                let owner_id = KubeResourceId::from_owner_ref(&owner, pod_namespace.into())?;
                if !self.config.track_utilization_for(&owner_id.gvk) {
                    continue;
                }

                let metric_type = match sample.metric.as_str() {
                    CONTAINER_CPU_USAGE_SECONDS_TOTAL => MetricType::CPU,
                    CONTAINER_MEMORY_WORKING_SET_BYTES => MetricType::Memory,
                    m => bail!("invalid metric type {m}"),
                };
                let metric_value = match sample.value {
                    Value::Counter(v) => v,
                    Value::Gauge(v) => v,
                    Value::Untyped(v) => v, // probably unnecessary but why not
                    _ => bail!("unsupported metric type: {sample:?}"),
                };

                metrics
                    .entry((owner_id, pod_ns_name))
                    .or_default()
                    .insert(container, metric_type, metric_value);
                break;
            }
        }
        self.owners_index.store_new_utilization_metrics(metrics)
    }

    async fn is_owned_by_tracked_object(
        &self,
        resource_id: &KubeResourceId,
        obj: &(impl Resource + SkResourceExt + Sync),
        // We specifically DO NOT use self.index here, because the index at time t_n
        // probably has ~little relation to whatever the index looked like at the
        // time we're performing the export.
        owning_objects: &HashSet<KubeResourceId>,
    ) -> anyhow::Result<bool> {
        // If any of the owners of this object are exported, we don't want to also
        // export this object; in the simulation replay, it would result in duplicate
        // objects being created
        let owners = self
            .owners_cache
            .lock()
            .await
            .lookup_by_name_or_obj(resource_id, Some(obj))
            .await;

        for owner in owners {
            // TODO right now we only look up _namespaced_ owners, not cluster-scoped; in
            // principle, it's possible to get the cluster-scoped owners, since the owner
            // cache knows what they are, but passing that information back up to us is
            // sortof annoying and I don't want to bother right now.
            let owner_id = KubeResourceId::from_owner_ref(&owner, obj.namespace().unwrap())?;
            if owning_objects.contains(&owner_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn obj_is_finished_pod(&self, obj: &DynamicObject, start_ts: i64) -> anyhow::Result<bool> {
        Ok(
            if GVK::from_dynamic_obj(obj)? == *POD_GVK
                && let lifecycles = self.owners_index.get_pod_lifecycles(&obj.namespaced_name())
                // if it's a bare pod, there "should" only be one recorded lifecycle
                && let Some(PodLifecycleData::Finished(_, finish)) = lifecycles.first()
                && *finish < start_ts
            {
                true
            } else {
                false
            },
        )
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
    use std::collections::HashSet;

    use super::*;

    impl TraceStore {
        // This is really stupid to have async, it's a consequence of collect_events now
        // querying ownership information.... probably should fix this at some point
        pub async fn objs_at(&self, end_ts: i64, filter: &ExportFilters) -> HashSet<KubeResourceId> {
            // To compute the list of tracked_objects at a particular timestamp, we _don't_ want to
            // keep the deleted objects around, so we set that parameter to `false`.
            let (_, objs) = self.collect_events(0, end_ts, filter, false, None).await.unwrap();
            objs
        }
    }
}
