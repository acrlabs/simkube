use super::*;
use crate::testutils::{
    pods,
    test_pod,
};

const START_TS: i64 = 1234;

#[rstest]
fn test_pod_lifecycle_data_for_empty(test_pod: corev1::Pod) {
    let res = PodLifecycleData::new_for(&test_pod).unwrap();
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
#[case::with_init_container(true)]
#[case::without_init_container(false)]
fn test_pod_lifecycle_data_for_start_time_only(mut test_pod: corev1::Pod, #[case] init_container: bool) {
    pods::add_running_container(&mut test_pod, START_TS + 10);
    if init_container {
        pods::add_running_init_container(&mut test_pod, START_TS);
        pods::add_running_container(&mut test_pod, START_TS + 5);
    } else {
        pods::add_running_container(&mut test_pod, START_TS);
    }

    let res = PodLifecycleData::new_for(&test_pod).unwrap();
    assert_eq!(res, PodLifecycleData::Running(START_TS));
}

#[rstest]
fn test_pod_lifecycle_data_for_with_some_end_times(mut test_pod: corev1::Pod) {
    let tmid = START_TS + 5;

    pods::add_finished_init_container(&mut test_pod, START_TS, tmid);
    pods::add_running_container(&mut test_pod, tmid);
    pods::add_finished_container(&mut test_pod, tmid, START_TS + 10);

    let res = PodLifecycleData::new_for(&test_pod).unwrap();
    assert_eq!(res, PodLifecycleData::Running(START_TS));
}

#[rstest]
fn test_pod_lifecycle_data_for_with_end_times(mut test_pod: corev1::Pod) {
    let tmid = START_TS + 5;
    let end_ts = START_TS + 10;

    pods::add_finished_init_container(&mut test_pod, START_TS, tmid);
    pods::add_finished_container(&mut test_pod, tmid, tmid + 1);
    pods::add_finished_container(&mut test_pod, tmid, end_ts);

    let res = PodLifecycleData::new_for(&test_pod).unwrap();
    assert_eq!(res, PodLifecycleData::Finished(START_TS, end_ts));
}
