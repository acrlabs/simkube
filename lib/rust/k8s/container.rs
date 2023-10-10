use super::*;

// StartEndTimeable provides helper functions for computing the start and end times of a container
// from its corresponding ContainerState object.  Note that as per the Kubernetes spec, it is an
// error for the ContainerState to have more than one of `running`, `terminated`, or `waiting` set,
// so we don't have to worry about handling these cases.
impl StartEndTimeable for corev1::ContainerState {
    fn start_ts(&self) -> anyhow::Result<Option<i64>> {
        match self {
            // TODO: saw a panic here once
            corev1::ContainerState { running: Some(r), terminated: None, waiting: None } => {
                Ok(Some(r.started_at.as_ref().unwrap().0.timestamp()))
            },
            corev1::ContainerState { running: None, terminated: Some(t), waiting: None } => {
                Ok(Some(t.started_at.as_ref().unwrap().0.timestamp()))
            },
            corev1::ContainerState { running: None, terminated: None, waiting: Some(_) } => Ok(None),
            _ => Err(KubernetesError::malformed_container_state(self)),
        }
    }

    fn end_ts(&self) -> anyhow::Result<Option<i64>> {
        match self {
            corev1::ContainerState { running: Some(_), terminated: None, waiting: None } => Ok(None),
            corev1::ContainerState { running: None, terminated: Some(t), waiting: None } => {
                Ok(Some(t.finished_at.as_ref().unwrap().0.timestamp()))
            },
            corev1::ContainerState { running: None, terminated: None, waiting: Some(_) } => Ok(None),
            _ => Err(KubernetesError::malformed_container_state(self)),
        }
    }
}
