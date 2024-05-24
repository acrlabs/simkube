use super::*;
use crate::api::v1::{
    SimulationHook,
    SimulationHooksConfig,
};
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
            driver_namespace: TEST_NAMESPACE.into(),
            trace_path: "file:///foo/bar".into(),
            metrics_config: Some(Default::default()),
            hooks: Some(SimulationHooksConfig {
                pre_start_hooks: Some(vec![SimulationHook { cmd: "echo".into(), args: vec!["foo".into()] }]),
                pre_run_hooks: Some(vec![SimulationHook {
                    cmd: "foo".into(),
                    args: vec!["bar".into(), "baz".into()],
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
