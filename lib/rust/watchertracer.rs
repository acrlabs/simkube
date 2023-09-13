mod trace_filter;
mod tracer;
mod watch_event;
mod watcher;

use std::sync::{
    Arc,
    Mutex,
};

use crate::config::TracerConfig;
use crate::prelude::*;
pub use crate::watchertracer::trace_filter::TraceFilter;
pub use crate::watchertracer::tracer::{
    TraceEvent,
    Tracer,
};
pub use crate::watchertracer::watcher::{
    KubeObjectStream,
    Watcher,
};

pub async fn new_watcher_tracer(
    config: &TracerConfig,
    client: kube::Client,
) -> SimKubeResult<(Watcher, Arc<Mutex<Tracer>>)> {
    let tracer = Tracer::new();
    Ok((Watcher::new(client, tracer.clone(), config).await?, tracer))
}

#[cfg(test)]
mod tracer_test;
