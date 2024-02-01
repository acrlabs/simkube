use kube::CustomResourceExt;
use simkube::prelude::*;

pub fn cmd() -> EmptyResult {
    print!("---\n{}", serde_yaml::to_string(&Simulation::crd())?);
    print!("---\n{}", serde_yaml::to_string(&SimulationRoot::crd())?);

    Ok(())
}
