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
use tokio::sync::mpsc;

use crate::TraceAction;
use crate::watchers::{
    EventHandler,
    ObjWatcher,
};

#[derive(Debug)]
pub struct Message {
    pub(crate) action: TraceAction,
    pub(crate) obj: DynamicObject,
    pub(crate) ts: i64,
}
pub type Sender = mpsc::UnboundedSender<Message>;
pub type Receiver = mpsc::UnboundedReceiver<Message>;

pub async fn new_with_stream(
    gvk: &GVK,
    apiset: &mut DynamicApiSet,
    dyn_obj_tx: Sender,
    ready_tx: mpsc::Sender<bool>,
) -> anyhow::Result<ObjWatcher<DynamicObject>> {
    // TODO if this fails (e.g., because some custom resource isn't present in the cluster)
    // it will prevent the tracer from starting up
    let api_version = gvk.api_version().clone();
    let kind = gvk.kind.clone();

    // The "unnamespaced" api variant can list/watch in all namespaces
    let (api, _) = apiset.unnamespaced_api_by_gvk(gvk).await?;

    let dyn_obj_handler = Box::new(DynObjHandler { gvk: gvk.clone(), dyn_obj_tx });
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
    dyn_obj_tx: Sender,
}

#[async_trait]
impl EventHandler<DynamicObject> for DynObjHandler {
    async fn applied(&mut self, obj: DynamicObject, ts: i64) -> EmptyResult {
        self.dyn_obj_tx.send(Message { action: TraceAction::ObjectApplied, obj, ts })?;
        Ok(())
    }

    async fn deleted(&mut self, ns_name: &str, ts: i64) -> EmptyResult {
        let obj = build_deletable(&self.gvk, ns_name);
        self.dyn_obj_tx.send(Message { action: TraceAction::ObjectDeleted, obj, ts })?;
        Ok(())
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
    dyn_obj_tx: Sender,
    stream: ObjStream<DynamicObject>,
    clock: Box<dyn Clockable + Send>,
    ready_tx: mpsc::Sender<bool>,
) -> ObjWatcher<DynamicObject> {
    ObjWatcher::new_from_parts(Box::new(DynObjHandler { gvk, dyn_obj_tx }), stream, clock, ready_tx)
}
