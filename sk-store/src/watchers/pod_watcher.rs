use std::collections::HashMap;
use std::mem::take;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use kube::runtime::watcher::watcher;
use sk_core::errors::*;
use sk_core::k8s::{ApiSet, OwnersCache, PodLifecycleData};
use sk_core::prelude::*;
use tracing::*;

use crate::watchers::{EventHandler, ObjStream};
use crate::TraceStorable;

// The PodWatcher object monitors incoming pod events and records the relevant ones to the object
// store; becaues in clusters of any reasonable size, there are a) a lot of pods, and b) a lot of
// update events for each pod, the pod watcher does a bunch of caching and pre-filtering before
// sending events on to the object store.  Combined with the fact that figuring out which events we
// care about is a little non-trivial, means that the logic in here is a bit complicated.
//
// At a high level: whenever a pod event happens, we check to see whether any properties of its
// lifecycle data (currently start time and end time) have changed.  If so, we compute the
// ownership chain for the pod, and forward that info on to the store.

pub struct PodHandler {
    // We store the list of owned pods in memory here, and cache the ownership chain for each pod;
    // This is a simpler data structure than what the object store needs, to allow for easy lookup
    // by pod name.  (The object store needs to store a bunch of extra metadata about sequence
    // number and pod hash and so forth).
    owned_pods: HashMap<String, PodLifecycleData>,
    owners_cache: OwnersCache,
}

impl PodHandler {
    // We take ownership of the apiset here, meaning that this has to be created after the
    // DynamicObject watcher.  We need ownership because pod owner chains could contain arbitrary
    // object types (we don't necessarily know what they are until we see the pod).  The
    // DynamicObject watcher just needs to construct the relevant api clients once, when it creates
    // the watch streams, so it can yield when it's done.  If at some point in the future this
    // becomes problematic, we can always stick the apiset in an Arc<Mutex<_>>.
    pub fn new_with_stream(client: kube::Client, apiset: ApiSet) -> (Box<PodHandler>, ObjStream<corev1::Pod>) {
        let pod_api: kube::Api<corev1::Pod> = kube::Api::all(client);
        (
            Box::new(PodHandler {
                owned_pods: HashMap::new(),
                owners_cache: OwnersCache::new(apiset),
            }),
            watcher(pod_api, Default::default()).map_err(|e| e.into()).boxed(),
        )
    }

    async fn handle_pod_applied(
        &mut self,
        ns_name: &str,
        pod: &corev1::Pod,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let new_lifecycle_data = PodLifecycleData::new_for(pod)?;
        let current_lifecycle_data = self.owned_pods.get(ns_name);

        // We only store data if the lifecycle data has changed; there is some magic happening in
        // the > operator here; see pod_lifecycle.rs for details, but in short, we only allow
        // lifecycle updates if the timestamps match and we've moved from one state to the next.
        //
        // Note that we only store non-empty lifecycle data, which is enforced since
        // PodLifecycleData::Empty < everything.
        if new_lifecycle_data > current_lifecycle_data {
            self.owned_pods.insert(ns_name.into(), new_lifecycle_data.clone());
            self.store_pod_lifecycle_data(ns_name, Some(pod), &new_lifecycle_data, store)
                .await?;
        } else if !new_lifecycle_data.empty() && new_lifecycle_data != current_lifecycle_data {
            warn!(
                "new lifecycle data for {} does not match stored data, cowardly refusing to update: {:?} !>= {:?}",
                ns_name, new_lifecycle_data, current_lifecycle_data
            );
        }

        Ok(())
    }

    // handle_pod_deleted takes a maybe_pod because on a watch stream refresh event, we only get
    // the list of pods that exist in between the last call and the refresh.  Anything that is
    // missing was deleted during the intervening time period, but we don't know any data about it.
    async fn handle_pod_deleted(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        current_lifecycle_data: PodLifecycleData,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
        ts: i64,
    ) -> EmptyResult {
        // Always remove the pod from our tracker, regardless of what else happens
        self.owned_pods.remove(ns_name);

        // If the current lifecycle data is finished, we know it's already been written to the
        // store so we don't store it a second time.
        if current_lifecycle_data.finished() {
            return Ok(());
        }

        // TODO: should this logic be somehow combined with the logic in
        // PodLifecycleData::guess_finished_lifecycle?
        let new_lifecycle_data = match maybe_pod {
            None => {
                // We never store "empty" data so this unwrap should always succeed
                let start_ts = current_lifecycle_data.start_ts().unwrap();
                PodLifecycleData::Finished(start_ts, ts)
            },
            Some(pod) => PodLifecycleData::guess_finished_lifecycle(pod, &current_lifecycle_data, ts)?,
        };

        self.store_pod_lifecycle_data(ns_name, maybe_pod, &new_lifecycle_data, store)
            .await
    }

    async fn store_pod_lifecycle_data(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        lifecycle_data: &PodLifecycleData,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        // Before storing the lifecycle data, we need to compute the ownership chain so the store
        // can determine if this pod is owned by anything it's tracking.  We do this _after_ we've
        // determined that we want to store the data but _before_ we unlock the object store, since
        // this involves a bunch of API calls, and we don't want to block either thread.
        let owners = match (self.owners_cache.lookup(ns_name), maybe_pod) {
            (Some(o), _) => o.clone(),
            (None, Some(pod)) => self.owners_cache.compute_owner_chain(pod).await?,
            _ => bail!("could not determine owner chain for {}", ns_name),
        };

        let mut s = store.lock().expect("trace store mutex poisoned");
        s.record_pod_lifecycle(ns_name, maybe_pod.cloned(), owners, lifecycle_data)
    }
}

#[async_trait]
impl EventHandler<corev1::Pod> for PodHandler {
    async fn applied(
        &mut self,
        pod: &corev1::Pod,
        _ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let ns_name = pod.namespaced_name();
        self.handle_pod_applied(&ns_name, pod, store).await
    }

    async fn deleted(
        &mut self,
        pod: &corev1::Pod,
        ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let ns_name = pod.namespaced_name();
        let Some(current_lifecycle_data) = self.owned_pods.get(&ns_name) else {
            warn!("pod {ns_name} deleted but not tracked, may have already been processed");
            return Ok(());
        };
        self.handle_pod_deleted(&ns_name, Some(pod), current_lifecycle_data.clone(), store, ts)
            .await
    }

    async fn initialized(
        &mut self,
        pods: &[corev1::Pod],
        ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        // We're essentially swapping the old data structure for the new one, and removing
        // events from the old and putting them into the new.  Then we know that anything
        // left in the old after we're done was deleted in the intervening period.  This
        // lets us not have to track "object versions" or use a bit vector or something
        // along those lines.
        let mut old_owned_pods = take(&mut self.owned_pods);
        for pod in pods {
            let ns_name = &pod.namespaced_name();
            if let Some(current_lifecycle_data) = old_owned_pods.remove(ns_name) {
                self.owned_pods.insert(ns_name.into(), current_lifecycle_data);
            }
            if let Err(err) = self.handle_pod_applied(ns_name, pod, store.clone()).await {
                skerr!(err, "(watcher restart) applied pod {} lifecycle data could not be stored", ns_name);
            }
        }

        for (ns_name, current_lifecycle_data) in &old_owned_pods {
            // We don't have data on the deleted pods aside from the name, so we just pass
            // in `None` for the pod object.
            if let Err(err) = self
                .handle_pod_deleted(ns_name, None, current_lifecycle_data.clone(), store.clone(), ts)
                .await
            {
                skerr!(err, "(watcher restart) deleted pod {} lifecycle data could not be stored", ns_name);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl PodHandler {
    pub(crate) fn new_from_parts(
        owned_pods: HashMap<String, PodLifecycleData>,
        owners_cache: OwnersCache,
    ) -> PodHandler {
        PodHandler { owned_pods, owners_cache }
    }

    pub(crate) fn get_owned_pod_lifecycle(&self, ns_name: &str) -> Option<&PodLifecycleData> {
        self.owned_pods.get(ns_name)
    }
}
