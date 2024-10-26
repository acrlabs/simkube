// Well-known labels, annotations, and taints
pub const KUBERNETES_IO_METADATA_NAME_KEY: &str = "kubernetes.io/metadata.name";
pub const APP_KUBERNETES_IO_NAME_KEY: &str = "app.kubernetes.io/name";
pub const APP_KUBERNETES_IO_COMPONENT_KEY: &str = "app.kubernetes.io/component";

// Common annotations and labels for SimKube
pub const LIFETIME_ANNOTATION_KEY: &str = "simkube.io/lifetime-seconds";
pub const ORIG_NAMESPACE_ANNOTATION_KEY: &str = "simkube.io/original-namespace";
pub const SIMULATION_LABEL_KEY: &str = "simkube.io/simulation";
pub const VIRTUAL_LABEL_KEY: &str = "simkube.io/virtual";
pub const PROM2PARQUET_PREFIX_KEY: &str = "prom2parquet_prefix";

// Taint/toleration key
pub const VIRTUAL_NODE_TOLERATION_KEY: &str = "kwok-provider";

// Env vars
pub const CTRL_NS_ENV_VAR: &str = "CTRL_NAMESPACE";
pub const DRIVER_NAME_ENV_VAR: &str = "DRIVER_NAME";
pub const POD_SVC_ACCOUNT_ENV_VAR: &str = "POD_SVC_ACCOUNT";

// Defaults
pub const DEFAULT_METRICS_NS: &str = "monitoring";
pub const DEFAULT_METRICS_SVC_ACCOUNT: &str = "prometheus-k8s";
pub const DRIVER_ADMISSION_WEBHOOK_PORT: &str = "8888";
pub const SK_LEASE_NAME: &str = "sk-lease";

// Timing
pub const RETRY_DELAY_SECONDS: u64 = 5;
pub const ERROR_RETRY_DELAY_SECONDS: u64 = 30;

#[cfg(feature = "testutils")]
mod test_constants {
    use lazy_static::lazy_static;

    use crate::k8s::GVK;

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
        pub static ref SA_GVK: GVK = GVK::new("", "v1", "ServiceAccount");
    }
}

#[cfg(feature = "testutils")]
pub use test_constants::*;
