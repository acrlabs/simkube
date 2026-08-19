use anyhow::bail;
use k8s_openapi::api::core::v1 as corev1;

use crate::k8s::{
    KubernetesError,
    PodExt,
};

// Helper functions to get references to a pod's spec and status objects
impl PodExt for corev1::Pod {
    fn labels_contains_key(&self, key: &str) -> bool {
        self.metadata.labels.as_ref().unwrap_or(&Default::default()).contains_key(key)
    }

    fn spec(&self) -> anyhow::Result<&corev1::PodSpec> {
        match self.spec.as_ref() {
            None => bail!(KubernetesError::field_not_found("pod spec")),
            Some(ps) => Ok(ps),
        }
    }

    fn status(&self) -> anyhow::Result<&corev1::PodStatus> {
        match self.status.as_ref() {
            None => bail!(KubernetesError::field_not_found("pod status")),
            Some(ps) => Ok(ps),
        }
    }
}
