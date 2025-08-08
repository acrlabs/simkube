use std::sync::{
    Arc,
    Mutex,
};

use kube::Client;
use sk_core::k8s::DynamicApiSet;
use sk_core::prelude::*;
use tokio::sync::mpsc;
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

    ready_tx: mpsc::Sender<bool>,
    ready_rx: mpsc::Receiver<bool>,

    js: JoinSet<()>,
}

impl TraceManager {
    pub fn new(config: TracerConfig, store: Arc<Mutex<TraceStore>>) -> Self {
        let (ready_tx, ready_rx): (mpsc::Sender<bool>, mpsc::Receiver<bool>) =
            mpsc::channel(config.tracked_objects.len());

        TraceManager {
            config,
            store,
            ready_tx,
            ready_rx,
            js: JoinSet::new(),
        }
    }

    pub async fn start(&mut self) -> EmptyResult {
        let client = Client::try_default().await.expect("failed to create kube client");
        let mut apiset = DynamicApiSet::new(client.clone());

        let (dyn_obj_tx, dyn_obj_rx): (dyn_obj_watcher::Sender, dyn_obj_watcher::Receiver) = mpsc::unbounded_channel();
        let (pod_tx, pod_rx): (pod_watcher::Sender, pod_watcher::Receiver) = mpsc::unbounded_channel();
        self.js.spawn(handle_messages(dyn_obj_rx, pod_rx, self.store.clone()));

        for gvk in self.config.tracked_objects.keys() {
            let do_watcher =
                dyn_obj_watcher::new_with_stream(gvk, &mut apiset, dyn_obj_tx.clone(), self.ready_tx.clone()).await?;
            self.js.spawn(do_watcher.start());
        }

        let pw = pod_watcher::new_with_stream(client, apiset, pod_tx.clone(), self.ready_tx.clone())?;
        self.js.spawn(pw.start());

        // Can't use .join_all here because it takes ownership of the joinset
        while let Some(_) = self.js.join_next().await {}

        Ok(())
    }

    pub async fn wait_ready(&mut self) {
        for _ in 0..self.config.tracked_objects.len() + 1 {
            let _ = self.ready_rx.recv();
        }
    }

    pub async fn shutdown(&mut self) {
        self.js.shutdown().await;
    }
}

pub(crate) async fn handle_messages(
    mut dyn_obj_rx: dyn_obj_watcher::Receiver,
    mut pod_rx: pod_watcher::Receiver,
    store_m: Arc<Mutex<TraceStore>>,
) -> () {
    loop {
        tokio::select! {
            Some(request) = dyn_obj_rx.recv() => {
                let mut store = store_m.lock().unwrap();
                let res = match request.action {
                    TraceAction::ObjectApplied => store.create_or_update_obj(&request.obj, request.ts),
                    TraceAction::ObjectDeleted => store.delete_obj(&request.obj, request.ts),
                };
                if let Err(err) = res {
                    error!("could not send dynamic object update for {request:?}: {err}");
                }
            },
            Some(request) = pod_rx.recv() => {
                let mut store = store_m.lock().unwrap();
                if let Err(err) = store.record_pod_lifecycle(&request.ns_name, &request.maybe_pod, &request.owners, &request.lifecycle_data) {
                    error!("could not send dynamic object update for {request:?}: {err}");
                }
            },
            else => break,
        }
    }
}
