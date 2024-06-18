pub mod hooks;

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

#[cfg(test)]
mod tests;
