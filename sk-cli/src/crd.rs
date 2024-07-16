use kube::CustomResourceExt;
use sk_core::prelude::*;

pub fn cmd() -> EmptyResult {
    print!("---\n{}", serde_yaml::to_string(&Simulation::crd())?);
    print!("---\n{}", serde_yaml::to_string(&SimulationRoot::crd())?);

    Ok(())
}
