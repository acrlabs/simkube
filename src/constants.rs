pub const DRIVER_ADMISSION_WEBHOOK_PORT: &str = "8888";

// Common annotations and labels
pub const LIFETIME_ANNOTATION_KEY: &str = "simkube.io/lifetime-seconds";
pub const ORIG_NAMESPACE_ANNOTATION_KEY: &str = "simkube.io/original-namespace";
pub const SIMULATION_LABEL_KEY: &str = "simkube.io/simulation";
pub const VIRTUAL_LABEL_KEY: &str = "simkube.io/virtual";
pub const APP_KUBERNETES_IO_NAME_KEY: &str = "app.kubernetes.io/name";

// Taint/toleration key
pub const VIRTUAL_NODE_TOLERATION_KEY: &str = "kwok-provider";

// Defaults
pub const DEFAULT_MONITORING_NS: &str = "monitoring";
pub const DEFAULT_PROM_SVC_ACCOUNT: &str = "prometheus-k8s";
