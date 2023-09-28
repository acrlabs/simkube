mod apiset;
mod container;
mod gvk;
pub mod macros;
mod pod;
mod pod_lifecycle;
mod util;

pub use apiset::*;
pub use gvk::*;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
pub use util::*;

use crate::errors::*;
use crate::macros::partial_ord_eq_ref;

const LAST_APPLIED_CONFIG_LABEL_KEY: &str = "kubectl.kubernetes.io/last-applied-configuration";
const DEPL_REVISION_LABEL_KEY: &str = "deployment.kubernetes.io/revision";

err_impl! {KubernetesError,
    #[error("field not found in struct: {0}")]
    FieldNotFound(String),

    #[error("malformed container status: {0:?}")]
    MalformedContainerState(corev1::ContainerState),

    #[error("malformed label selector: {0:?}")]
    MalformedLabelSelector(metav1::LabelSelectorRequirement),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PodLifecycleData {
    Empty,
    Running(i64),
    Finished(i64, i64),
}
partial_ord_eq_ref!(PodLifecycleData);

pub trait KubeResourceExt {
    fn namespaced_name(&self) -> String;
    fn matches(&self, sel: &metav1::LabelSelector) -> anyhow::Result<bool>;
}

pub trait PodExt {
    fn spec(&self) -> anyhow::Result<&corev1::PodSpec>;
    fn status(&self) -> anyhow::Result<&corev1::PodStatus>;
    fn spec_mut(&mut self) -> &mut corev1::PodSpec;
    fn status_mut(&mut self) -> &mut corev1::PodStatus;
}

trait StartEndTimeable {
    fn start_ts(&self) -> anyhow::Result<Option<i64>>;
    fn end_ts(&self) -> anyhow::Result<Option<i64>>;
}

#[cfg(test)]
mod test;
