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
    list_params_for,
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

pub struct PodWatcher {
    apiset: ApiSet,
    pod_stream: PodStream,

    owned_pods: HashMap<String, PodLifecycleData>,
    owners_cache: SizedCache<String, Vec<metav1::OwnerReference>>,
    store: Arc<Mutex<dyn TraceStorable + Send>>,

    clock: Box<dyn Clockable + Send>,
}

impl PodWatcher {
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

    async fn handle_pod_deleted(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        current_lifecycle_data: PodLifecycleData,
    ) -> EmptyResult {
        // Always remove the pod from our tracker, regardless of what else happens
        self.owned_pods.remove(ns_name);

        if current_lifecycle_data.finished() {
            return Ok(());
        }

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
        let resp = api.list(&list_params_for(&obj.namespace().unwrap(), &rf.name)).await?;
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
