use std::sync::Arc;

use sk_api::v1::Simulation;
use sk_core::prelude::*;

use crate::Options;

#[derive(Clone)]
pub struct SimulationContext {
    pub client: kube::Client,
    pub opts: Options,

    pub name: String,
    pub metaroot_name: String,
    pub driver_name: String,
    pub driver_svc: String,
    pub prometheus_name: String,
    pub prometheus_svc: String,
    pub webhook_name: String,
}

impl SimulationContext {
    pub fn new(client: kube::Client, opts: Options) -> SimulationContext {
        SimulationContext {
            client,
            opts,

            name: String::new(),
            metaroot_name: String::new(),
            driver_name: String::new(),
            driver_svc: String::new(),
            prometheus_name: String::new(),
            prometheus_svc: String::new(),
            webhook_name: String::new(),
        }
    }

    pub fn with_sim(self: Arc<Self>, sim: &Simulation) -> Self {
        let mut new = (*self).clone();
        new.name = sim.name_any();
        new.metaroot_name = format!("sk-{}-metaroot", new.name);
        new.driver_name = format!("sk-{}-driver", new.name);
        new.driver_svc = format!("sk-{}-driver-svc", new.name);
        new.prometheus_name = format!("sk-{}-prom", new.name);
        new.prometheus_svc = format!("sk-{}-prom-svc", new.name);
        new.webhook_name = format!("sk-{}-mutatepods", new.name);

        new
    }
}
