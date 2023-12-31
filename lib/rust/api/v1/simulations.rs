// WARNING: generated by kopium - manual changes will be overwritten
// kopium command: kopium -f k8s/raw/simkube.io_simulations.yaml
// kopium version: 0.15.0

use kube::CustomResource;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug)]
#[kube(group = "simkube.io", version = "v1", kind = "Simulation", plural = "simulations")]
#[kube(status = "SimulationStatus")]
#[kube(schema = "disabled")]
pub struct SimulationSpec {
    #[serde(rename = "driverNamespace")]
    pub driver_namespace: String,
    pub trace: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimulationStatus {}
