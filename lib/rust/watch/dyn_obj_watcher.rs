use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

use futures::stream::select_all::{
    select_all,
    SelectAll,
};
use futures::stream::TryStreamExt;
use futures::StreamExt;
use kube::api::DynamicObject;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use kube::runtime::WatchStreamExt;
use tracing::*;

use super::KubeObjectStream;
use crate::k8s::{
    sanitize_obj,
    ApiSet,
    GVK,
};
use crate::prelude::*;
use crate::time::{
    Clockable,
    UtcClock,
};
use crate::trace::Tracer;

pub struct DynObjWatcher {
    clock: Box<dyn Clockable + Send>,
    obj_stream: SelectAll<KubeObjectStream>,
    tracer: Arc<Mutex<Tracer>>,
}

impl DynObjWatcher {
    pub async fn new(
        tracer: Arc<Mutex<Tracer>>,
        apiset: &mut ApiSet,
        tracked_objects: &HashMap<GVK, TrackedObjectConfig>,
    ) -> anyhow::Result<DynObjWatcher> {
        let mut apis = vec![];
        for (gvk, obj_cfg) in tracked_objects {
            let stream = build_stream_for_tracked_obj(apiset, gvk, &obj_cfg.pod_spec_path).await?;
            apis.push(stream);
        }

        Ok(DynObjWatcher {
            clock: Box::new(UtcClock),
            obj_stream: select_all(apis),
            tracer,
        })
    }

    pub fn new_from_parts(
        objs: KubeObjectStream,
        tracer: Arc<Mutex<Tracer>>,
        clock: Box<dyn Clockable + Send>,
    ) -> DynObjWatcher {
        DynObjWatcher { obj_stream: select_all(vec![objs]), tracer, clock }
    }

    pub async fn start(mut self) {
        while let Some(res) = self.obj_stream.next().await {
            let ts = self.clock.now();

            match res {
                Ok(evt) => self.handle_obj_event(evt, ts),
                Err(e) => error!("watcher received error on stream: {}", e),
            }
        }
    }

    fn handle_obj_event(&self, evt: Event<DynamicObject>, ts: i64) {
        let mut tracer = self.tracer.lock().unwrap();
        match evt {
            Event::Applied(obj) => tracer.create_or_update_obj(&obj, ts, None),
            Event::Deleted(obj) => tracer.delete_obj(&obj, ts),
            Event::Restarted(objs) => tracer.update_all_objs(&objs, ts),
        };
    }
}

async fn build_stream_for_tracked_obj(
    apiset: &mut ApiSet,
    gvk: &GVK,
    pod_spec_path: &str,
) -> anyhow::Result<KubeObjectStream> {
    let gvk = gvk.clone();
    let pod_spec_path = pod_spec_path.to_owned();

    let api_version = gvk.api_version().clone();
    let kind = gvk.kind.clone();
    let api = apiset.api_for(gvk).await?;

    Ok(watcher(api.clone(), Default::default())
        .modify(move |obj| {
            sanitize_obj(obj, &pod_spec_path, &api_version, &kind);
        })
        .map_err(|e| e.into())
        .boxed())
}
