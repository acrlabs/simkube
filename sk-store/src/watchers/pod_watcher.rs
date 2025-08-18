use std::collections::HashMap;

use async_trait::async_trait;
use futures::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::watcher::watcher;
use sk_core::errors::*;
use sk_core::k8s::PodLifecycleData;
use sk_core::prelude::*;
use tokio::sync::mpsc;
use tracing::*;

use crate::watchers::{
    EventHandler,
    ObjWatcher,
};

#[derive(Debug)]
pub struct Message {
    pub(crate) ns_name: String,
    pub(crate) maybe_pod: Option<corev1::Pod>,
    pub(crate) lifecycle_data: PodLifecycleData,
}
pub type Sender = mpsc::UnboundedSender<Message>;
pub type Receiver = mpsc::UnboundedReceiver<Message>;

pub fn new_with_stream(
    client: kube::Client,
    pod_tx: Sender,
    ready_tx: mpsc::Sender<bool>,
) -> anyhow::Result<ObjWatcher<corev1::Pod>> {
    let pod_api: kube::Api<corev1::Pod> = kube::Api::all(client);
    let pod_handler = Box::new(PodHandler { owned_pods: HashMap::new(), pod_tx });
    let pod_stream = watcher(pod_api, Default::default()).map_err(|e| e.into()).boxed();
    Ok(ObjWatcher::new(pod_handler, pod_stream, ready_tx))
}

// The PodHandler object monitors incoming pod events and records the relevant ones to the object
// store; becaues in clusters of any reasonable size, there are a) a lot of pods, and b) a lot of
// update events for each pod, the pod watcher does a bunch of caching and pre-filtering before
// sending events on to the object store.  Combined with the fact that figuring out which events we
// care about is a little non-trivial, means that the logic in here is a bit complicated.
//
// At a high level: whenever a pod event happens, we check to see whether any properties of its
// lifecycle data (currently start time and end time) have changed.  If so we forward that info
// on to the store.
pub(super) struct PodHandler {
    // We store the list of owned pods in memory here, and cache the ownership chain for each pod;
    // This is a simpler data structure than what the object store needs, to allow for easy lookup
    // by pod name.  (The object store needs to store a bunch of extra metadata about sequence
    // number and pod hash and so forth).
    owned_pods: HashMap<String, PodLifecycleData>,
    pod_tx: Sender,
}

impl PodHandler {
    async fn handle_pod_applied(&mut self, ns_name: &str, pod: corev1::Pod) -> EmptyResult {
        let new_lifecycle_data = PodLifecycleData::new_for(&pod)?;
        let current_lifecycle_data = self.owned_pods.get(ns_name);

        // We only store data if the lifecycle data has changed; there is some magic happening in
        // the > operator here; see pod_lifecycle.rs for details, but in short, we only allow
        // lifecycle updates if the timestamps match and we've moved from one state to the next.
        //
        // Note that we only store non-empty lifecycle data, which is enforced since
        // PodLifecycleData::Empty < everything.
        if new_lifecycle_data > current_lifecycle_data {
            self.owned_pods.insert(ns_name.into(), new_lifecycle_data.clone());
            self.send_pod_lifecycle_data(ns_name, Some(pod), new_lifecycle_data).await?;
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
        current_lifecycle_data: PodLifecycleData,
        ts: i64,
    ) -> EmptyResult {
        // Always remove the pod from our tracker, regardless of what else happens
        self.owned_pods.remove(ns_name);

        // If the current lifecycle data is finished, we know it's already been written to the
        // store so we don't store it a second time.
        if current_lifecycle_data.finished() {
            return Ok(());
        }

        // We never store "empty" data so this unwrap should always succeed
        let start_ts = current_lifecycle_data.start_ts().unwrap();

        // We're just guessing that the pod finished timestamp is the timestamp we received
        // the event, which might not be perfect, but a) we aren't guaranteed to have access
        // to the pod object that was deleted, and b) this lifecycle stuff is all guesswork
        // anyways, and c) this makes the code a heck of a lot simpler.
        let new_lifecycle_data = PodLifecycleData::Finished(start_ts, ts);

        self.send_pod_lifecycle_data(ns_name, None, new_lifecycle_data).await
    }

    async fn send_pod_lifecycle_data(
        &self,
        ns_name: &str,
        maybe_pod: Option<corev1::Pod>,
        lifecycle_data: PodLifecycleData,
    ) -> EmptyResult {
        self.pod_tx
            .send(Message { ns_name: ns_name.into(), maybe_pod, lifecycle_data })?;
        Ok(())
    }
}

#[async_trait]
impl EventHandler<corev1::Pod> for PodHandler {
    // TODO test it's OK to leave extra things in the owned_pods index (if it's still there when
    // we're done with all this
    async fn applied(&mut self, pod: corev1::Pod, _ts: i64) -> EmptyResult {
        let ns_name = pod.namespaced_name();
        self.handle_pod_applied(&ns_name, pod).await
    }

    async fn deleted(&mut self, ns_name: &str, ts: i64) -> EmptyResult {
        let Some(current_lifecycle_data) = self.owned_pods.get(ns_name) else {
            warn!("pod {ns_name} deleted but not tracked, may have already been processed");
            return Ok(());
        };
        self.handle_pod_deleted(ns_name, current_lifecycle_data.clone(), ts).await
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
impl PodHandler {
    pub(crate) fn new_from_parts(owned_pods: HashMap<String, PodLifecycleData>, pod_tx: Sender) -> PodHandler {
        PodHandler { owned_pods, pod_tx }
    }

    pub(crate) fn get_owned_pod_lifecycle(&self, ns_name: &str) -> Option<&PodLifecycleData> {
        self.owned_pods.get(ns_name)
    }
}
