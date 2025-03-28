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
    fn labels_contains_key(&self, key: &str) -> bool {
        self.metadata.labels.as_ref().unwrap_or(&Default::default()).contains_key(key)
    }

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

            // We strip ports when running the simulation (see note in driver/mutation.rs)
            // so we also need to strip them when computing the pod hash
            //
            // A reasonable question might be, why don't we just strip the ports when we collect
            // the trace?  My current hypothesis is that saving as much data as possible during
            // the trace, and then allowing the things processing the trace to do whatever they
            // need with it is a "better" option, but it results in us having to modify things
            // in two places at once, which could cause problems in the future.
            //
            // TODO is it possible to write a test to check for this somehow?
            container.ports = None
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
