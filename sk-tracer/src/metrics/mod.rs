mod collector;
mod scraper;

use std::collections::HashMap;

use prometheus_parse::Sample;
use sk_core::prelude::*;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;

use crate::ObjStream;

pub type Sender = mpsc::UnboundedSender<Vec<Sample>>;
pub type Receiver = mpsc::UnboundedReceiver<Vec<Sample>>;

const KUBELET_PORT: u16 = 10250;
const UTILIZATION_METRICS: &[&str] = &[CONTAINER_CPU_USAGE_SECONDS_TOTAL, CONTAINER_MEMORY_WORKING_SET_BYTES];

pub(crate) struct Collector {
    http_client: reqwest::Client,
    metrics_tx: Sender,
    node_scrapers: HashMap<String, AbortOnDropHandle<EmptyResult>>,
    node_stream: ObjStream<corev1::Node>,
    scrape_interval_seconds: u64,

    service_account_token: String,

    init_node_buffer: HashMap<String, String>,
    is_ready: bool,
    ready_tx: mpsc::Sender<bool>,
}

pub(crate) struct MetricScraper<'a> {
    http_client: reqwest::Client,
    target_url: String,
    metric_names: &'a [&'a str],
    scrape_interval_seconds: u64,
    token: String,
    metrics_tx: Sender,
}

#[cfg(test)]
mod tests;
