use sk_api::v1::{Simulation, SimulationMetricsConfig, SimulationRoot, SimulationRootSpec, SimulationState};

use crate::k8s::build_global_object_meta;
use crate::prelude::*;

pub fn metrics_ns(sim: &Simulation) -> String {
    match &sim.spec.metrics {
        Some(SimulationMetricsConfig { namespace: Some(ns), .. }) => ns.clone(),
        _ => DEFAULT_METRICS_NS.into(),
    }
}

pub fn metrics_svc_account(sim: &Simulation) -> String {
    match &sim.spec.metrics {
        Some(SimulationMetricsConfig { service_account: Some(sa), .. }) => sa.clone(),
        _ => DEFAULT_METRICS_SVC_ACCOUNT.into(),
    }
}

pub fn is_terminal(sim_state: &SimulationState) -> bool {
    matches!(sim_state, SimulationState::Finished | SimulationState::Failed)
}

pub fn build_simulation_root(name: &str, sim: &Simulation) -> SimulationRoot {
    let owner = sim;
    SimulationRoot {
        metadata: build_global_object_meta(name, &sim.name_any(), owner),
        spec: SimulationRootSpec {},
    }
}
