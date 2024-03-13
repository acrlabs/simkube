use chrono::DateTime;

use super::*;

#[rstest]
fn test_container_state_waiting() {
    let state = corev1::ContainerState {
        waiting: Some(Default::default()),
        ..Default::default()
    };

    assert_eq!(state.start_ts().unwrap(), None);
    assert_eq!(state.end_ts().unwrap(), None);
}

#[rstest]
fn test_container_state_running() {
    let start_ts = 1234;
    let state = corev1::ContainerState {
        running: Some(corev1::ContainerStateRunning {
            started_at: Some(metav1::Time(DateTime::from_timestamp(start_ts, 0).unwrap())),
        }),
        ..Default::default()
    };

    assert_eq!(state.start_ts().unwrap(), Some(start_ts));
    assert_eq!(state.end_ts().unwrap(), None);
}

#[rstest]
fn test_container_state_running_invalid() {
    let state = corev1::ContainerState {
        running: Some(corev1::ContainerStateRunning { started_at: None }),
        ..Default::default()
    };

    assert!(matches!(
        state.start_ts().unwrap_err().downcast::<KubernetesError>().unwrap(),
        KubernetesError::FieldNotFound(_)
    ));
    assert_eq!(state.end_ts().unwrap(), None);
}

#[rstest]
fn test_container_state_terminated() {
    let start_ts = 1234;
    let end_ts = 5678;
    let state = corev1::ContainerState {
        terminated: Some(corev1::ContainerStateTerminated {
            started_at: Some(metav1::Time(DateTime::from_timestamp(start_ts, 0).unwrap())),
            finished_at: Some(metav1::Time(DateTime::from_timestamp(end_ts, 0).unwrap())),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(state.start_ts().unwrap(), Some(start_ts));
    assert_eq!(state.end_ts().unwrap(), Some(end_ts));
}

#[rstest]
fn test_container_state_terminated_invalid() {
    let state = corev1::ContainerState {
        terminated: Some(corev1::ContainerStateTerminated {
            started_at: None,
            finished_at: None,
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(matches!(
        state.start_ts().unwrap_err().downcast::<KubernetesError>().unwrap(),
        KubernetesError::FieldNotFound(_)
    ));
    assert!(matches!(
        state.end_ts().unwrap_err().downcast::<KubernetesError>().unwrap(),
        KubernetesError::FieldNotFound(_)
    ));
}

#[rstest]
fn test_container_state_invalid() {
    let state = corev1::ContainerState {
        running: Some(corev1::ContainerStateRunning { started_at: None }),
        terminated: Some(corev1::ContainerStateTerminated {
            started_at: None,
            finished_at: None,
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(matches!(
        state.start_ts().unwrap_err().downcast::<KubernetesError>().unwrap(),
        KubernetesError::MalformedContainerState(_)
    ));
    assert!(matches!(
        state.end_ts().unwrap_err().downcast::<KubernetesError>().unwrap(),
        KubernetesError::MalformedContainerState(_)
    ));
}
