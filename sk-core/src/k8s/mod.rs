mod apiset;
mod container_state;
mod gvk;
mod lease;
mod owners;
mod pod_ext;
mod pod_lifecycle;
mod sim;
mod util;

pub use apiset::*;
pub use gvk::*;
pub use lease::*;
pub use owners::OwnersCache;
use serde::{
    Deserialize,
    Serialize,
};
pub use sim::*;
pub use util::*;

use crate::errors::*;
use crate::macros::partial_ord_eq_ref;
use crate::prelude::*;

const LAST_APPLIED_CONFIG_LABEL_KEY: &str = "kubectl.kubernetes.io/last-applied-configuration";
const DEPL_REVISION_LABEL_KEY: &str = "deployment.kubernetes.io/revision";

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

pub trait KubeResourceExt {
    fn namespaced_name(&self) -> String;
    fn matches(&self, sel: &metav1::LabelSelector) -> anyhow::Result<bool>;
}

pub trait PodExt {
    fn spec(&self) -> anyhow::Result<&corev1::PodSpec>;
    fn stable_spec(&self) -> anyhow::Result<corev1::PodSpec>;
    fn status(&self) -> anyhow::Result<&corev1::PodStatus>;
}

pub trait OpenApiResourceExt {
    fn type_meta() -> TypeMeta;
}

impl<T: k8s_openapi::Resource> OpenApiResourceExt for T {
    fn type_meta() -> TypeMeta {
        TypeMeta {
            api_version: T::API_VERSION.into(),
            kind: T::KIND.into(),
        }
    }
}

trait StartEndTimeable {
    fn start_ts(&self) -> anyhow::Result<Option<i64>>;
    fn end_ts(&self) -> anyhow::Result<Option<i64>>;
}

#[cfg(test)]
pub mod tests;
