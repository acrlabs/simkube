use std::sync::Arc;

use sk_core::k8s::DynamicApiSet;
use tokio::sync::{
    Mutex,
    mpsc,
};
use tokio::task::JoinSet;
use tracing::*;

use crate::config::TracerConfig;
use crate::event::TraceAction;
use crate::store::TraceStore;
use crate::watchers::{
    dyn_obj_watcher,
    pod_watcher,
};

pub struct TraceManager {
    config: TracerConfig,
    store: Arc<Mutex<TraceStore>>,
    ready_rx: mpsc::Receiver<bool>,
    js: JoinSet<()>,
}

impl TraceManager {
    pub async fn start(client: kube::Client, config: TracerConfig) -> anyhow::Result<Self> {
        let mut apiset = DynamicApiSet::new(client.clone());

        let (ready_tx, ready_rx): (mpsc::Sender<bool>, mpsc::Receiver<bool>) =
            mpsc::channel(config.tracked_objects.len() + 1);
        let (dyn_obj_tx, dyn_obj_rx): (dyn_obj_watcher::Sender, dyn_obj_watcher::Receiver) = mpsc::unbounded_channel();
        let (pod_tx, pod_rx): (pod_watcher::Sender, pod_watcher::Receiver) = mpsc::unbounded_channel();

        let mut js = JoinSet::new();
        for gvk in config.tracked_objects.keys() {
            let watcher =
                dyn_obj_watcher::new_with_stream(gvk, &mut apiset, dyn_obj_tx.clone(), ready_tx.clone()).await?;
            js.spawn(watcher.start());
        }

        let pw = pod_watcher::new_with_stream(client.clone(), pod_tx, ready_tx.clone())?;
        js.spawn(pw.start());

        let store = Arc::new(Mutex::new(TraceStore::new(config.clone(), apiset)));
        js.spawn(handle_messages(dyn_obj_rx, pod_rx, store.clone()));

        Ok(TraceManager { config, store, ready_rx, js })
    }

    pub fn get_store(&self) -> Arc<Mutex<TraceStore>> {
        self.store.clone()
    }

    pub async fn shutdown(&mut self) {
        self.js.shutdown().await;
    }

    pub async fn wait_ready(&mut self) {
        for _ in 0..self.config.tracked_objects.len() + 1 {
            let _ = self.ready_rx.recv().await;
        }
    }
}

pub(crate) async fn handle_messages(
    mut dyn_obj_rx: dyn_obj_watcher::Receiver,
    mut pod_rx: pod_watcher::Receiver,
    m_store: Arc<Mutex<TraceStore>>,
) -> () {
    loop {
        tokio::select! {
            Some(request) = dyn_obj_rx.recv() => {
                let mut store = m_store.lock().await;
                let res = match request.action {
                    TraceAction::ObjectApplied => store.create_or_update_obj(&request.obj, request.ts),
                    TraceAction::ObjectDeleted => store.delete_obj(&request.obj, request.ts),
                };
                if let Err(err) = res {
                    error!("could not send dynamic object update for {request:?}: {err}");
                }
            },
            Some(request) = pod_rx.recv() => {
                let mut store = m_store.lock().await;
                if let Err(err) = store.record_pod_lifecycle(&request.ns_name, &request.maybe_pod, &request.lifecycle_data).await {
                    error!("could not send dynamic object update for {request:?}: {err}");
                }
            },
            else => break,
        }
    }
}
