use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};

use futures::future::try_join_all;
use futures::stream::select_all::select_all;
use futures::{
    Stream,
    StreamExt,
};
use kube::api::DynamicObject;
use kube::discovery;
use kube::runtime::watcher::{
    watcher,
    Error,
    Event,
};
use kube::runtime::WatchStreamExt;
use tracing::*;

use crate::prelude::*;
use crate::util::{
    Clockable,
    UtcClock,
};
use crate::watchertracer::tracer::Tracer;

pub type KubeObjectStream = Pin<Box<dyn Stream<Item = Result<Event<DynamicObject>, Error>> + Send>>;

pub struct Watcher {
    w: futures::stream::SelectAll<KubeObjectStream>,
    t: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}

fn strip_obj(obj: &mut DynamicObject) {
    obj.metadata.uid = None;
    obj.metadata.resource_version = None;
    obj.metadata.managed_fields = None;
    obj.metadata.creation_timestamp = None;
    obj.metadata.deletion_timestamp = None;
    obj.metadata.owner_references = None;
    // if let Some(ref mut pspec) = obj.spec {
    //     pspec.node_name = None;
    //     pspec.service_account = None;
    //     pspec.service_account_name = None;
    // }
}

async fn build_api_for(obj: &TrackedObject, client: kube::Client) -> SimKubeResult<KubeObjectStream> {
    let apigroup = discovery::group(&client, &obj.api_version).await?;
    let (ar, _) = apigroup.recommended_kind(&obj.kind).unwrap();
    Ok(watcher(kube::Api::all_with(client, &ar), Default::default()).modify(strip_obj).boxed())
}

impl Watcher {
    pub async fn new(client: kube::Client, t: Arc<Mutex<Tracer>>, config: &TracerConfig) -> SimKubeResult<Watcher> {
        let apis = try_join_all(config.tracked_objects.iter().map(|obj| build_api_for(obj, client.clone()))).await?;

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
