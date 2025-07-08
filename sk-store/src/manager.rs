use std::sync::mpsc::{
    Receiver,
    Sender,
};
use std::sync::{
    Arc,
    Mutex,
    mpsc,
};

use kube::Client;
use sk_api::v1::ExportFilters;
use sk_core::k8s::DynamicApiSet;
use sk_core::prelude::*;
use tokio::task::JoinSet;

use crate::config::TracerConfig;
use crate::store::TraceStore;
use crate::watchers::{
    dyn_obj_watcher,
    pod_watcher,
};

#[derive(Clone)]
pub struct TraceManager {
    store: Arc<Mutex<TraceStore>>,
    config: TracerConfig,

    ready_tx: Sender<bool>,
    ready_rx: Arc<Mutex<Receiver<bool>>>,
    js: Arc<Mutex<JoinSet<()>>>,
}

impl TraceManager {
    pub fn new(config: TracerConfig) -> Self {
        let store = Arc::new(Mutex::new(TraceStore::new(config.clone())));
        let (ready_tx, ready_rx): (Sender<bool>, Receiver<bool>) = mpsc::channel();

        TraceManager {
            store,
            config,

            ready_tx,
            ready_rx: Arc::new(Mutex::new(ready_rx)),
            js: Arc::new(Mutex::new(JoinSet::new())),
        }
    }

    pub async fn start(&mut self) -> EmptyResult {
        let client = Client::try_default().await.expect("failed to create kube client");
        let mut apiset = DynamicApiSet::new(client.clone());

        let mut js = JoinSet::new();
        for gvk in self.config.tracked_objects.keys() {
            let do_watcher =
                dyn_obj_watcher::new_with_stream(gvk, &mut apiset, self.store.clone(), self.ready_tx.clone()).await?;
            js.spawn(do_watcher.start());
        }

        let pw = pod_watcher::new_with_stream(client, apiset, self.store.clone(), self.ready_tx.clone())?;
        js.spawn(pw.start());
        js.join_all().await;
        Ok(())
    }

    pub async fn wait_ready(&self) {
        let ready_rx = self.ready_rx.lock().unwrap();
        let len = { self.js.lock().unwrap().len() }; // Drop the lock on this once we're done
        for _ in 0..len {
            let _ = ready_rx.recv();
        }
    }

    pub async fn shutdown(&mut self) {
        self.js.lock().unwrap().shutdown().await;
    }

    pub async fn export(&self, start_ts: i64, end_ts: i64, filter: &ExportFilters) -> anyhow::Result<Vec<u8>> {
        self.store.lock().unwrap().export(start_ts, end_ts, filter)
    }
}
