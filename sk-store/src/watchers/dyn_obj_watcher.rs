use std::sync::{
    Arc,
    Mutex,
};

use async_trait::async_trait;
use futures::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::watcher::watcher;
use kube::runtime::WatchStreamExt;
use sk_core::errors::*;
use sk_core::k8s::{
    sanitize_obj,
    ApiSet,
    GVK,
};
use sk_core::prelude::*;

use crate::watchers::{
    EventHandler,
    ObjStream,
};
use crate::TraceStorable;

// Watch a (customizable) list of objects.  Since we don't know what these object types will be at
// runtime, we have to use the DynamicObject API, which gives us everything in JSON format that we
// have to parse.  Unlike the pod watcher, this is pretty straightforward.  We just forward all the
// events that we receive to the object store.

pub struct DynObjHandler {
    gvk: GVK,
}

impl DynObjHandler {
    pub async fn new_with_stream(
        gvk: &GVK,
        apiset: &mut ApiSet,
    ) -> anyhow::Result<(Box<DynObjHandler>, ObjStream<DynamicObject>)> {
        // TODO if this fails (e.g., because some custom resource isn't present in the cluster)
        // it will prevent the tracer from starting up
        let api_version = gvk.api_version().clone();
        let kind = gvk.kind.clone();

        // The "unnamespaced" api variant can list/watch in all namespaces
        let (api, _) = apiset.unnamespaced_api_by_gvk(gvk).await?;

        Ok((
            Box::new(DynObjHandler { gvk: gvk.clone() }),
            watcher(api.clone(), Default::default())
                // All these objects need to be cloned because they're moved into the stream here
                .modify(move |obj| sanitize_obj(obj, &api_version, &kind))
                .map_err(|e| e.into())
                .boxed(),
        ))
    }
}

#[async_trait]
impl EventHandler<DynamicObject> for DynObjHandler {
    async fn applied(
        &mut self,
        obj: &DynamicObject,
        ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let mut s = store.lock().expect("trace store mutex poisoned");
        s.create_or_update_obj(obj, ts, None)
    }

    async fn deleted(
        &mut self,
        obj: &DynamicObject,
        ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let mut s = store.lock().expect("trace store mutex poisoned");
        s.delete_obj(obj, ts)
    }

    async fn initialized(
        &mut self,
        objs: &[DynamicObject],
        ts: i64,
        store: Arc<Mutex<dyn TraceStorable + Send>>,
    ) -> EmptyResult {
        let mut s = store.lock().expect("trace store mutex poisoned");
        s.update_all_objs_for_gvk(&self.gvk, objs, ts)
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
impl DynObjHandler {
    pub fn new(gvk: GVK) -> Box<DynObjHandler> {
        Box::new(DynObjHandler { gvk })
    }
}
