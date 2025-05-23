use clockabilly::DateTime;
use rstest::fixture;
use sk_core::macros::*;
use sk_core::prelude::*;

use crate::constants::*;

const CONTAINER_PREFIX: &str = "container";
const INIT_CONTAINER_PREFIX: &str = "init-container";

#[fixture]
pub fn test_pod(#[default("the-pod".into())] name: String) -> corev1::Pod {
    corev1::Pod {
        metadata: metav1::ObjectMeta {
            namespace: Some(TEST_NAMESPACE.into()),
            name: Some(name),
            labels: klabel!("foo" => "bar"),
            ..Default::default()
        },
        spec: Some(corev1::PodSpec { ..Default::default() }),
        status: Some(corev1::PodStatus { ..Default::default() }),
    }
}

pub fn add_running_init_container(pod: &mut corev1::Pod, t: i64) {
    add_container_with_status(pod, build_container_state_running(t), true);
}

pub fn add_finished_init_container(pod: &mut corev1::Pod, t1: i64, t2: i64) {
    add_container_with_status(pod, build_container_state_finished(t1, t2), true);
}

pub fn add_running_container(pod: &mut corev1::Pod, t: i64) {
    add_container_with_status(pod, build_container_state_running(t), false);
}

pub fn add_finished_container(pod: &mut corev1::Pod, t1: i64, t2: i64) {
    add_container_with_status(pod, build_container_state_finished(t1, t2), false);
}

fn build_container_state_running(t: i64) -> Option<corev1::ContainerState> {
    Some(corev1::ContainerState {
        running: Some(corev1::ContainerStateRunning {
            started_at: Some(metav1::Time(DateTime::from_timestamp(t, 0).unwrap())),
        }),
        ..Default::default()
    })
}

fn build_container_state_finished(t1: i64, t2: i64) -> Option<corev1::ContainerState> {
    Some(corev1::ContainerState {
        terminated: Some(corev1::ContainerStateTerminated {
            started_at: Some(metav1::Time(DateTime::from_timestamp(t1, 0).unwrap())),
            finished_at: Some(metav1::Time(DateTime::from_timestamp(t2, 0).unwrap())),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn add_container_with_status(pod: &mut corev1::Pod, state: Option<corev1::ContainerState>, init_container: bool) {
    let spec = pod.spec.get_or_insert_default();
    let status = pod.status.get_or_insert_default();
    let (name, containers, statuses) = if init_container {
        let containers = spec.init_containers.get_or_insert_default();
        let statuses = status.init_container_statuses.get_or_insert_default();
        (format!("{}-{}", INIT_CONTAINER_PREFIX, containers.len()), containers, statuses)
    } else {
        let containers = &mut spec.containers;
        let statuses = status.container_statuses.get_or_insert_default();
        (format!("{}-{}", CONTAINER_PREFIX, containers.len()), containers, statuses)
    };

    containers.push(corev1::Container { name: name.clone(), ..Default::default() });
    statuses.push(corev1::ContainerStatus { name: name.clone(), state, ..Default::default() });
}
