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
use kube::api::DynamicObject;
use kube::discovery;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use tracing::*;

use crate::prelude::*;
use crate::util::{
    strip_obj,
    Clockable,
    UtcClock,
};
use crate::watchertracer::tracer::Tracer;
use crate::watchertracer::watch_event::TryModify;

pub type KubeObjectStream<'a> = Pin<Box<dyn Stream<Item = SimKubeResult<Event<DynamicObject>>> + Send + 'a>>;

pub struct Watcher<'a> {
    w: SelectAll<KubeObjectStream<'a>>,
    t: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}


async fn build_api_for(obj_cfg: &TrackedObject, client: kube::Client) -> SimKubeResult<KubeObjectStream> {
    let apigroup = discovery::group(&client, &obj_cfg.api_version).await?;
    let (ar, _) = apigroup.recommended_kind(&obj_cfg.kind).unwrap();

    Ok(watcher(kube::Api::all_with(client, &ar), Default::default())
        .map(|str_res| match str_res {
            Ok(evt) => evt.try_modify(|obj| strip_obj(obj, &obj_cfg.pod_spec_path)),
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
