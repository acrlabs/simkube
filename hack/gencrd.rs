use kube::CustomResourceExt;
use simkube::prelude::*;

fn main() {
    println!("---");
    println!("{}", serde_yaml::to_string(&Simulation::crd()).unwrap());
    println!("---");
    println!("{}", serde_yaml::to_string(&SimulationRoot::crd()).unwrap());
}
