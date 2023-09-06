use kube::CustomResourceExt;
use simkube::prelude::*;

fn main() {
    println!("{}", serde_yaml::to_string(&Simulation::crd()).unwrap());
}
