use std::collections::HashMap;
use std::pin::Pin;
use std::sync::mpsc::{
    Receiver,
    Sender,
};
use std::sync::{
    mpsc,
    Arc,
    Mutex,
};

use clockabilly::{
    Clockable,
    UtcClock,
};
use futures::stream::select_all::{
    select_all,
    SelectAll,
};
use futures::{
    Stream,
    StreamExt,
    TryStreamExt,
};
use kube::api::DynamicObject;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use kube::runtime::WatchStreamExt;
use sk_core::errors::*;
use sk_core::k8s::{
    sanitize_obj,
    ApiSet,
    GVK,
};
use sk_core::prelude::*;

use crate::{
    TraceStorable,
    TraceStore,
    TrackedObjectConfig,
};

pub type KubeObjectStream = Pin<Box<dyn Stream<Item = anyhow::Result<Event<DynamicObject>>> + Send>>;

// Watch a (customizable) list of objects.  Since we don't know what these object types will be at
// runtime, we have to use the DynamicObject API, which gives us everything in JSON format that we
// have to parse.  Unlike the pod watcher, this is pretty straightforward.  We just forward all the
// events that we receive to the object store.

pub struct DynObjWatcher {
    clock: Box<dyn Clockable + Send>,
    obj_stream: SelectAll<KubeObjectStream>,
    store: Arc<Mutex<dyn TraceStorable + Send>>,

    is_ready: bool,
    ready_tx: Sender<bool>,
}

impl DynObjWatcher {
    pub async fn new(
        store: Arc<Mutex<TraceStore>>,
        apiset: &mut ApiSet,
        tracked_objects: &HashMap<GVK, TrackedObjectConfig>,
    ) -> anyhow::Result<(DynObjWatcher, Receiver<bool>)> {
        let mut apis = vec![];
        for gvk in tracked_objects.keys() {
            let stream = build_stream_for_tracked_obj(apiset, gvk).await?;
            apis.push(stream);
        }

        let (tx, rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

        Ok((
            DynObjWatcher {
                clock: UtcClock::boxed(),
                obj_stream: select_all(apis),
                store,

                is_ready: false,
                ready_tx: tx,
            },
            rx,
        ))
    }

    pub async fn start(mut self) {
        while let Some(res) = self.obj_stream.next().await {
            let ts = self.clock.now_ts();

            match res {
                Ok(evt) => self.handle_obj_event(evt, ts),
                Err(err) => {
                    skerr!(err, "watcher received error on stream");
                },
            }
        }
    }

    fn handle_obj_event(&mut self, evt: Event<DynamicObject>, ts: i64) {
        // We don't expect the trace store to panic, but if it does we should panic here too
        let mut store = self.store.lock().unwrap();
        match evt {
            Event::Applied(obj) => store.create_or_update_obj(&obj, ts, None),
            Event::Deleted(obj) => store.delete_obj(&obj, ts),
            Event::Restarted(objs) => {
                store.update_all_objs(&objs, ts);

                // When the watcher first starts up it does a List call, which (internally) gets
                // converted into a "Restarted" event that contains all of the listed objects.
                // Once we've handled this event the first time, we know we have a complete view of
                // the cluster at startup time.
                if !self.is_ready {
                    self.is_ready = true;

                    // unlike golang, sending is non-blocking
                    if let Err(e) = self.ready_tx.send(true) {
                        error!("failed to update dynobjwatcher ready status: {e:?}")
                    }
                }
            },
        };
    }
}

async fn build_stream_for_tracked_obj(apiset: &mut ApiSet, gvk: &GVK) -> anyhow::Result<KubeObjectStream> {
    // TODO if this fails (e.g., because some custom resource isn't present in the cluster)
    // it will prevent the tracer from starting up
    let api_version = gvk.api_version().clone();
    let kind = gvk.kind.clone();
    let (api, _) = apiset.api_for(gvk).await?;

    Ok(watcher(api.clone(), Default::default())
        // All these objects need to be cloned because they're moved into the stream here
        .modify(move |obj| {
            sanitize_obj(obj, &api_version, &kind);
        })
        .map_err(|e| e.into())
        .boxed())
}

#[cfg(test)]
impl DynObjWatcher {
    pub fn new_from_parts(
        objs: KubeObjectStream,
        store: Arc<Mutex<TraceStore>>,
        clock: Box<dyn Clockable + Send>,
    ) -> DynObjWatcher {
        let (tx, _): (Sender<bool>, Receiver<bool>) = mpsc::channel();
        DynObjWatcher {
            obj_stream: select_all(vec![objs]),
            store,
            clock,
            is_ready: true,
            ready_tx: tx,
        }
    }
}
