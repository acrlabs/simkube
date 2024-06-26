pub mod clock;
pub mod fake;
pub mod pods;
pub mod sim;
pub mod store;

pub const EMPTY_OBJ_HASH: u64 = 15130871412783076140;
pub const EMPTY_POD_SPEC_HASH: u64 = 17506812802394981455;
pub const TEST_DEPLOYMENT: &str = "the-deployment";
pub const TEST_NAMESPACE: &str = "test";
pub const TEST_SIM_NAME: &str = "test-sim";
pub const TEST_SIM_ROOT_NAME: &str = "test-sim-root";
pub const TEST_DRIVER_NAME: &str = "sk-test-driver-12345";
pub const TEST_DRIVER_ROOT_NAME: &str = "sk-test-driver-12345-root";
pub const TEST_VIRT_NS_PREFIX: &str = "virt-test";
pub const TEST_CTRL_NAMESPACE: &str = "ctrl-ns";

pub use clock::MockUtcClock;
pub use fake::{
    apps_v1_discovery,
    make_fake_apiserver,
    status_not_found,
    status_ok,
    MockServerBuilder,
};
pub use pods::test_pod;
use rstest::*;
pub use sim::{
    test_sim,
    test_sim_root,
};
pub use store::MockTraceStore;
