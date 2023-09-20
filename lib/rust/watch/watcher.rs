use std::sync::{
    Arc,
    Mutex,
    MutexGuard,
};

use futures::future::try_join_all;
use futures::stream::select_all::{
    select_all,
    SelectAll,
};
use futures::stream::TryStreamExt;
use futures::StreamExt;
use k8s_openapi::api::core::v1 as corev1;
use kube::api::{
    DynamicObject,
    GroupVersionKind,
};
use kube::core::TypeMeta;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use tracing::*;

use super::{
    KubeEvent,
    KubeObjectStream,
};
use crate::k8s::{
    get_api_resource,
    strip_obj,
};
use crate::prelude::*;
use crate::time::{
    Clockable,
    UtcClock,
};
use crate::trace::Tracer;

pub struct Watcher<'a> {
    objs: SelectAll<KubeObjectStream<'a>>,
    tracer: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}

async fn build_stream_for_tracked_obj<'a>(
    gvk: &'a GroupVersionKind,
    obj_cfg: &'a TrackedObject,
    client: kube::Client,
) -> anyhow::Result<KubeObjectStream<'a>> {
    let (ar, _) = get_api_resource(gvk, &client).await?;
    Ok(watcher(kube::Api::all_with(client, &ar), Default::default())
        .map_ok(|evt| {
            let evt = evt.modify(|obj| {
                strip_obj(obj, &obj_cfg.pod_spec_path);
                obj.types = Some(TypeMeta {
                    api_version: gvk.api_version(),
                    kind: gvk.kind.clone(),
                })
            });
            KubeEvent::Dyn(evt)
        })
        .map_err(|e| e.into())
        .boxed())
}

fn build_stream_for_pods(client: kube::Client) -> KubeObjectStream<'static> {
    let pod_api: kube::Api<corev1::Pod> = kube::Api::all(client);
    watcher(pod_api, Default::default())
        .map_ok(KubeEvent::Pod)
        .map_err(|e| e.into())
        .boxed()
}

impl<'a> Watcher<'a> {
    pub async fn new(client: kube::Client, t: Arc<Mutex<Tracer>>, config: &'a TracerConfig) -> anyhow::Result<Watcher> {
        let mut apis = try_join_all(
            config
                .tracked_objects
                .iter()
                .map(|(gvk, obj_cfg)| build_stream_for_tracked_obj(gvk, obj_cfg, client.clone())),
        )
        .await?;
        apis.push(build_stream_for_pods(client.clone()));

        Ok(Watcher {
            objs: select_all(apis),
            clock: Arc::new(Mutex::new(UtcClock {})),
            tracer: t,
        })
    }

    pub fn new_from_parts(
        objs: KubeObjectStream<'a>,
        pods: KubeObjectStream<'a>,
        tracer: Arc<Mutex<Tracer>>,
        clock: Arc<Mutex<dyn Clockable>>,
    ) -> Watcher<'a> {
        Watcher { objs: select_all(vec![objs, pods]), tracer, clock }
    }

    pub async fn start(&mut self) {
        loop {
            tokio::select! {
                item = self.objs.next() => if let Some(res) = item {
                    let ts = self.clock.lock().unwrap().now();
                    let mut tracer = self.tracer.lock().unwrap();

                    match res {
                        Ok(KubeEvent::Dyn(evt)) => self.handle_obj_event(evt, ts, &mut tracer),
                        Ok(KubeEvent::Pod(evt)) => self.handle_pod_event(evt, ts, &mut tracer),
                        Err(e) => error!("watcher received error on stream: {}", e),
                    }
                } else { break },
            }
        }
    }

    fn handle_obj_event(&self, evt: Event<DynamicObject>, ts: i64, tracer: &mut MutexGuard<Tracer>) {
        match evt {
            Event::Applied(obj) => tracer.create_or_update_obj(&obj, ts),
            Event::Deleted(obj) => tracer.delete_obj(&obj, ts),
            Event::Restarted(objs) => tracer.update_all_objs(&objs, ts),
        };
    }

    fn handle_pod_event(&self, evt: Event<corev1::Pod>, ts: i64, tracer: &mut MutexGuard<Tracer>) {
        match evt {
            Event::Applied(pod) => tracer.record_pod_lifecycle(&pod, ts),
            Event::Deleted(pod) => tracer.record_pod_deleted(&pod, ts),
            Event::Restarted(pods) => tracer.update_pod_lifecycles(pods, ts),
        }
    }
}
