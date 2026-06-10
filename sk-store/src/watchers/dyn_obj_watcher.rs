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
    dyn_obj_spec_mut,
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
    // TODO if this function fails (e.g., because some requested custom resource isn't present in
    // the cluster) it will prevent the tracer from starting up

    // The GVK needs to be cloned ahead of time because it's moved into the stream
    let stream_gvk = gvk.clone();

    // The "unnamespaced" api variant can list/watch in all namespaces
    let (api, _) = apiset.unnamespaced_api_by_gvk(gvk).await?;

    let dyn_obj_handler = Box::new(DynObjHandler { gvk: gvk.clone(), dyn_obj_tx });
    let dyn_obj_stream = watcher(api.clone(), Default::default())
        .modify(move |obj| {
            // Kubernetes does not always fill out the TypeMeta (possibly for deleted resources, but I'm
            // pretty sure definitely for the results of a List API call -- you know, like when you run
            // ListAndWatch to start a new informer).  I _believe_ the type information for the list call only
            // gets populated on the outer wrapper of the list and not for individual objects in the list.
            //
            // There are a couple related GitHub issues here:
            //   - https://github.com/kubernetes-sigs/controller-runtime/issues/1517
            //   - https://github.com/kubernetes-sigs/controller-runtime/issues/1735
            //
            // ANYWAYS as a result of this extremely annoying behaviour, we fill in the type meta here,
            // which we know as a part of setting up the informer.
            obj.types = Some(stream_gvk.into_type_meta());

            // If we are tracking pod objects (whether bare or not!), we want to ignore the node it was
            // assigned to "in production" since this will certainly not exist in the simulation.
            if stream_gvk == *POD_GVK {
                dyn_obj_spec_mut(obj).map(|spec| spec.remove("nodeName"));
            }
            sanitize_obj(obj);
        })
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
