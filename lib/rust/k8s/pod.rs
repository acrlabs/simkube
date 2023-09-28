use super::*;

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

    fn spec_mut(&mut self) -> &mut corev1::PodSpec {
        if self.spec.is_none() {
            self.spec = Some(Default::default());
        }
        self.spec.as_mut().unwrap()
    }

    fn status_mut(&mut self) -> &mut corev1::PodStatus {
        if self.status.is_none() {
            self.status = Some(Default::default());
        }
        self.status.as_mut().unwrap()
    }
}
