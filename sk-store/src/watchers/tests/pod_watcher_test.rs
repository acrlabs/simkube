use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

use clockabilly::mock::MockUtcClock;
use mockall::predicate;
use sk_core::k8s::{
    DynamicApiSet,
    OwnersCache,
    PodLifecycleData,
};
use sk_core::prelude::*;

use super::pod_watcher::PodHandler;
use super::*;

const START_TS: i64 = 1234;
const END_TS: i64 = 5678;

#[fixture]
fn clock() -> Box<MockUtcClock> {
    MockUtcClock::boxed(START_TS)
}

fn make_pod_handler(
    ns_name: &str,
    stored_data: Option<&PodLifecycleData>,
    expected_data: Option<&PodLifecycleData>,
) -> PodHandler {
    let mut store = MockTraceStore::new();
    let owner_ref_vec = vec![metav1::OwnerReference {
        api_version: "foo/v1".into(),
        kind: "bar".into(),
        name: "the-owner".into(),
        ..Default::default()
    }];
    if let Some(data) = expected_data {
        let _ = store
            .expect_record_pod_lifecycle()
            .with(
                predicate::eq(ns_name.to_string()),
                predicate::always(),
                predicate::eq(owner_ref_vec.clone()),
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
    let apiset = DynamicApiSet::new(client);
    let owners = HashMap::from([((corev1::Pod::gvk(), ns_name.into()), owner_ref_vec)]);
    let owners_cache = OwnersCache::new_from_parts(apiset, owners);
    PodHandler::new_from_parts(stored_pods, owners_cache, Arc::new(Mutex::new(store)))
}

#[rstest(tokio::test)]
async fn test_handle_event_applied_empty(test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let now = clock.now_ts();
    let mut h = make_pod_handler(&ns_name, None, None);

    h.applied(&test_pod.clone(), now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest(tokio::test)]
async fn test_handle_event_applied(mut test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let expected_data = PodLifecycleData::Running(START_TS);
    let now = clock.now_ts();
    let mut h = make_pod_handler(&ns_name, None, Some(&expected_data));

    add_running_container(&mut test_pod, START_TS);

    h.applied(&test_pod, now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest(tokio::test)]
#[case::same_ts(START_TS)]
#[case::diff_ts(5555)]
async fn test_handle_event_applied_already_stored(
    mut test_pod: corev1::Pod,
    clock: Box<MockUtcClock>,
    #[case] stored_ts: i64,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(stored_ts);
    let now = clock.now_ts();
    let mut h = make_pod_handler(&ns_name, Some(&stored_data), None);

    add_running_container(&mut test_pod, START_TS);

    h.applied(&test_pod, now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest(tokio::test)]
async fn test_handle_event_applied_running_to_finished(mut test_pod: corev1::Pod, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    let now = clock.now_ts();
    let mut h = make_pod_handler(&ns_name, Some(&stored_data), Some(&expected_data));

    add_finished_container(&mut test_pod, START_TS, END_TS);

    h.applied(&test_pod, now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest(tokio::test)]
async fn test_handle_event_applied_running_to_finished_wrong_start_ts(
    mut test_pod: corev1::Pod,
    clock: Box<MockUtcClock>,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(5555);
    let now = clock.now_ts();
    let mut h = make_pod_handler(&ns_name, Some(&stored_data), None);

    add_finished_container(&mut test_pod, START_TS, END_TS);

    h.applied(&test_pod, now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest(tokio::test)]
#[case::no_data(None)]
#[case::mismatched_data(Some(&PodLifecycleData::Finished(1, 2)))]
async fn test_handle_event_deleted_no_update(
    mut test_pod: corev1::Pod,
    mut clock: Box<MockUtcClock>,
    #[case] stored_data: Option<&PodLifecycleData>,
) {
    let ns_name = test_pod.namespaced_name();
    let now = clock.set(END_TS);

    let mut h = make_pod_handler(&ns_name, stored_data, None);

    add_running_container(&mut test_pod, START_TS);

    h.deleted(&test_pod.namespaced_name(), now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest(tokio::test)]
async fn test_handle_event_deleted_already_finished(test_pod: corev1::Pod, mut clock: Box<MockUtcClock>) {
    // If the watcher index says the pod is finished, we've already
    // recorded it in the store, so it really shouldn't matter what the clock says
    let ns_name = test_pod.namespaced_name();
    let finished = PodLifecycleData::Finished(START_TS, END_TS);
    let stored_data = finished.clone();
    let expected_data = None;
    let now = clock.set(10000);

    let mut h = make_pod_handler(&ns_name, Some(&stored_data), expected_data);

    h.deleted(&test_pod.namespaced_name(), now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest(tokio::test)]
async fn test_handle_event_deleted(test_pod: corev1::Pod, mut clock: Box<MockUtcClock>) {
    // Same as the test case above, except this time the pod object
    // doesn't include any info about its containers, it just has metadata
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    let now = clock.set(END_TS);

    let mut h = make_pod_handler(&ns_name, Some(&stored_data), Some(&expected_data));

    h.deleted(&test_pod.namespaced_name(), now).await.unwrap();

    assert_eq!(h.get_owned_pod_lifecycle(&ns_name), None);
}
