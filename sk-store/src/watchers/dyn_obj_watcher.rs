use std::sync::{
    Arc,
    Mutex,
    mpsc,
};

use async_trait::async_trait;
use futures::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::WatchStreamExt;
use kube::runtime::watcher::watcher;
use sk_core::errors::*;
use sk_core::k8s::{
    DynamicApiSet,
    GVK,
    build_deletable,
    sanitize_obj,
};
use sk_core::prelude::*;

use crate::TraceStorable;
use crate::watchers::{
    EventHandler,
    ObjWatcher,
};

pub async fn new_with_stream(
    gvk: &GVK,
    apiset: &mut DynamicApiSet,
    store: Arc<Mutex<dyn TraceStorable + Send>>,
    ready_tx: mpsc::Sender<bool>,
) -> anyhow::Result<ObjWatcher<DynamicObject>> {
    // TODO if this fails (e.g., because some custom resource isn't present in the cluster)
    // it will prevent the tracer from starting up
    let api_version = gvk.api_version().clone();
    let kind = gvk.kind.clone();

    // The "unnamespaced" api variant can list/watch in all namespaces
    let (api, _) = apiset.unnamespaced_api_by_gvk(gvk).await?;

    let dyn_obj_handler = Box::new(DynObjHandler { gvk: gvk.clone(), store });
    let dyn_obj_stream = watcher(api.clone(), Default::default())
        // All these objects need to be cloned because they're moved into the stream here
        .modify(move |obj| sanitize_obj(obj, &api_version, &kind))
        .map_err(|e| e.into())
        .boxed();

    Ok(ObjWatcher::new(dyn_obj_handler, dyn_obj_stream, ready_tx))
}

// Watch a (customizable) list of objects.  Since we don't know what these object types will be at
// runtime, we have to use the DynamicObject API, which gives us everything in JSON format that we
// have to parse.  Unlike the pod watcher, this is pretty straightforward.  We just forward all the
// events that we receive to the object store.
pub(super) struct DynObjHandler {
    gvk: GVK,
    store: Arc<Mutex<dyn TraceStorable + Send>>,
}

#[async_trait]
impl EventHandler<DynamicObject> for DynObjHandler {
    async fn applied(&mut self, obj: &DynamicObject, ts: i64) -> EmptyResult {
        let mut s = self.store.lock().expect("trace store mutex poisoned");
        s.create_or_update_obj(obj, ts, None)
    }

    async fn deleted(&mut self, ns: &str, name: &str, ts: i64) -> EmptyResult {
        let mut s = self.store.lock().expect("trace store mutex poisoned");
        let obj = build_deletable(&self.gvk, &format!("{ns}/{name}"));
        s.delete_obj(&obj, ts)
    }
}

#[cfg(test)]
use clockabilly::Clockable;

#[cfg(test)]
use super::ObjStream;

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
pub(crate) fn new_from_parts(
    gvk: GVK,
    store: Arc<Mutex<dyn TraceStorable + Send>>,
    stream: ObjStream<DynamicObject>,
    clock: Box<dyn Clockable + Send>,
    ready_tx: mpsc::Sender<bool>,
) -> ObjWatcher<DynamicObject> {
    ObjWatcher::new_from_parts(Box::new(DynObjHandler { gvk, store }), stream, clock, ready_tx)
}
