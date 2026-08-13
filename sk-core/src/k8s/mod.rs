mod apiset;
mod container_state;
mod gvk;
mod lease;
mod owners;
mod pod_ext;
mod pod_lifecycle;
mod resource;
mod sim;
mod util;

pub use apiset::*;
pub use gvk::*;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
pub use lease::*;
pub use owners::OwnersCache;
pub use resource::*;
use serde::{
    Deserialize,
    Serialize,
};
pub use sim::*;
pub use util::*;

use crate::errors::*;
use crate::macros::partial_ord_eq_ref;

err_impl! {KubernetesError,
    #[error("field not found in struct: {0}")]
    FieldNotFound(String),

    #[error("lease has different owner: {0}")]
    LeaseHeldByOther(String),

    #[error("malformed container status: {0:?}")]
    MalformedContainerState(corev1::ContainerState),

    #[error("malformed label selector: {0:?}")]
    MalformedLabelSelector(metav1::LabelSelectorRequirement),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PodLifecycleData {
    Empty,
    Running(i64),
    Finished(i64, i64),
}
partial_ord_eq_ref!(PodLifecycleData);

pub trait PodExt {
    fn labels_contains_key(&self, key: &str) -> bool;
    fn spec(&self) -> anyhow::Result<&corev1::PodSpec>;
    fn stable_spec(&self) -> anyhow::Result<corev1::PodSpec>;
    fn status(&self) -> anyhow::Result<&corev1::PodStatus>;
}

trait StartEndTimeable {
    fn start_ts(&self) -> anyhow::Result<Option<i64>>;
    fn end_ts(&self) -> anyhow::Result<Option<i64>>;
}

#[cfg(test)]
pub mod tests;
