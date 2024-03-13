use std::cmp::Ordering;

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

#[test]
fn test_partial_eq() {
    assert_eq!(PodLifecycleData::Empty, None);
    assert_eq!(PodLifecycleData::Empty, Some(&PodLifecycleData::Empty));
    assert_eq!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Running(1)));
    assert_eq!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Finished(1, 2)));

    assert_ne!(PodLifecycleData::Empty, Some(&PodLifecycleData::Running(1)));
    assert_ne!(PodLifecycleData::Empty, Some(&PodLifecycleData::Finished(1, 2)));
    assert_ne!(PodLifecycleData::Running(1), None);
    assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Empty));
    assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Running(2)));
    assert_ne!(PodLifecycleData::Running(1), Some(&PodLifecycleData::Finished(1, 2)));
    assert_ne!(PodLifecycleData::Finished(1, 2), None);
    assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Empty));
    assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Running(2)));
    assert_ne!(PodLifecycleData::Finished(1, 2), Some(&PodLifecycleData::Finished(1, 3)));
}

#[test]
fn test_partial_ord() {
    for cmp in [
        PodLifecycleData::Empty.partial_cmp(&None),
        PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Empty)),
        PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Running(1))),
        PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
    ] {
        assert_eq!(cmp, Some(Ordering::Equal));
    }

    for cmp in [
        PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Running(1))),
        PodLifecycleData::Empty.partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
        PodLifecycleData::Running(1).partial_cmp(&None),
        PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Empty)),
        PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Running(2))),
        PodLifecycleData::Running(1).partial_cmp(&Some(&PodLifecycleData::Finished(1, 2))),
        PodLifecycleData::Finished(1, 2).partial_cmp(&None),
        PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Empty)),
        PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Running(2))),
        PodLifecycleData::Finished(1, 2).partial_cmp(&Some(&PodLifecycleData::Finished(1, 3))),
    ] {
        assert_ne!(cmp, Some(Ordering::Equal));
    }

    assert!(PodLifecycleData::Empty < Some(&PodLifecycleData::Running(1)));
    assert!(PodLifecycleData::Empty < Some(&PodLifecycleData::Finished(1, 2)));
    assert!(PodLifecycleData::Running(1) < Some(&PodLifecycleData::Finished(1, 2)));

    assert!(PodLifecycleData::Running(1) > None);
    assert!(PodLifecycleData::Running(1) > Some(&PodLifecycleData::Empty));
    assert!(PodLifecycleData::Finished(1, 2) > None);
    assert!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Empty));
    assert!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Running(1)));

    assert!(!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Running(0))));
    assert!(!(PodLifecycleData::Finished(1, 2) < Some(&PodLifecycleData::Running(0))));
    assert!(!(PodLifecycleData::Finished(1, 2) > Some(&PodLifecycleData::Finished(1, 3))));
    assert!(!(PodLifecycleData::Finished(1, 2) < Some(&PodLifecycleData::Finished(1, 3))));
    assert!(!(PodLifecycleData::Running(1) < Some(&PodLifecycleData::Running(2))));
    assert!(!(PodLifecycleData::Running(1) > Some(&PodLifecycleData::Running(2))));
}
