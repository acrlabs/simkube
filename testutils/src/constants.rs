use lazy_static::lazy_static;
use sk_core::k8s::GVK;

pub const EMPTY_POD_SPEC_HASH: u64 = 17506812802394981455;
pub const TEST_DEPLOYMENT: &str = "the-deployment";
pub const TEST_DAEMONSET: &str = "the-daemonset";
pub const TEST_SERVICE_ACCOUNT: &str = "the-service-account";
pub const TEST_NAMESPACE: &str = "test-namespace";
pub const TEST_SIM_NAME: &str = "test-sim";
pub const TEST_SIM_ROOT_NAME: &str = "test-sim-root";
pub const TEST_DRIVER_NAME: &str = "sk-test-driver-12345";
pub const TEST_DRIVER_ROOT_NAME: &str = "sk-test-driver-12345-root";
pub const TEST_VIRT_NS_PREFIX: &str = "virt-test";
pub const TEST_CTRL_NAMESPACE: &str = "ctrl-ns";

lazy_static! {
    pub static ref DEPL_GVK: GVK = GVK::new("apps", "v1", "Deployment");
    pub static ref DS_GVK: GVK = GVK::new("apps", "v1", "DaemonSet");
}
