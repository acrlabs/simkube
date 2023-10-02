use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

use cached::{
    Cached,
    SizedCache,
};
use futures::{
    stream,
    StreamExt,
};
use hyper::Body;
use mockall::predicate;
use tracing_test::*;

use super::*;
use crate::k8s::{
    ApiSet,
    KubeResourceExt,
    PodLifecycleData,
    GVK,
};
use crate::store::MockTraceStorable;
use crate::testutils::{
    pods,
    test_pod,
    MockUtcClock,
};

const START_TS: i64 = 1234;
const END_TS: i64 = 5678;

#[fixture]
fn clock() -> Box<MockUtcClock> {
    MockUtcClock::new(START_TS)
}

#[fixture]
fn client() -> kube::Client {
    let (mock_service, _) = tower_test::mock::pair::<http::Request<Body>, http::Response<Body>>();
    kube::Client::new(mock_service, "default")
}

#[fixture]
fn apiset(client: kube::Client) -> ApiSet {
    let gvk = GVK::new("foo", "v1", "bar");
    let ar = gvk.guess_api_resource();

    let api_resources = HashMap::from([(gvk.clone(), ar.clone())]);
    let api: kube::Api<DynamicObject> = kube::Api::all_with(client.clone(), &ar);
    let apis = HashMap::from([(gvk, api)]);

    ApiSet::new_from_parts(client.clone(), api_resources, apis, HashMap::new())
}

fn make_pod_watcher(
    ns_name: &str,
    apiset: ApiSet,
    clock: Box<MockUtcClock>,
    stored_data: Option<&PodLifecycleData>,
    expected_data: Option<&PodLifecycleData>,
) -> PodWatcher {
    let mut store = MockTraceStorable::new();
    if let Some(data) = expected_data {
        let _ = store
            .expect_record_pod_lifecycle()
            .with(predicate::eq(ns_name.to_string()), predicate::eq(vec![]), predicate::eq(data.clone()))
            .return_const(())
            .once();
    }

    let stored_pods = if let Some(sd) = stored_data {
        HashMap::from([(ns_name.into(), sd.clone())])
    } else {
        HashMap::new()
    };

    PodWatcher::new_from_parts(
        apiset,
        stream::empty().boxed(),
        stored_pods,
        SizedCache::with_size(CACHE_SIZE),
        Arc::new(Mutex::new(store)),
        clock,
    )
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_empty(test_pod: corev1::Pod, apiset: ApiSet, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let mut pw = make_pod_watcher(&ns_name, apiset, clock, None, None);

    let mut evt = Event::Applied(test_pod.clone());

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied(mut test_pod: corev1::Pod, apiset: ApiSet, clock: Box<MockUtcClock>) {
    let ns_name = test_pod.namespaced_name();
    let expected_data = PodLifecycleData::Running(START_TS);
    let mut pw = make_pod_watcher(&ns_name, apiset, clock, None, Some(&expected_data));

    pods::add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest]
#[case::same_ts(START_TS)]
#[case::diff_ts(5555)]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_already_stored(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
    clock: Box<MockUtcClock>,
    #[case] stored_ts: i64,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(stored_ts);
    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), None);

    pods::add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_running_to_finished(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
    clock: Box<MockUtcClock>,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), Some(&expected_data));

    pods::add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), expected_data);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_applied_running_to_finished_wrong_start_ts(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
    clock: Box<MockUtcClock>,
) {
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(5555);
    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), None);

    pods::add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Applied(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name).unwrap(), stored_data);
}

#[rstest]
#[case::no_data(None)]
#[case::mismatched_data(Some(&PodLifecycleData::Finished(1, 2)))]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_no_update(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
    mut clock: Box<MockUtcClock>,
    #[case] stored_data: Option<&PodLifecycleData>,
) {
    let ns_name = test_pod.namespaced_name();
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, apiset, clock, stored_data, None);

    pods::add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Deleted(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[case::old_still_running(false)]
#[case::old_finished(true)]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_finished(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
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

    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), expected_data);

    pods::add_finished_container(&mut test_pod, START_TS, END_TS);
    let mut evt = Event::Deleted(test_pod);

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_running(
    mut test_pod: corev1::Pod,
    apiset: ApiSet,
    mut clock: Box<MockUtcClock>,
) {
    // Here the pod is still "running" when the delete call comes in, so we
    // expect the end_ts in the lifecycle data to match the current time
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), Some(&expected_data));

    pods::add_running_container(&mut test_pod, START_TS);
    let mut evt = Event::Deleted(test_pod.clone());

    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_deleted_no_container_data(
    test_pod: corev1::Pod,
    apiset: ApiSet,
    mut clock: Box<MockUtcClock>,
) {
    // Same as the test case above, except this time the pod object
    // doesn't include any info about its containers, it just has metadata
    let ns_name = test_pod.namespaced_name();
    let stored_data = PodLifecycleData::Running(START_TS);
    let expected_data = PodLifecycleData::Finished(START_TS, END_TS);
    clock.set(END_TS);

    let mut pw = make_pod_watcher(&ns_name, apiset, clock, Some(&stored_data), Some(&expected_data));
    let mut evt = Event::Deleted(test_pod);
    pw.handle_pod_event(&mut evt).await.unwrap();

    assert_eq!(pw.get_owned_pod_lifecycle(&ns_name), None);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_handle_pod_event_restarted(apiset: ApiSet, mut clock: Box<MockUtcClock>) {
    let pod_lifecycles = [
        ("test/pod1".to_string(), PodLifecycleData::Running(START_TS)),
        ("test/pod2".to_string(), PodLifecycleData::Running(START_TS)),
        ("test/pod3".to_string(), PodLifecycleData::Running(START_TS)),
    ];
    let mut update_pod1 = test_pod("test".into(), "pod1".into());
    pods::add_finished_container(&mut update_pod1, START_TS, END_TS);
    let mut update_pod2 = test_pod("test".into(), "pod2".into());
    pods::add_running_container(&mut update_pod2, START_TS);

    let clock_ts = clock.set(10000);

    let mut store = MockTraceStorable::new();
    let _ = store
        .expect_record_pod_lifecycle()
        .with(
            predicate::eq("test/pod1"),
            predicate::eq(vec![]),
            predicate::eq(PodLifecycleData::Finished(START_TS, END_TS)),
        )
        .return_const(())
        .once();

    let _ = store
        .expect_record_pod_lifecycle()
        .with(predicate::eq("test/pod2".to_string()), predicate::eq(vec![]), predicate::always())
        .never();

    let _ = store
        .expect_record_pod_lifecycle()
        .with(
            predicate::eq("test/pod3".to_string()),
            predicate::eq(vec![]),
            predicate::eq(PodLifecycleData::Finished(START_TS, clock_ts)),
        )
        .return_const(())
        .once();

    let mut cache = SizedCache::with_size(CACHE_SIZE);
    cache.cache_set("test/pod1".into(), vec![]);
    cache.cache_set("test/pod2".into(), vec![]);
    cache.cache_set("test/pod3".into(), vec![]);

    let mut pw = PodWatcher::new_from_parts(
        apiset,
        stream::empty().boxed(),
        HashMap::from(pod_lifecycles),
        cache,
        Arc::new(Mutex::new(store)),
        clock,
    );

    let mut evt = Event::Restarted(vec![update_pod1, update_pod2]);

    pw.handle_pod_event(&mut evt).await.unwrap();
    assert_eq!(pw.get_owned_pod_lifecycle("test/pod1").unwrap(), PodLifecycleData::Finished(START_TS, END_TS));
    assert_eq!(pw.get_owned_pod_lifecycle("test/pod2").unwrap(), PodLifecycleData::Running(START_TS));
    assert_eq!(pw.get_owned_pod_lifecycle("test/pod3"), None);
}
