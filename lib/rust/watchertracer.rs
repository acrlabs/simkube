use std::sync::{
    Arc,
    Mutex,
};

use crate::config::TracerConfig;
use crate::prelude::*;
use crate::trace::Tracer;
use crate::watch::Watcher;

pub async fn new_watcher_tracer(
    config: &TracerConfig,
    client: kube::Client,
) -> SimKubeResult<(Watcher, Arc<Mutex<Tracer>>)> {
    let tracer = Tracer::new();
    Ok((Watcher::new(client, tracer.clone(), config).await?, tracer))
}
