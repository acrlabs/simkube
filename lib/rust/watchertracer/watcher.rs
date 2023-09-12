use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};

use futures::future::try_join_all;
use futures::stream::select_all::{
    select_all,
    SelectAll,
};
use futures::{
    Stream,
    StreamExt,
};
use json_patch::{
    patch,
    PatchErrorKind,
    PatchOperation,
    RemoveOperation,
};
use kube::api::DynamicObject;
use kube::discovery;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use tracing::*;

use crate::prelude::*;
use crate::util::{
    namespaced_name,
    Clockable,
    UtcClock,
};
use crate::watchertracer::tracer::Tracer;
use crate::watchertracer::watch_event::try_modify;

pub type KubeObjectStream<'a> = Pin<Box<dyn Stream<Item = Result<Event<DynamicObject>, SimKubeError>> + Send + 'a>>;

pub struct Watcher<'a> {
    w: SelectAll<KubeObjectStream<'a>>,
    t: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}

fn strip_obj(obj: &mut DynamicObject, pod_spec_path: &str) -> SimKubeResult<()> {
    obj.metadata.uid = None;
    obj.metadata.resource_version = None;
    obj.metadata.managed_fields = None;
    obj.metadata.creation_timestamp = None;
    obj.metadata.deletion_timestamp = None;
    obj.metadata.owner_references = None;

    for suffix in &["nodeName", "serviceAccount", "serviceAccountName"] {
        let p = PatchOperation::Remove(RemoveOperation { path: format!("{}/{}", pod_spec_path, suffix) });
        if let Err(e) = patch(&mut obj.data, &[p]) {
            match e.kind {
                PatchErrorKind::InvalidPointer => {
                    debug!("could not find path {} for object {}, skipping", e.path, namespaced_name(obj));
                },
                _ => return Err(SimKubeError::JsonPatchError(e)),
            }
        }
    }

    Ok(())
}

async fn build_api_for(obj_cfg: &TrackedObject, client: kube::Client) -> SimKubeResult<KubeObjectStream> {
    let apigroup = discovery::group(&client, &obj_cfg.api_version).await?;
    let (ar, _) = apigroup.recommended_kind(&obj_cfg.kind).unwrap();

    Ok(watcher(kube::Api::all_with(client, &ar), Default::default())
        .map(|str_res| match str_res {
            Ok(evt) => match try_modify(evt, |obj| strip_obj(obj, &obj_cfg.pod_spec_path)) {
                Ok(new_evt) => Ok(new_evt),
                Err(e) => Err(e),
            },
            Err(e) => Err(SimKubeError::KubeWatchError(e)),
        })
        .boxed())
}

impl<'a> Watcher<'a> {
    pub async fn new(client: kube::Client, t: Arc<Mutex<Tracer>>, config: &'a TracerConfig) -> SimKubeResult<Watcher> {
        let apis =
            try_join_all(config.tracked_objects.iter().map(|obj_cfg| build_api_for(obj_cfg, client.clone()))).await?;

        Ok(Watcher {
            w: select_all(apis),
            clock: Arc::new(Mutex::new(UtcClock {})),
            t,
        })
    }

    pub fn new_from_parts(w: KubeObjectStream, t: Arc<Mutex<Tracer>>, clock: Arc<Mutex<dyn Clockable>>) -> Watcher {
        return Watcher { w: select_all(vec![w]), t, clock };
    }

    pub async fn start(&mut self) {
        loop {
            tokio::select! {
                item = self.w.next() => if let Some(res) = item {
                    match res {
                        Ok(evt) => self.handle_pod_event(evt),
                        Err(e) => error!("tracer received error on stream: {}", e),
                    }
                } else { break },
            }
        }
    }

    fn handle_pod_event(&mut self, evt: Event<DynamicObject>) {
        let ts = self.clock.lock().unwrap().now();
        let mut tracer = self.t.lock().unwrap();

        match evt {
            Event::Applied(obj) => tracer.create_obj(&obj, ts),
            Event::Deleted(obj) => tracer.delete_obj(&obj, ts),
            Event::Restarted(objs) => tracer.update_all_objs(objs, ts),
        }
    }
}
