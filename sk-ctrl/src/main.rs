mod cert_manager;
mod controller;
mod errors;
mod objects;

use std::sync::Arc;

use clap::Parser;
use futures::{
    StreamExt,
    TryStreamExt,
    future,
};
use k8s_openapi::api::batch::v1 as batchv1;
use kube::runtime::controller::Controller;
use kube::runtime::{
    WatchStreamExt,
    reflector,
    watcher,
};
use sk_core::logging;
use sk_core::prelude::*;
use tracing::*;

use crate::controller::{
    error_policy,
    reconcile,
};

#[derive(Clone, Debug, Default, Parser)]
struct Options {
    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

struct GlobalContext {
    client: kube::Client,
    recorder: SkEventRecorder,
}

#[instrument(ret, err)]
async fn run(opts: Options) -> EmptyResult {
    let client = kube::Client::try_default().await?;
    let sim_api = kube::Api::<Simulation>::all(client.clone());
    let job_api = kube::Api::<batchv1::Job>::all(client.clone());

    let (reader, writer) = reflector::store();
    let sim_stream = watcher(sim_api, Default::default())
        .default_backoff()
        .reflect(writer)
        .applied_objects()
        .try_filter(|evt| {
            future::ready(
                // Use the "observed generation" field to filter out status updates
                //
                // This conceivably could cause the controller to miss some things if somehow
                // one or the other of the "default"/unwrap_or values gets injected in the
                // wrong place.  Guess we'll see if that happens.
                //
                // I'm not using the predicate::generation filter because this causes the
                // controller to miss events if I delete and recreate the same object.
                evt.status.as_ref().unwrap_or(&Default::default()).observed_generation
                    != evt.metadata.generation.unwrap_or(1),
            )
        });

    let ctx = GlobalContext {
        client: client.clone(),
        recorder: SkEventRecorder::new(client.clone(), "sk-ctrl".into()),
    };
    let ctrl = Controller::for_stream(sim_stream, reader)
        .owns(job_api, Default::default())
        .run(reconcile, error_policy, Arc::new(ctx))
        .for_each(|_| future::ready(()));

    ctrl.await;
    Ok(())
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = Options::parse();
    logging::setup(&args.verbosity);
    run(args).await
}

#[cfg(test)]
mod tests;
