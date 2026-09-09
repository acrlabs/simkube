use std::collections::HashMap;
use std::mem::take;
use std::time::Duration;

use futures::{
    StreamExt,
    TryStreamExt,
};
use kube::runtime::watcher::{
    Event,
    watcher,
};
use sk_core::errors::*;
use sk_core::k8s::internal_node_ip;
use sk_core::prelude::*;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;
use tracing::*;

use super::*;

impl Collector {
    pub fn new(
        client: kube::Client,
        config: &TracerConfig,
        service_account_token: String,
        metrics_tx: Sender,
        ready_tx: mpsc::Sender<bool>,
    ) -> anyhow::Result<Self> {
        let node_api: kube::Api<corev1::Node> = kube::Api::all(client);
        let node_stream = watcher(node_api.clone(), Default::default()).map_err(|e| e.into()).boxed();
        let http_client = reqwest::ClientBuilder::new()
            // this-is-fine-dot-jpeg (no really it's fine we just need to read from the kubelet endpoint)
            .danger_accept_invalid_certs(true)
            .build()?;

        Ok(Collector {
            http_client,
            metrics_tx,
            node_scrapers: HashMap::new(),
            node_stream,
            scrape_interval: Duration::from_secs(config.metrics.scrape_interval_seconds),

            service_account_token,

            init_node_buffer: HashMap::new(),
            is_ready: false,
            ready_tx,
        })
    }

    // This is not a reference because it needs to "own" itself when tokio spawns it
    pub async fn start(mut self) {
        info!("Spawning node metrics watcher");
        while let Some(res) = self.node_stream.next().await {
            match res {
                Ok(ref evt) => self.handle_node_event(evt).await.unwrap_or_else(|err| {
                    // This error is "sortof" OK, in the sense that if we can't handle a single
                    // node, the collector can potentially keep going on other events, so we don't
                    // display a stack trace here.
                    error!("could not handle event:\n\n{err}\n");
                }),
                Err(err) => {
                    // However, if there's a fundamental error getting something from the stream,
                    // the tracer can still maybe attempt to keep going, but that indicates
                    // somthing more problematic and program-stopping is going on, so we display a
                    // stack trace (using skerr).
                    skerr!(err, "watcher received error on stream");
                },
            }
        }
    }

    pub(crate) async fn handle_node_event(&mut self, evt: &Event<corev1::Node>) -> EmptyResult {
        match evt {
            Event::Apply(node) => {
                let Some(node_ip) = internal_node_ip(node) else {
                    bail!("no IP address found for node: {node:?}");
                };
                let handle = self.run_scraper_for_node(&node_ip);
                self.node_scrapers.insert(node.name_any(), handle);
            },
            Event::Delete(node) => {
                self.node_scrapers.remove(&node.name_any());
            },
            Event::Init => (),
            Event::InitApply(node) => {
                let Some(node_ip) = internal_node_ip(node) else {
                    bail!("no IP address found for node: {node:?}");
                };
                self.init_node_buffer.insert(node.name_any(), node_ip);
            },
            Event::InitDone => {
                // It is extremely tempting to try to do a set-difference approach here, but that
                // turns out to be impractical and kindof ugly because we need to collect both the
                // node name and other metadata (e.g., node_ip) during the initialization phase, and
                // converting those hashmaps into sets that you can then do a difference on is a
                // pain.
                //
                // It is also also extremely tempting to try to do something like
                //
                //     for node in self.node_scrapers
                //         if node not in self.init_node_buffer
                //             self.node_scrapers.remove(node)
                //
                // but this is infeasible because you're modifying the thing you're iterating over.
                //
                // It is ALSO also also extremely tempting to use tokio_util::task::JoinMap, but
                // that is infeasible because it requires the tasks to be 'static which these are
                // not (I _think_ because the tasks are owned by a non-static object, aka this
                // Collector).
                let mut old_node_scrapers = take(&mut self.node_scrapers);
                for (node, node_ip) in &self.init_node_buffer {
                    let handle = old_node_scrapers
                        .remove(node)
                        .unwrap_or_else(|| self.run_scraper_for_node(node_ip));
                    self.node_scrapers.insert(node.into(), handle);
                }

                // When the watcher first starts up it does a List call, which (internally) gets
                // converted into a "Restarted" event that contains all of the listed objects.
                // Once we've handled this event the first time, we know we have a complete view of
                // the cluster at startup time.
                if !self.is_ready {
                    self.is_ready = true;

                    // if nobody's listening on the other end it's "fine" so we ignore the error
                    if let Err(e) = self.ready_tx.send(true).await {
                        error!("failed to notify ready: {e}");
                    }
                }
                self.init_node_buffer.clear();
            },
        }
        Ok(())
    }

    fn run_scraper_for_node(&self, node_ip: &str) -> AbortOnDropHandle<EmptyResult> {
        let target_url = format!("https://{node_ip}:{KUBELET_PORT}/metrics/cadvisor");
        let scraper = MetricScraper::new(
            self.http_client.clone(),
            target_url,
            UTILIZATION_METRICS,
            self.scrape_interval,
            self.service_account_token.clone(),
            self.metrics_tx.clone(),
        );
        AbortOnDropHandle::new(tokio::spawn(async move { scraper.start().await }))
    }
}
