use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

use clockabilly::mock::MockUtcClock;
use futures::{
    stream,
    StreamExt,
};
use kube::runtime::watcher::Event;
use mockall::predicate;
use sk_core::k8s::{
    ApiSet,
    KubeResourceExt,
    OwnersCache,
    PodLifecycleData,
};
use sk_core::prelude::*;

use super::*;
use crate::mock::MockTraceStore;

const START_TS: i64 = 1234;
const END_TS: i64 = 5678;

#[fixture]
fn clock() -> Box<MockUtcClock> {
    MockUtcClock::new(START_TS)
}

fn make_pod_watcher(
    ns_name: &str,
    clock: Box<MockUtcClock>,
    stored_data: Option<&PodLifecycleData>,
    expected_data: Option<&PodLifecycleData>,
) -> PodWatcher {
    let mut store = MockTraceStore::new();
    if let Some(data) = expected_data {
        let _ = store
            .expect_record_pod_lifecycle()
            .with(
                predicate::eq(ns_name.to_string()),
                predicate::always(),
                predicate::eq(vec![]),
                predicate::eq(data.clone()),
            )
            .returning(|_, _, _, _| Ok(()))
            .once();
    }

    let stored_pods = if let Some(sd) = stored_data {
        HashMap::from([(ns_name.into(), sd.clone())])
    } else {
        HashMap::new()
    };

    let (_, client) = make_fake_apiserver();
    PodWatcher::new_from_parts(
        stream::empty().boxed(),
        stored_pods,
        OwnersCache::new(ApiSet::new(client)),
        Arc::new(Mutex::new(store)),
        clock,
    )
    .0
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_empty(test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let mut pw = make_pod_watcher(&ns_name, clock, None, None);

    let mut evt = Event::Applied(test_pod.clone());

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied(mut test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let expected_data = PodLifecycleData::Running(START_TS);
    let mut pw = make_pod_watcher(&ns_name, clock, None, Some(&expected_data));

    add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest]
#[case::same_ts(START_TS)]
#[case::diff_ts(5555)]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_already_stored(
    mut test_pod: corev1::Pod,
    clock: Box<MockUtcClock>,
    #[case] stored_ts: i64,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(stored_ts);
    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), None);

    add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_running_to_finished(mut test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), Some(&expected_data));

    add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_running_to_finished_wrong_start_ts(
    mut test_pod: corev1::Pod,
    clock: Box<MockUtcClock>,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(5555);
    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), None);

    add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest]
#[case::no_data(None)]
#[case::mismatched_data(Some(&PodLifecycleData::Finished(1, 2)))]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_no_update(
    mut test_pod: corev1::Pod,
    mut clock: Box<MockUtcClock>,
    #[case] stored_data: Option<&PodLifecycleData>,
) {
    let ns_name = test_pod.namespaced_name();
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, clock, stored_data, None);

    add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Deleted(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[case::old_still_running(false)]
#[case::old_finished(true)]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_finished(
    mut test_pod: corev1::Pod,
    mut clock: Box<MockUtcClock>,
    #[case] old_finished: bool,
) {
    // If the watcher index says the pod is finished, we've already
    // recorded it in the store, so it really shouldn't matter what the clock says
    let ns_name = test_pod.namespaced_name();
    let finished = PodLifecycleData::Finished(START_TS, END_TS);
    let stored_data = if old_finished { finished.clone() } else { PodLifecycleData::Running(START_TS) };
    let expected_data = if old_finished { None } else { Some(&finished) };
    clock.set(10000);

    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), expected_data);

    add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Deleted(test_pod);

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_running(mut test_pod: corev1::Pod, mut clock: Box<MockUtcClock>) {
    // Here the pod is still "running" when the delete call comes in, so we
    // expect the end_ts in the lifecycle data to match the current time
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), Some(&expected_data));

    add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Deleted(test_pod.clone());

    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_no_container_data(test_pod: corev1::Pod, mut clock: Box<MockUtcClock>) {
    // Same as the test case above, except this time the pod object
    // doesn't include any info about its containers, it just has metadata
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, clock, Some(&stored_data), Some(&expected_data));
    let mut evt = Event::Deleted(test_pod);
    pw.handle_pod_event(&mut evt).await;

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_restarted(mut clock: Box<MockUtcClock>) {
    // This test probably requires some explanation: pod0 and pod1 are testing that when a
    // restart event comes in, updated or unchanged data is processed correctly.  pod2 will fail
    // because there is no stored ownership data, and this is testing that we correctly continue
    // processing on an error.  pod3 is testing that we handle deletes correctly.
    let pod_names = ["pod0", "pod1", "pod2", "pod3"].map(|name| format!("{TEST_NAMESPACE}/{name}"));
    let pod_lifecycles: HashMap<String, PodLifecycleData> = pod_names
        .iter()
        .map(|ns_name| (ns_name.clone(), PodLifecycleData::Running(START_TS)))
        .collect();

    let mut update_pod0 = test_pod("pod0".into());
    add_finished_container(&mut update_pod0, START_TS, END_TS);
    let mut update_pod1 = test_pod("pod1".into());
    add_running_container(&mut update_pod1, START_TS);

    let clock_ts = clock.set(10000);

    let mut store = MockTraceStore::new();
    let _ = store
        .expect_record_pod_lifecycle()
        .with(
            predicate::eq(pod_names[0].clone()),
            predicate::always(),
            predicate::eq(vec![]),
            predicate::eq(PodLifecycleData::Finished(START_TS, END_TS)),
        )
        .returning(|_, _, _, _| Ok(()))
        .once();

    let _ = store
        .expect_record_pod_lifecycle()
        .with(predicate::eq(pod_names[1].clone()), predicate::always(), predicate::eq(vec![]), predicate::always())
        .never();

    // no expectations for pod2, because it errors out
    let _ = store
        .expect_record_pod_lifecycle()
        .with(predicate::eq(pod_names[2].clone()), predicate::always(), predicate::eq(vec![]), predicate::always())
        .never();

    let _ = store
        .expect_record_pod_lifecycle()
        .with(
            predicate::eq(pod_names[3].clone()),
            predicate::eq(None),
            predicate::eq(vec![]),
            predicate::eq(PodLifecycleData::Finished(START_TS, clock_ts)),
        )
        .returning(|_, _, _, _| Ok(()))
        .once();

    let (_, client) = make_fake_apiserver();
    let owners = HashMap::from([
        (pod_names[0].clone(), vec![]),
        (pod_names[1].clone(), vec![]),
        // pod2 doesn't belong in the cache so we can induce an error when looking up ownership
        (pod_names[3].clone(), vec![]),
    ]);

    let cache = OwnersCache::new_from_parts(ApiSet::new(client), owners);
    let (mut pw, rx) =
        PodWatcher::new_from_parts(stream::empty().boxed(), pod_lifecycles, cache, Arc::new(Mutex::new(store)), clock);

    let mut evt = Event::Restarted(vec![update_pod0, update_pod1]);

    pw.handle_pod_event(&mut evt).await;
    assert_eq!(pw.get_owned_pod_lifecycle(&pod_names[0]).unwrap(), PodLifecycleData::Finished(START_TS, END_TS));
    assert_eq!(pw.get_owned_pod_lifecycle(&pod_names[1]).unwrap(), PodLifecycleData::Running(START_TS));
    assert_eq!(pw.get_owned_pod_lifecycle(&pod_names[2]), None); // pod2 should still be deleted from our index
    assert_eq!(pw.get_owned_pod_lifecycle(&pod_names[3]), None);
    assert!(rx.try_recv().unwrap()); // don't block this recv
}
