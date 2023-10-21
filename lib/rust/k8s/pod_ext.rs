use super::*;
use crate::prelude::*;

const KUBE_SVC_ACCOUNT_VOLUME_NAME_PREFIX: &str = "kube-api-access";

macro_rules! filter_volumes {
    ($vols:expr) => {
        $vols
            .as_ref()
            .unwrap_or(&vec![])
            .iter()
            .filter(|v| !v.name.starts_with(KUBE_SVC_ACCOUNT_VOLUME_NAME_PREFIX))
            .cloned()
            .collect()
    };
}

// Helper functions to get references to a pod's spec and status objects
impl PodExt for corev1::Pod {
    fn spec(&self) -> anyhow::Result<&corev1::PodSpec> {
        match self.spec.as_ref() {
            None => bail!(KubernetesError::field_not_found("pod spec")),
            Some(ps) => Ok(ps),
        }
    }

    fn stable_spec(&self) -> anyhow::Result<corev1::PodSpec> {
        let mut spec = self.spec()?.clone();
        spec.volumes = Some(filter_volumes!(spec.volumes));
        spec.node_name = None;
        spec.service_account = None;
        spec.service_account_name = None;

        if let Some(containers) = spec.init_containers.as_mut() {
            for container in containers {
                container.volume_mounts = Some(filter_volumes!(container.volume_mounts));
            }
        }

        for container in &mut spec.containers {
            container.volume_mounts = Some(filter_volumes!(container.volume_mounts));
        }

        Ok(spec)
    }

    fn status(&self) -> anyhow::Result<&corev1::PodStatus> {
        match self.status.as_ref() {
            None => bail!(KubernetesError::field_not_found("pod status")),
            Some(ps) => Ok(ps),
        }
    }
}
