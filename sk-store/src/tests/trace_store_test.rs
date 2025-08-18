use std::collections::HashMap;

use assertables::*;
use serde_json::json;
use sk_api::v1::ExportFilters;
use sk_core::k8s::{
    DynamicApiSet,
    PodLifecycleData,
};

use super::*;
use crate::pod_owners_map::PodOwnersMap;

#[fixture]
fn tracer() -> TraceStore {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(|when, then| {
        when.path("/apis/apps/v1");
        then.json_body(apps_v1_discovery());
    });
    fake_apiserver.handle(|when, then| {
        when.path("/apis/apps/v1/deployments");
        then.json_body(json!({
            "metadata": {},
            "items": [
                {
                    "metadata": {
                        "namespace": TEST_NAMESPACE,
                        "name": TEST_DEPLOYMENT,
                    }
                },
            ],
        }));
    });
    let apiset = DynamicApiSet::new(client);
    TraceStore::new(
        TracerConfig {
            tracked_objects: HashMap::from([(
                DEPL_GVK.clone(),
                TrackedObjectConfig {
                    track_lifecycle: true,
                    pod_spec_template_paths: Some(vec!["/spec/template".into()]),
                    ..Default::default()
                },
            )]),
        },
        apiset,
    )
}

#[fixture]
fn owner_ref() -> metav1::OwnerReference {
    metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: TEST_DEPLOYMENT.into(),
        ..Default::default()
    }
}

#[rstest(tokio::test)]
async fn test_collect_events_filtered(mut tracer: TraceStore) {
    tracer.events = [("obj1", 0), ("obj2", 1), ("obj3", 5), ("obj4", 10), ("obj5", 15)]
        .iter()
        .map(|(name, ts)| TraceEvent {
            ts: *ts,
            applied_objs: vec![test_deployment(name)],
            deleted_objs: vec![],
        })
        .collect();

    let (events, index) = tracer
        .collect_events(
            1,
            10,
            &ExportFilters {
                excluded_namespaces: vec![TEST_NAMESPACE.into()],
                ..Default::default()
            },
            false,
        )
        .unwrap();

    // Always an empty event at the beginning
    assert_eq!(events, vec![TraceEvent { ts: 1, ..Default::default() }]);
    assert!(index.is_empty());
}

#[rstest(tokio::test)]
async fn test_collect_events(mut tracer: TraceStore) {
    let mut all_events: Vec<_> = [("obj1", 0), ("obj2", 1), ("obj3", 5), ("obj4", 10), ("obj5", 15)]
        .iter()
        .map(|(name, ts)| TraceEvent {
            ts: *ts,
            applied_objs: vec![test_deployment(name)],
            deleted_objs: vec![],
        })
        .collect();
    all_events.insert(
        3,
        TraceEvent {
            ts: 4,
            applied_objs: vec![],
            deleted_objs: vec![test_deployment("obj2")],
        },
    );
    all_events.push(TraceEvent {
        ts: 25,
        applied_objs: vec![],
        deleted_objs: vec![test_deployment("obj1")],
    });
    tracer.events = all_events.clone().into();
    let (events, index) = tracer.collect_events(1, 10, &Default::default(), true).unwrap();

    // The first object was created before the collection started so the timestamp changes
    all_events[0].ts = 1;
    assert_eq!(events, all_events[0..4]);
    assert_bag_eq!(
        index.flattened_keys(),
        [
            format!("{}:{TEST_NAMESPACE}/obj1", &*DEPL_GVK),
            format!("{}:{TEST_NAMESPACE}/obj2", &*DEPL_GVK),
            format!("{}:{TEST_NAMESPACE}/obj3", &*DEPL_GVK)
        ]
        .map(|s| s.to_string())
    );
}

#[rstest(tokio::test)]
async fn test_create_or_update_obj(mut tracer: TraceStore, test_deployment: DynamicObject) {
    let ns_name = test_deployment.namespaced_name();
    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_or_update_obj(&test_deployment, ts).unwrap();
    tracer.create_or_update_obj(&test_deployment, 2445).unwrap();

    assert_eq!(tracer.index.len(), 1);
    assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 1);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest(tokio::test)]
async fn test_create_or_update_objs(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2"];
    let ts = vec![1234, 3445];
    let objs: Vec<_> = obj_names.iter().map(|p| test_deployment(p)).collect();

    for i in 0..objs.len() {
        tracer.create_or_update_obj(&objs[i], ts[i]).unwrap();
    }

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    }
    assert_eq!(tracer.events.len(), 2);

    for i in 0..objs.len() {
        assert_eq!(tracer.events[i].applied_objs.len(), 1);
        assert_eq!(tracer.events[i].deleted_objs.len(), 0);
        assert_eq!(tracer.events[i].ts, ts[i]);
    }
}

#[rstest(tokio::test)]
async fn test_delete_obj(mut tracer: TraceStore, test_deployment: DynamicObject) {
    let ns_name = test_deployment.namespaced_name();
    let ts: i64 = 1234;

    tracer.index.insert(DEPL_GVK.clone(), ns_name.clone(), TEST_DEPL_HASH);

    tracer.delete_obj(&test_deployment, ts).unwrap();

    assert_eq!(tracer.index.len(), 0);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 0);
    assert_eq!(tracer.events[0].deleted_objs.len(), 1);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_already_stored_no_data(mut tracer: TraceStore) {
    let res = tracer
        .record_pod_lifecycle("test/the-pod", &None, &PodLifecycleData::Running(1))
        .await;
    assert!(matches!(res, Err(_)));
}

fn mock_pod_owners_map(
    pod_ns_name: &str,
    pod_spec_hash: u64,
    owner_ns_name: &str,
    init_lifecycle_data: Vec<PodLifecycleData>,
    pod_seq_idx: usize,
) -> PodOwnersMap {
    PodOwnersMap::new_from_parts(
        HashMap::from([(
            (DEPL_GVK.clone(), owner_ns_name.into()),
            HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]),
        )]),
        HashMap::from([(pod_ns_name.into(), ((DEPL_GVK.clone(), owner_ns_name.into()), pod_spec_hash, pod_seq_idx))]),
    )
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_already_stored_no_pod(mut tracer: TraceStore) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let pod_seq_idx = 2;
    let init_lifecycle_data = vec![
        PodLifecycleData::Running(10),
        PodLifecycleData::Running(20),
        PodLifecycleData::Running(5),
        PodLifecycleData::Running(40),
    ];
    let mut expected_lifecycle_data = init_lifecycle_data.clone();
    expected_lifecycle_data[pod_seq_idx] = new_lifecycle_data.clone();

    let pod_ns_name = format!("{}/{}", TEST_NAMESPACE, "the-pod");
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_REPLICASET);
    tracer.pod_owners =
        mock_pod_owners_map(&pod_ns_name, EMPTY_POD_SPEC_HASH, &owner_ns_name, init_lifecycle_data, pod_seq_idx);
    tracer
        .record_pod_lifecycle(&pod_ns_name, &None, &new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_new_pod_no_tracked_owner(mut tracer: TraceStore, test_pod: corev1::Pod) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_REPLICASET);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    tracer
        .record_pod_lifecycle(&ns_name, &Some(test_pod), &new_lifecycle_data.clone())
        .await
        .unwrap();

    let unused_hash = 0;
    assert_eq!(tracer.pod_owners.lifecycle_data_for(&DEPL_GVK, &owner_ns_name, unused_hash), None);
}

#[rstest(tokio::test)]
#[case::track_lifecycle(true)]
#[case::dont_track_lifecycle(false)]
async fn test_record_pod_lifecycle_with_new_pod_hash(
    mut tracer: TraceStore,
    mut test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
    #[case] track_lifecycle: bool,
) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    test_pod.owner_references_mut().push(owner_ref);

    tracer.config.tracked_objects.get_mut(&*DEPL_GVK).unwrap().track_lifecycle = track_lifecycle;
    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer
        .record_pod_lifecycle(&ns_name, &Some(test_pod), &new_lifecycle_data.clone())
        .await
        .unwrap();

    let lifecycle_data = tracer
        .pod_owners
        .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH);
    if track_lifecycle {
        assert_eq!(lifecycle_data, Some(&vec![new_lifecycle_data]));
    } else {
        assert_eq!(lifecycle_data, None);
    }
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_new_pod_existing_hash(
    mut tracer: TraceStore,
    mut test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = vec![PodLifecycleData::Running(5)];
    let mut expected_lifecycle_data = init_lifecycle_data.clone();
    expected_lifecycle_data.push(new_lifecycle_data.clone());

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    test_pod.owner_references_mut().push(owner_ref);

    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(
            (DEPL_GVK.clone(), owner_ns_name.clone()),
            HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]),
        )]),
        HashMap::from([("asdf".into(), ((DEPL_GVK.clone(), owner_ns_name.clone()), 1234, 0))]),
    );

    tracer
        .record_pod_lifecycle(&pod_ns_name, &Some(test_pod), &new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_existing_pod(mut tracer: TraceStore, test_pod: corev1::Pod) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = vec![PodLifecycleData::Running(5)];
    let expected_lifecycle_data = vec![new_lifecycle_data.clone()];

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_REPLICASET);

    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer.pod_owners = mock_pod_owners_map(&pod_ns_name, EMPTY_POD_SPEC_HASH, &owner_ns_name, init_lifecycle_data, 0);

    tracer
        .record_pod_lifecycle(&pod_ns_name, &Some(test_pod), &new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}
