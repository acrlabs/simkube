pub const DRIVER_ADMISSION_WEBHOOK_PORT: &str = "8888";

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

// Defaults
pub const DEFAULT_METRICS_NS: &str = "monitoring";
pub const DEFAULT_METRICS_SVC_ACCOUNT: &str = "prometheus-k8s";

// Misc
pub const METRIC_CONFIG_MAP_QUERY_KEY: &str = "queries";
