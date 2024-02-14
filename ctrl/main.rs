mod cert_manager;
mod controller;
mod metrics;
mod objects;
mod trace;

use std::ops::Deref;
use std::sync::Arc;

use clap::Parser;
use futures::{
    future,
    StreamExt,
    TryStreamExt,
};
use k8s_openapi::api::batch::v1 as batchv1;
use kube::runtime::controller::Controller;
use kube::runtime::{
    reflector,
    watcher,
    WatchStreamExt,
};
use kube::ResourceExt;
use simkube::errors::*;
use simkube::prelude::*;

use crate::controller::{
    error_policy,
    reconcile,
};
use crate::objects::*;

#[derive(Clone, Debug, Parser)]
struct Options {
    #[arg(long)]
    driver_image: String,

    #[arg(long, default_value = DRIVER_ADMISSION_WEBHOOK_PORT)]
    driver_port: i32,

    // TODO: should support non-cert-manager for configuring certs as well
    #[arg(long)]
    use_cert_manager: bool,

    #[arg(long, default_value = "")]
    cert_manager_issuer: String,

    #[arg(short, long, default_value = "info")]
    verbosity: String,
}

// This is sortof a stupid hack, because anyhow::Error doesn't derive from
// std::error::Error, but the reconcile functions require you to return a
// result that derives from std::error::Error.  So we just wrap the anyhow,
// and then implement deref for it so we can get back to the underlying error
// wherever we actually care.
#[derive(Debug, Error)]
#[error(transparent)]
struct AnyhowError(#[from] anyhow::Error);

impl Deref for AnyhowError {
    type Target = anyhow::Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

err_impl! {SkControllerError,
    #[error("configmap {0} not found")]
    ConfigmapNotFound(String),

    #[error("missing status field: {0}")]
    MissingStatusField(String),

    #[error("namespace {0} not found")]
    NamespaceNotFound(String),
}

#[derive(Clone)]
struct SimulationContext {
    client: kube::Client,
    opts: Options,

    name: String,
    root: String,
    driver_ns: String,
    driver_name: String,
    driver_svc: String,
    prometheus_name: String,
    prometheus_svc: String,
    webhook_name: String,
}

impl SimulationContext {
    fn new(client: kube::Client, opts: Options) -> SimulationContext {
        SimulationContext {
            client,
            opts,
            name: String::new(),
            root: String::new(),
            driver_ns: String::new(),
            driver_name: String::new(),
            driver_svc: String::new(),
            prometheus_name: String::new(),
            prometheus_svc: String::new(),
            webhook_name: String::new(),
        }
    }

    fn with_sim(self: Arc<Self>, sim: &Simulation) -> Self {
        let mut new = (*self).clone();
        new.name = sim.name_any();
        new.root = format!("sk-{}-root", new.name);
        new.driver_name = format!("sk-{}-driver", new.name);
        new.driver_ns = sim.spec.driver_namespace.clone();
        new.driver_svc = format!("sk-{}-driver-svc", new.name);
        new.prometheus_name = format!("sk-{}-prom", new.name);
        new.prometheus_svc = format!("sk-{}-prom-svc", new.name);
        new.webhook_name = format!("sk-{}-mutatepods", new.name);

        new
    }
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

    let ctrl = Controller::for_stream(sim_stream, reader)
        .owns(job_api, Default::default())
        .run(reconcile, error_policy, Arc::new(SimulationContext::new(client, opts)))
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
