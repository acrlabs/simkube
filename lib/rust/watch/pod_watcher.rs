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
use kube::runtime::WatchStreamExt;
use kube::{
    Resource,
    ResourceExt,
};
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tracing::*;

use super::PodStream;
use crate::errors::*;
use crate::k8s::{
    list_params_for,
    namespaced_name,
    ApiSet,
    GVK,
};
use crate::time::{
    Clockable,
    UtcClock,
};
use crate::trace::Tracer;

type OwnerCache = SizedCache<String, Vec<metav1::OwnerReference>>;

pub struct PodWatcher {
    clock: Box<dyn Clockable + Send>,
    pod_stream: PodStream,
    tracer: Arc<Mutex<Tracer>>,
}

impl PodWatcher {
    pub fn new(tracer: Arc<Mutex<Tracer>>, apiset: ApiSet) -> PodWatcher {
        let pod_stream = build_stream_for_pods(apiset);
        PodWatcher { clock: Box::new(UtcClock), pod_stream, tracer }
    }

    pub async fn start(mut self) {
        while let Some(res) = self.pod_stream.next().await {
            let ts = self.clock.now();

            match res {
                Ok(evt) => self.handle_pod_event(evt, ts),
                Err(e) => error!("pod watcher received error on stream: {}", e),
            }
        }
    }

    fn handle_pod_event(&self, evt: Event<corev1::Pod>, ts: i64) {
        let mut tracer = self.tracer.lock().unwrap();
        match evt {
            Event::Applied(pod) => tracer.record_pod_lifecycle(&pod, ts),
            Event::Deleted(pod) => tracer.record_pod_deleted(&pod, ts),
            Event::Restarted(pods) => tracer.update_pod_lifecycles(&pods, ts),
        };
    }
}

#[async_recursion(?Send)]
async fn compute_owner_chain(
    apiset: &mut ApiSet,
    obj: &impl Resource,
    cache: &mut OwnerCache,
) -> anyhow::Result<Vec<metav1::OwnerReference>> {
    let ns_name = namespaced_name(obj);

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
            bail!("could not find single owner for {}, found {:?}", namespaced_name(obj), resp.items);
        }

        let owner = &resp.items[0];
        owners.extend(compute_owner_chain(apiset, owner, cache).await?);
    }

    cache.cache_set(ns_name, owners.clone());
    Ok(owners)
}

fn build_stream_for_pods(mut apiset: ApiSet) -> PodStream {
    let pod_api: kube::Api<corev1::Pod> = kube::Api::all(apiset.client().clone());
    let mut cache: SizedCache<String, Vec<metav1::OwnerReference>> = SizedCache::with_size(100);
    watcher(pod_api, Default::default())
        .modify(move |pod| {
            block_in_place(|| {
                Handle::current().block_on(async {
                    let owners = compute_owner_chain(&mut apiset, pod, &mut cache).await;
                    pod.metadata.owner_references = owners.ok();
                })
            });
        })
        .map_err(|e| e.into())
        .boxed()
}
