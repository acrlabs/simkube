use std::borrow::Borrow;
use std::collections::HashMap;
use std::mem::take;
use std::sync::{
    Arc,
    Mutex,
};

use async_recursion::async_recursion;
use cached::{
    Cached,
    SizedCache,
};
use futures::stream::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::watcher::{
    watcher,
    Event,
};
use kube::{
    Resource,
    ResourceExt,
};
use tracing::*;

use super::*;
use crate::errors::*;
use crate::k8s::{
    namespaced_name_selector,
    ApiSet,
    PodLifecycleData,
    GVK,
};
use crate::prelude::*;
use crate::store::{
    TraceStorable,
    TraceStore,
};
use crate::time::{
    Clockable,
    UtcClock,
};

type OwnerCache = SizedCache<String, Vec<metav1::OwnerReference>>;
pub(crate) const CACHE_SIZE: usize = 10000;

// The PodWatcher object monitors incoming pod events and records the relevant ones to the object
// store; becaues in clusters of any reasonable size, there are a) a lot of pods, and b) a lot of
// update events for each pod, the pod watcher does a bunch of caching and pre-filtering before
// sending events on to the object store.  Combined with the fact that figuring out which events we
// care about is a little non-trivial, means that the logic in here is a bit complicated.
//
// At a high level: whenever a pod event happens, we check to see whether any properties of its
// lifecycle data (currently start time and end time) have changed.  If so, we compute the
// ownership chain for the pod, and forward that info on to the store.

pub struct PodWatcher {
    apiset: ApiSet,
    pod_stream: PodStream,

    // We store the list of owned pods in memory here, and cache the ownership chain for each pod;
    // This is a simpler data structure than what the object store needs, to allow for easy lookup
    // by pod name.  (The object store needs to store a bunch of extra metadata about sequence
    // number and pod hash and so forth).
    owned_pods: HashMap<String, PodLifecycleData>,
    owners_cache: SizedCache<String, Vec<metav1::OwnerReference>>,
    store: Arc<Mutex<dyn TraceStorable + Send>>,

    clock: Box<dyn Clockable + Send>,
}

impl PodWatcher {
    // We take ownership of the apiset here, meaning that this has to be created after the
    // DynamicObject watcher.  We need ownership because pod owner chains could contain arbitrary
    // object types (we don't necessarily know what they are until we see the pod).  The
    // DynamicObject watcher just needs to construct the relevant api clients once, when it creates
    // the watch streams, so it can yield when it's done.  If at some point in the future this
    // becomes problematic, we can always stick the apiset in an Arc<Mutex<_>>.
    pub fn new(store: Arc<Mutex<TraceStore>>, apiset: ApiSet) -> PodWatcher {
        let pod_api: kube::Api<corev1::Pod> = kube::Api::all(apiset.client().clone());
        let pod_stream = watcher(pod_api, Default::default()).map_err(|e| e.into()).boxed();
        PodWatcher {
            apiset,
            pod_stream,
            store,
            owned_pods: HashMap::new(),
            owners_cache: SizedCache::with_size(CACHE_SIZE),
            clock: Box::new(UtcClock),
        }
    }

    pub async fn start(mut self) {
        while let Some(res) = self.pod_stream.next().await {
            match res {
                Ok(mut evt) => self.handle_pod_event(&mut evt).await,
                Err(e) => error!("pod watcher received error on stream: {}", e),
            }
        }
    }

    // We swallow errors inside handle_pod_lifecycle to make sure that, on a refresh event, if one
    // pod update fails we can still process the remaining events.  If we use ? and return an error
    // from handle_pod_event, then this function will bail after the first failed pod update.
    pub(super) async fn handle_pod_event(&mut self, evt: &mut Event<corev1::Pod>) {
        match evt {
            Event::Applied(pod) => {
                let ns_name = pod.namespaced_name();
                if let Err(e) = self.handle_pod_applied(&ns_name, pod).await {
                    error!("applied pod {} lifecycle data could not be stored: {}", ns_name, e);
                }
            },
            Event::Deleted(pod) => {
                let ns_name = pod.namespaced_name();
                let current_lifecycle_data = match self.owned_pods.get(&ns_name) {
                    None => {
                        warn!("pod {} deleted but not tracked, may have already been processed", ns_name);
                        return;
                    },
                    Some(data) => data.clone(),
                };
                if let Err(e) = self.handle_pod_deleted(&ns_name, Some(pod), current_lifecycle_data).await {
                    error!("deleted pod {} lifecycle data could not be stored: {}", ns_name, e);
                }
            },
            Event::Restarted(pods) => {
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
                    if let Err(e) = self.handle_pod_applied(ns_name, pod).await {
                        error!("applied pod {} lifecycle data could not be stored: {} (watcher restart)", ns_name, e);
                    }
                }

                for (ns_name, current_lifecycle_data) in &old_owned_pods {
                    // We don't have data on the deleted pods aside from the name, so we just pass
                    // in `None` for the pod object.
                    if let Err(e) = self.handle_pod_deleted(ns_name, None, current_lifecycle_data.clone()).await {
                        error!("deleted pod {} lifecycle data could not be stored: {} (watcher restart)", ns_name, e);
                    }
                }
            },
        };
    }

    async fn handle_pod_applied(&mut self, ns_name: &str, pod: &corev1::Pod) -> EmptyResult {
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
            self.store_pod_lifecycle_data(ns_name, Some(pod), new_lifecycle_data).await?;
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
                PodLifecycleData::Finished(start_ts, self.clock.now())
            },
            Some(pod) => PodLifecycleData::guess_finished_lifecycle(pod, &current_lifecycle_data, self.clock.borrow())?,
        };

        self.store_pod_lifecycle_data(ns_name, maybe_pod, new_lifecycle_data).await?;
        Ok(())
    }

    async fn store_pod_lifecycle_data(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        lifecycle_data: PodLifecycleData,
    ) -> EmptyResult {
        // Before storing the lifecycle data, we need to compute the ownership chain so the store
        // can determine if this pod is owned by anything it's tracking.  We do this _after_ we've
        // determined that we want to store the data but _before_ we unlock the object store, since
        // this involves a bunch of API calls, and we don't want to block either thread.
        let owners = match (self.owners_cache.cache_get(ns_name), maybe_pod) {
            (Some(o), _) => o.clone(),
            (None, Some(pod)) => compute_owner_chain(&mut self.apiset, pod, &mut self.owners_cache).await?,
            _ => bail!("could not determine owner chain for {}", ns_name),
        };

        info!(
            "{} owned by {:?} is {:?}",
            ns_name,
            owners
                .iter()
                .map(|rf| format!("{}/{}", rf.kind, rf.name))
                .collect::<Vec<String>>(),
            lifecycle_data
        );

        let mut store = self.store.lock().unwrap();
        store.record_pod_lifecycle(ns_name, maybe_pod.cloned(), owners, lifecycle_data)?;

        Ok(())
    }
}

// Recursively look up all of the owning objects for a given Kubernetes object
#[async_recursion]
pub(super) async fn compute_owner_chain(
    apiset: &mut ApiSet,
    obj: &(impl Resource + Sync),
    cache: &mut OwnerCache,
) -> anyhow::Result<Vec<metav1::OwnerReference>> {
    let ns_name = obj.namespaced_name();
    info!("computing owner references for {}", ns_name);

    if let Some(owners) = cache.cache_get(&ns_name) {
        info!("found owners for {} in cache", ns_name);
        return Ok(owners.clone());
    }

    let mut owners = Vec::from(obj.owner_references());

    for rf in obj.owner_references() {
        let gvk = GVK::from_owner_ref(rf)?;
        let api = apiset.api_for(gvk).await?;
        let resp = api.list(&namespaced_name_selector(&obj.namespace().unwrap(), &rf.name)).await?;
        if resp.items.len() != 1 {
            bail!("could not find single owner for {}, found {:?}", obj.namespaced_name(), resp.items);
        }

        let owner = &resp.items[0];
        owners.extend(compute_owner_chain(apiset, owner, cache).await?);
    }

    cache.cache_set(ns_name, owners.clone());
    Ok(owners)
}

#[cfg(test)]
impl PodWatcher {
    pub(crate) fn new_from_parts(
        apiset: ApiSet,
        pod_stream: PodStream,
        owned_pods: HashMap<String, PodLifecycleData>,
        owners_cache: SizedCache<String, Vec<metav1::OwnerReference>>,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
        clock: Box<dyn Clockable + Send>,
    ) -> PodWatcher {
        PodWatcher {
            apiset,
            pod_stream,
            owned_pods,
            owners_cache,
            store,
            clock,
        }
    }

    pub(crate) fn get_owned_pod_lifecycle(&self, ns_name: &str) -> Option<&PodLifecycleData> {
        self.owned_pods.get(ns_name)
    }
}
