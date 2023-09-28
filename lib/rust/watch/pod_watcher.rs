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
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use kube::{
    Resource,
    ResourceExt,
};
use tracing::*;

use super::PodStream;
use crate::errors::*;
use crate::k8s::{
    list_params_for,
    ApiSet,
    KubeResourceExt,
    PodLifecycleData,
    GVK,
};
use crate::store::TraceStore;

type OwnerCache = SizedCache<String, Vec<metav1::OwnerReference>>;
const CACHE_SIZE: usize = 10000;

pub struct PodWatcher {
    apiset: ApiSet,
    pod_stream: PodStream,
    store: Arc<Mutex<TraceStore>>,
    owned_pods: HashMap<String, PodLifecycleData>,
    owners_cache: SizedCache<String, Vec<metav1::OwnerReference>>,
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
        }
    }

    pub async fn start(mut self) {
        while let Some(res) = self.pod_stream.next().await {
            match res {
                Ok(mut evt) => {
                    let _ = self.handle_pod_event(&mut evt).await;
                },
                Err(e) => error!("pod watcher received error on stream: {}", e),
            }
        }
    }

    async fn handle_pod_event(&mut self, evt: &mut Event<corev1::Pod>) -> EmptyResult {
        match evt {
            Event::Applied(pod) => {
                let ns_name = pod.namespaced_name();
                let current_lifecycle_data = self.owned_pods.get(&ns_name).cloned();
                self.handle_pod_applied(&ns_name, pod, current_lifecycle_data).await?;
            },
            Event::Deleted(pod) => {
                let ns_name = pod.namespaced_name();
                let current_lifecycle_data = match self.owned_pods.get(&ns_name) {
                    None => {
                        warn!("pod {} deleted but not tracked, may have already been processed", ns_name);
                        return Ok(());
                    },
                    Some(data) => data.clone(),
                };
                self.handle_pod_deleted(&ns_name, Some(pod), current_lifecycle_data).await?;
            },
            Event::Restarted(pods) => {
                let mut old_owned_pods = take(&mut self.owned_pods);
                for pod in pods {
                    let ns_name = &pod.namespaced_name();
                    let old_entry = old_owned_pods.remove(ns_name);
                    self.handle_pod_applied(ns_name, pod, old_entry).await?;
                }

                for (ns_name, current_lifecycle_data) in &old_owned_pods {
                    self.handle_pod_deleted(ns_name, None, current_lifecycle_data.clone()).await?;
                }
            },
        };

        Ok(())
    }

    async fn handle_pod_applied(
        &mut self,
        ns_name: &str,
        pod: &corev1::Pod,
        current_lifecycle_data: Option<PodLifecycleData>,
    ) -> EmptyResult {
        let lifecycle_data = PodLifecycleData::new_for(pod)?;

        if lifecycle_data > current_lifecycle_data {
            self.owned_pods.insert(ns_name.into(), lifecycle_data.clone());
            self.store_pod_lifecycle_data(ns_name, Some(pod), &lifecycle_data).await?;
        } else if lifecycle_data != current_lifecycle_data {
            warn!(
                "new lifecycle data for {} does not match stored data, cowardly refusing to update: {:?} !>= {:?}",
                ns_name, lifecycle_data, current_lifecycle_data
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
        let new_lifecycle_data = PodLifecycleData::guess_finished(maybe_pod, &current_lifecycle_data);

        if current_lifecycle_data.finished() {
            // do nothing
            if new_lifecycle_data != current_lifecycle_data {
                warn!(
                    "new lifecycle data for {} does not match stored data, cowardly refusing to update: {:?} !>= {:?}",
                    ns_name, current_lifecycle_data, new_lifecycle_data,
                );
            }
        } else if new_lifecycle_data.finished() && new_lifecycle_data > current_lifecycle_data {
            self.store_pod_lifecycle_data(ns_name, maybe_pod, &new_lifecycle_data).await?;
        } else {
            bail!("could not determine lifecycle data for pod {}", ns_name);
        }

        Ok(())
    }

    async fn store_pod_lifecycle_data(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        lifecycle_data: &PodLifecycleData,
    ) -> EmptyResult {
        let owners = match (self.owners_cache.cache_get(ns_name), maybe_pod) {
            (Some(o), _) => o.clone(),
            (None, Some(pod)) => compute_owner_chain(&mut self.apiset, pod, &mut self.owners_cache).await?,
            _ => bail!("could not store lifecycle data for {}", ns_name),
        };

        let mut store = self.store.lock().unwrap();
        store.record_pod_lifecycle(ns_name, owners, lifecycle_data);

        Ok(())
    }
}

#[async_recursion]
async fn compute_owner_chain(
    apiset: &mut ApiSet,
    obj: &(impl Resource + Sync),
    cache: &mut OwnerCache,
) -> anyhow::Result<Vec<metav1::OwnerReference>> {
    let ns_name = obj.namespaced_name();

    if let Some(owners) = cache.cache_get(&ns_name) {
        info!("found owners for {} in cache", ns_name);
        return Ok(owners.clone());
    }

    info!("computing owner references for {}", ns_name);
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
