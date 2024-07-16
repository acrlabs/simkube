use rstest::*;
use sk_api::v1::*;

use crate::prelude::*;

#[fixture]
pub fn test_sim() -> Simulation {
    Simulation {
        metadata: metav1::ObjectMeta {
            name: Some(TEST_SIM_NAME.into()),
            uid: Some("1234-asdf".into()),
            ..Default::default()
        },
        spec: SimulationSpec {
            driver: SimulationDriverConfig {
                namespace: TEST_NAMESPACE.into(),
                image: "docker.foo:1234/sk-driver:latest".into(),
                port: 9876,
                trace_path: "file:///foo/bar".into(),
            },
            metrics: Some(Default::default()),
            hooks: Some(SimulationHooksConfig {
                pre_start_hooks: Some(vec![SimulationHook {
                    cmd: "echo".into(),
                    args: vec!["foo".into()],
                    ..Default::default()
                }]),
                pre_run_hooks: Some(vec![SimulationHook {
                    cmd: "foo".into(),
                    args: vec!["bar".into(), "baz".into()],
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        },
        status: Default::default(),
    }
}

#[fixture]
pub fn test_sim_root() -> SimulationRoot {
    SimulationRoot {
        metadata: metav1::ObjectMeta {
            name: Some(format!("sk-{TEST_SIM_NAME}-root")),
            uid: Some("qwerty-5678".into()),
            ..Default::default()
        },
        spec: SimulationRootSpec {},
    }
}
