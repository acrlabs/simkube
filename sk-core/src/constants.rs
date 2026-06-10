use std::collections::HashMap;
use std::sync::LazyLock;

use k8s_openapi::api::apps::v1 as appsv1;
use k8s_openapi::api::batch::v1 as batchv1;
use k8s_openapi::api::core::v1 as corev1;

use crate::k8s::{
    GVK,
    OpenApiResourceExt,
};

// Well-known labels, annotations, and taints
pub const KUBERNETES_IO_METADATA_NAME_KEY: &str = "kubernetes.io/metadata.name";
pub const APP_KUBERNETES_IO_NAME_KEY: &str = "app.kubernetes.io/name";
pub const APP_KUBERNETES_IO_COMPONENT_KEY: &str = "app.kubernetes.io/component";

// Common annotations and labels for SimKube
pub const ORIG_NAMESPACE_ANNOTATION_KEY: &str = "simkube.io/original-namespace";
pub const SIMULATION_LABEL_KEY: &str = "simkube.io/simulation";
pub const SKIP_LOCAL_VOLUME_MOUNT_ANNOTATION_KEY: &str = "simkube.io/skip-local-volume-mount";
pub const POD_SPEC_STABLE_HASH_KEY: &str = "simkube.io/pod-spec-stable-hash";
pub const POD_SEQUENCE_NUMBER_KEY: &str = "simkube.io/pod-sequence-number";
pub const VIRTUAL_LABEL_KEY: &str = "simkube.io/virtual";

// Lifecycle management labels and annotations
pub const KWOK_STAGE_COMPLETE_KEY: &str = "simkube.io/kwok-stage-complete";
pub const KWOK_STAGE_COMPLETE_TIMESTAMP_KEY: &str = "simkube.io/kwok-stage-complete-time";
pub const KWOK_STAGE_ERROR_TIMESTAMP_KEY: &str = "simkube.io/kwok-stage-error-time";
pub const KWOK_STAGE_CREATE_DELAY_KEY: &str = "simkube.io/kwok-stage-create-delay";
pub const KWOK_STAGE_CREATE_DELAY_JITTER_KEY: &str = "simkube.io/kwok-stage-create-delay-jitter";
pub const KWOK_STAGE_READY_DELAY_KEY: &str = "simkube.io/kwok-stage-ready-delay";
pub const KWOK_STAGE_READY_DELAY_JITTER_KEY: &str = "simkube.io/kwok-stage-ready-delay-jitter";

// Metrics
pub const PROM2PARQUET_PREFIX_KEY: &str = "prom2parquet_prefix";

// Certificates
pub const DRIVER_CERT_NAME: &str = "sk-driver-cert";
pub const CERT_MANAGER_INJECT_ANNOTATION_KEY: &str = "cert-manager.io/inject-ca-from";

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
pub static SVC_ACCOUNT_GVK: LazyLock<GVK> = LazyLock::new(|| GVK::new("", "v1", SVC_ACCOUNT_KIND));
pub static CRONJOB_GVK: LazyLock<GVK> = LazyLock::new(batchv1::CronJob::gvk);
pub static DAEMONSET_GVK: LazyLock<GVK> = LazyLock::new(appsv1::DaemonSet::gvk);
pub static DEPLOYMENT_GVK: LazyLock<GVK> = LazyLock::new(appsv1::Deployment::gvk);
pub static JOB_GVK: LazyLock<GVK> = LazyLock::new(batchv1::Job::gvk);
pub static REPLICASET_GVK: LazyLock<GVK> = LazyLock::new(appsv1::ReplicaSet::gvk);
pub static STATEFULSET_GVK: LazyLock<GVK> = LazyLock::new(appsv1::StatefulSet::gvk);
pub static POD_GVK: LazyLock<GVK> = LazyLock::new(corev1::Pod::gvk);

// Supported default podSpecTemplatePaths
pub static KNOWN_GVKS_METADATA: LazyLock<HashMap<GVK, Vec<&'static str>>> = LazyLock::new(|| {
    HashMap::from([
        (CRONJOB_GVK.clone(), vec!["/spec/jobTemplate/spec/template"]),
        (DAEMONSET_GVK.clone(), vec!["/spec/template"]),
        (DEPLOYMENT_GVK.clone(), vec!["/spec/template"]),
        (JOB_GVK.clone(), vec!["/spec/template"]),
        (REPLICASET_GVK.clone(), vec!["/spec"]),
        (STATEFULSET_GVK.clone(), vec!["/spec/template"]),
        (POD_GVK.clone(), vec![""]),
    ])
});
