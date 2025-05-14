use lazy_static::lazy_static;

use crate::k8s::GVK;

// Well-known labels, annotations, and taints
pub const KUBERNETES_IO_METADATA_NAME_KEY: &str = "kubernetes.io/metadata.name";
pub const APP_KUBERNETES_IO_NAME_KEY: &str = "app.kubernetes.io/name";
pub const APP_KUBERNETES_IO_COMPONENT_KEY: &str = "app.kubernetes.io/component";

// Common annotations and labels for SimKube
pub const ORIG_NAMESPACE_ANNOTATION_KEY: &str = "simkube.io/original-namespace";
pub const SIMULATION_LABEL_KEY: &str = "simkube.io/simulation";
pub const VIRTUAL_LABEL_KEY: &str = "simkube.io/virtual";
pub const POD_SPEC_STABLE_HASH_KEY: &str = "simkube.io/pod-spec-stable-hash";
pub const POD_SEQUENCE_NUMBER_KEY: &str = "simkube.io/pod-sequence-number";
pub const SKIP_LOCAL_VOLUME_MOUNT_ANNOTATION_KEY: &str = "simkube.io/skip-local-volue-mount";

// Lifecycle management labels and annotations
pub const KWOK_STAGE_COMPLETE_KEY: &str = "simkube.kwok.io/stage-complete";
pub const KWOK_STAGE_COMPLETE_TIMESTAMP_KEY: &str = "simkube.kwok.io/stage-complete-time";
pub const KWOK_STAGE_ERROR_TIMESTAMP_KEY: &str = "simkube.kwok.io/stage-error-time";

// Metrics
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

// Kinds
pub const SVC_ACCOUNT_KIND: &str = "ServiceAccount";

// Built-in GVKs
lazy_static! {
    pub static ref SVC_ACCOUNT_GVK: GVK = GVK::new("", "v1", SVC_ACCOUNT_KIND);
}
