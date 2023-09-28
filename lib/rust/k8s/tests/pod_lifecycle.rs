use chrono::{
    DateTime,
    Duration,
    Utc,
};

use super::{
    pod,
    *,
};

#[rstest]
fn test_pod_lifecycle_data_for_empty(pod: corev1::Pod) {
    let res = PodLifecycleData::new_for(&pod).unwrap();
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
#[case::with_init_container(true)]
#[case::without_init_container(false)]
fn test_pod_lifecycle_data_for_start_time_only(mut pod: corev1::Pod, #[case] init_container: bool) {
    let t1 = Utc::now();

    add_container_status_running(&mut pod.status_mut().container_statuses, &(t1 + Duration::seconds(10)));
    if init_container {
        add_container_status_running(&mut pod.status_mut().init_container_statuses, &t1);
        add_container_status_running(&mut pod.status_mut().container_statuses, &(t1 + Duration::seconds(5)));
    } else {
        add_container_status_running(&mut pod.status_mut().init_container_statuses, &t1);
    }

    let res = PodLifecycleData::new_for(&pod).unwrap();
    assert_eq!(res, PodLifecycleData::Running(t1.timestamp()));
}

#[rstest]
fn test_pod_lifecycle_data_for_with_some_end_times(mut pod: corev1::Pod) {
    let t1 = Utc::now();
    let tmid = t1 + Duration::seconds(5);
    let t2 = t1 + Duration::seconds(10);

    add_container_status_finished(&mut pod.status_mut().init_container_statuses, &t1, &tmid);
    add_container_status_running(&mut pod.status_mut().container_statuses, &tmid);
    add_container_status_finished(&mut pod.status_mut().container_statuses, &tmid, &t2);

    let res = PodLifecycleData::new_for(&pod).unwrap();
    assert_eq!(res, PodLifecycleData::Running(t1.timestamp()));
}

#[rstest]
fn test_pod_lifecycle_data_for_with_end_times(mut pod: corev1::Pod) {
    let t1 = Utc::now();
    let tmid = t1 + Duration::seconds(5);
    let t2 = t1 + Duration::seconds(10);

    pod.spec_mut().containers.extend(vec![Default::default(), Default::default()]);
    add_container_status_finished(&mut pod.status_mut().init_container_statuses, &t1, &tmid);
    add_container_status_finished(&mut pod.status_mut().container_statuses, &tmid, &(tmid + Duration::seconds(1)));
    add_container_status_finished(&mut pod.status_mut().container_statuses, &tmid, &t2);

    let res = PodLifecycleData::new_for(&pod).unwrap();
    assert_eq!(res, PodLifecycleData::Finished(t1.timestamp(), t2.timestamp()));
}

fn add_container_status_running(container_statuses: &mut Option<Vec<corev1::ContainerStatus>>, t: &DateTime<Utc>) {
    if container_statuses.is_none() {
        *container_statuses = Some(vec![])
    }

    container_statuses.as_mut().unwrap().push(corev1::ContainerStatus {
        state: Some(corev1::ContainerState {
            running: Some(corev1::ContainerStateRunning { started_at: Some(metav1::Time(t.clone())) }),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn add_container_status_finished(
    container_statuses: &mut Option<Vec<corev1::ContainerStatus>>,
    t1: &DateTime<Utc>,
    t2: &DateTime<Utc>,
) {
    if container_statuses.is_none() {
        *container_statuses = Some(vec![])
    }

    container_statuses.as_mut().unwrap().push(corev1::ContainerStatus {
        state: Some(corev1::ContainerState {
            terminated: Some(corev1::ContainerStateTerminated {
                started_at: Some(metav1::Time(t1.clone())),
                finished_at: Some(metav1::Time(t2.clone())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    })
}
