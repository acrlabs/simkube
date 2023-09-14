use std::sync::{
    Arc,
    Mutex,
};

use crate::config::TracerConfig;
use crate::trace::Tracer;
use crate::watch::Watcher;

pub async fn new_watcher_tracer(
    config: &TracerConfig,
    client: kube::Client,
) -> anyhow::Result<(Watcher, Arc<Mutex<Tracer>>)> {
    let tracer = Tracer::new(config);
    let watcher = Watcher::new(client, tracer.clone(), config).await?;

    Ok((watcher, tracer))
}
