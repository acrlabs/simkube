use super::*;

// Helper functions to get references to a pod's spec and status objects
impl PodExt for corev1::Pod {
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
