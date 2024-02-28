use std::borrow::Borrow;
use std::collections::HashMap;
use std::mem::take;
use std::sync::mpsc::{
    Receiver,
    Sender,
};
use std::sync::{
    mpsc,
    Arc,
    Mutex,
};

use futures::stream::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::watcher::{
    watcher,
    Event,
};
use tracing::*;

use super::*;
use crate::errors::*;
use crate::k8s::{
    ApiSet,
    OwnersCache,
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
    pod_stream: PodStream,

    // We store the list of owned pods in memory here, and cache the ownership chain for each pod;
    // This is a simpler data structure than what the object store needs, to allow for easy lookup
    // by pod name.  (The object store needs to store a bunch of extra metadata about sequence
    // number and pod hash and so forth).
    owned_pods: HashMap<String, PodLifecycleData>,
    owners_cache: OwnersCache,
    store: Arc<Mutex<dyn TraceStorable + Send>>,

    clock: Box<dyn Clockable + Send>,
    is_ready: bool,
    ready_tx: Sender<bool>,
}

impl PodWatcher {
    // We take ownership of the apiset here, meaning that this has to be created after the
    // DynamicObject watcher.  We need ownership because pod owner chains could contain arbitrary
    // object types (we don't necessarily know what they are until we see the pod).  The
    // DynamicObject watcher just needs to construct the relevant api clients once, when it creates
    // the watch streams, so it can yield when it's done.  If at some point in the future this
    // becomes problematic, we can always stick the apiset in an Arc<Mutex<_>>.
    pub fn new(client: kube::Client, store: Arc<Mutex<TraceStore>>, apiset: ApiSet) -> (PodWatcher, Receiver<bool>) {
        let pod_api: kube::Api<corev1::Pod> = kube::Api::all(client);
        let pod_stream = watcher(pod_api, Default::default()).map_err(|e| e.into()).boxed();
        let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

        (
            PodWatcher {
                pod_stream,

                owned_pods: HashMap::new(),
                owners_cache: OwnersCache::new(apiset),
                store,

                clock: Box::new(UtcClock),
                is_ready: false,
                ready_tx: tx,
            },
            rx,
        )
    }

    // This is not a reference because it needs to "own" itself when tokio spawns it
    pub async fn start(mut self) {
        while let Some(res) = self.pod_stream.next().await {
            match res {
                Ok(mut evt) => self.handle_pod_event(&mut evt).await,
                Err(err) => {
                    skerr!(err, "pod watcher received error on stream");
                },
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
                if let Err(err) = self.handle_pod_applied(&ns_name, pod).await {
                    skerr!(err, "applied pod {} lifecycle data could not be stored", ns_name);
                }
            },
            Event::Deleted(pod) => {
                let ns_name = pod.namespaced_name();
                let current_lifecycle_data = match self.owned_pods.get(&ns_name) {
                    None => {
                        warn!("pod {ns_name} deleted but not tracked, may have already been processed");
                        return;
                    },
                    Some(data) => data.clone(),
                };
                if let Err(err) = self.handle_pod_deleted(&ns_name, Some(pod), current_lifecycle_data).await {
                    skerr!(err, "deleted pod {} lifecycle data could not be stored", ns_name);
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
                    if let Err(err) = self.handle_pod_applied(ns_name, pod).await {
                        skerr!(err, "(watcher restart) applied pod {} lifecycle data could not be stored", ns_name);
                    }
                }

                for (ns_name, current_lifecycle_data) in &old_owned_pods {
                    // We don't have data on the deleted pods aside from the name, so we just pass
                    // in `None` for the pod object.
                    if let Err(err) = self.handle_pod_deleted(ns_name, None, current_lifecycle_data.clone()).await {
                        skerr!(err, "(watcher restart) deleted pod {} lifecycle data could not be stored", ns_name);
                    }
                }

                // When the watcher first starts up it does a List call, which (internally) gets
                // converted into a "Restarted" event that contains all of the listed objects.
                // Once we've handled this event the first time, we know we have a complete view of
                // the cluster at startup time.
                if !self.is_ready {
                    self.is_ready = true;

                    // TODO probably don't want to unwrap this
                    // unlike golang, sending is non-blocking
                    self.ready_tx.send(true).unwrap();
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
            self.store_pod_lifecycle_data(ns_name, Some(pod), &new_lifecycle_data).await?;
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

        self.store_pod_lifecycle_data(ns_name, maybe_pod, &new_lifecycle_data).await
    }

    async fn store_pod_lifecycle_data(
        &mut self,
        ns_name: &str,
        maybe_pod: Option<&corev1::Pod>,
        lifecycle_data: &PodLifecycleData,
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

        // We don't expect the trace store to panic, but if it does, we should panic here too
        let mut store = self.store.lock().unwrap();
        store.record_pod_lifecycle(ns_name, maybe_pod.cloned(), owners, lifecycle_data)
    }
}

#[cfg(test)]
impl PodWatcher {
    pub(crate) fn new_from_parts(
        pod_stream: PodStream,
        owned_pods: HashMap<String, PodLifecycleData>,
        owners_cache: OwnersCache,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
        clock: Box<dyn Clockable + Send>,
    ) -> PodWatcher {
        let (tx, _): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        PodWatcher {
            pod_stream,
            owned_pods,
            owners_cache,
            store,
            clock,
            is_ready: true,
            ready_tx: tx,
        }
    }

    pub(crate) fn get_owned_pod_lifecycle(&self, ns_name: &str) -> Option<&PodLifecycleData> {
        self.owned_pods.get(ns_name)
    }
}
