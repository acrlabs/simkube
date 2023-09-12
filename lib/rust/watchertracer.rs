mod trace_filter;
mod tracer;
mod watcher;

use std::sync::{
    Arc,
    Mutex,
};

use crate::config::TracerConfig;
pub use crate::watchertracer::trace_filter::TraceFilter;
pub use crate::watchertracer::tracer::{
    TraceEvent,
    Tracer,
};
pub use crate::watchertracer::watcher::{
    PodStream,
    Watcher,
};

pub async fn new_watcher_tracer(config: &TracerConfig, client: kube::Client) -> (Watcher, Arc<Mutex<Tracer>>) {
    let tracer = Tracer::new();
    return (Watcher::new(client, tracer.clone(), config).await, tracer);
}

#[cfg(test)]
mod tracer_test;
