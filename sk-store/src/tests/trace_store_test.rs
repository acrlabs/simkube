use std::collections::HashMap;

use assertables::*;
use sk_api::v1::ExportFilters;
use sk_core::k8s::GVK;

use super::*;
use crate::pod_owners_map::PodOwnersMap;

#[fixture]
fn tracer() -> TraceStore {
    TraceStore::new(TracerConfig {
        tracked_objects: HashMap::from([(
            DEPL_GVK.clone(),
            TrackedObjectConfig {
                track_lifecycle: true,
                pod_spec_template_paths: Some(vec!["/spec/template".into()]),
                ..Default::default()
            },
        )]),
    })
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

#[rstest]
fn test_export_all(mut tracer: TraceStore, test_deployment: DynamicObject) {
    tracer.create_or_update_obj(&test_deployment, 1, None).unwrap();
    let export_data = tracer.export_all().unwrap();
    let new_trace = TraceStore::import(export_data, None).unwrap();
    assert_len_eq_x!(new_trace.events, 2); // start marker + the deployment event
}

#[rstest]
fn test_lookup_pod_lifecycle_no_owner(tracer: TraceStore) {
    let res = tracer.lookup_pod_lifecycle(&DEPL_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle_no_hash(mut tracer: TraceStore) {
    tracer.index.insert(DEPL_GVK.clone(), TEST_DEPLOYMENT.into(), 1234);
    let res = tracer.lookup_pod_lifecycle(&DEPL_GVK, TEST_DEPLOYMENT, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, PodLifecycleData::Empty);
}

#[rstest]
fn test_lookup_pod_lifecycle(mut tracer: TraceStore) {
    let owner_ns_name = format!("{TEST_NAMESPACE}/{TEST_DEPLOYMENT}");
    let pod_lifecycle = PodLifecycleData::Finished(1, 2);

    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), 1234);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(
            (DEPL_GVK.clone(), owner_ns_name.clone()),
            HashMap::from([(EMPTY_POD_SPEC_HASH, vec![pod_lifecycle.clone()])]),
        )]),
        HashMap::new(),
    );

    let res = tracer.lookup_pod_lifecycle(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH, 0);
    assert_eq!(res, pod_lifecycle);
}

#[rstest]
fn test_collect_events_filtered(mut tracer: TraceStore) {
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

#[rstest]
fn test_collect_events(mut tracer: TraceStore) {
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

#[rstest]
fn test_create_or_update_obj(mut tracer: TraceStore, test_deployment: DynamicObject) {
    let ns_name = test_deployment.namespaced_name();
    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_or_update_obj(&test_deployment, ts, None).unwrap();
    tracer.create_or_update_obj(&test_deployment, 2445, None).unwrap();

    assert_eq!(tracer.index.len(), 1);
    assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 1);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_create_or_update_objs(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2"];
    let ts = vec![1234, 3445];
    let objs: Vec<_> = obj_names.iter().map(|p| test_deployment(p)).collect();

    for i in 0..objs.len() {
        tracer.create_or_update_obj(&objs[i], ts[i], None).unwrap();
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

#[rstest]
fn test_delete_obj(mut tracer: TraceStore, test_deployment: DynamicObject) {
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

#[rstest]
fn test_recreate_index_all_new(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_deployment(p)).collect();
    let ts: i64 = 1234;

    // Calling it twice shouldn't change the tracked objs
    tracer.update_all_objs_for_gvk(&DEPL_GVK, &objs, ts).unwrap();
    tracer.update_all_objs_for_gvk(&DEPL_GVK, &objs, 2445).unwrap();

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    }
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_recreate_index_with_created_obj(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2", "obj3", "obj4"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_deployment(p)).collect();
    let ts = vec![1234, 2445];

    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs_for_gvk(&DEPL_GVK, &fewer_objs, ts[0]).unwrap();
    tracer.update_all_objs_for_gvk(&DEPL_GVK, &objs, ts[1]).unwrap();

    assert_eq!(tracer.index.len(), objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    }
    assert_eq!(tracer.events.len(), 2);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts[0]);
    assert_eq!(tracer.events[1].applied_objs.len(), 1);
    assert_eq!(tracer.events[1].deleted_objs.len(), 0);
    assert_eq!(tracer.events[1].ts, ts[1]);
}

#[rstest]
fn test_recreate_index_with_deleted_obj(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_deployment(p)).collect();
    let ts = vec![1234, 2445];

    tracer.update_all_objs_for_gvk(&DEPL_GVK, &objs, ts[0]).unwrap();
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs_for_gvk(&DEPL_GVK, &fewer_objs, ts[1]).unwrap();

    assert_eq!(tracer.index.len(), fewer_objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    }
    assert_eq!(tracer.events.len(), 2);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts[0]);
    assert_eq!(tracer.events[1].applied_objs.len(), 0);
    assert_eq!(tracer.events[1].deleted_objs.len(), 1);
    assert_eq!(tracer.events[1].ts, ts[1]);
}


#[rstest]
fn test_recreate_index_two_obj_types(mut tracer: TraceStore) {
    let obj_names_1 = vec!["obj1", "obj2", "obj3"];
    let objs1: Vec<_> = obj_names_1.iter().map(|p| test_deployment(p)).collect();
    let obj_names_2 = vec!["obj4", "obj5", "obj6"];
    let objs2: Vec<_> = obj_names_2.iter().map(|p| test_daemonset(p)).collect();
    let ts = 1234;

    tracer.update_all_objs_for_gvk(&DEPL_GVK, &objs1, ts).unwrap();
    tracer.update_all_objs_for_gvk(&DS_GVK, &objs2, ts).unwrap();

    assert_eq!(tracer.index.len(), objs1.len() + objs2.len());
    for p in objs1.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DEPL_GVK, &ns_name).unwrap(), TEST_DEPL_HASH);
    }
    for p in objs2.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index.get(&DS_GVK, &ns_name).unwrap(), TEST_DS_HASH);
    }
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 6);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_record_pod_lifecycle_already_stored_no_data(mut tracer: TraceStore, owner_ref: metav1::OwnerReference) {
    assert!(matches!(
        tracer.record_pod_lifecycle("test/the-pod", None, vec![owner_ref], &PodLifecycleData::Running(1)),
        Err(_)
    ));
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

#[rstest]
fn test_record_pod_lifecycle_already_stored_no_pod(mut tracer: TraceStore, owner_ref: metav1::OwnerReference) {
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
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    tracer.pod_owners =
        mock_pod_owners_map(&pod_ns_name, EMPTY_POD_SPEC_HASH, &owner_ns_name, init_lifecycle_data, pod_seq_idx);
    tracer
        .record_pod_lifecycle(&pod_ns_name, None, vec![owner_ref], &new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}

#[rstest]
fn test_record_pod_lifecycle_with_new_pod_no_tracked_owner(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    tracer
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], &new_lifecycle_data.clone())
        .unwrap();

    let unused_hash = 0;
    assert_eq!(tracer.pod_owners.lifecycle_data_for(&DEPL_GVK, &owner_ns_name, unused_hash), None);
}

#[rstest]
#[case::track_lifecycle(true)]
#[case::dont_track_lifecycle(false)]
fn test_record_pod_lifecycle_with_new_pod_hash(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
    #[case] track_lifecycle: bool,
) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let gvk = GVK::from_owner_ref(&owner_ref).unwrap();
    tracer.config.tracked_objects.get_mut(&gvk).unwrap().track_lifecycle = track_lifecycle;
    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], &new_lifecycle_data.clone())
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

#[rstest]
fn test_record_pod_lifecycle_with_new_pod_existing_hash(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = vec![PodLifecycleData::Running(5)];
    let mut expected_lifecycle_data = init_lifecycle_data.clone();
    expected_lifecycle_data.push(new_lifecycle_data.clone());

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);

    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(
            (DEPL_GVK.clone(), owner_ns_name.clone()),
            HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]),
        )]),
        HashMap::from([("asdf".into(), ((DEPL_GVK.clone(), owner_ns_name.clone()), 1234, 0))]),
    );

    tracer
        .record_pod_lifecycle(&pod_ns_name, Some(test_pod), vec![owner_ref], &new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}

#[rstest]
fn test_record_pod_lifecycle_with_existing_pod(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = vec![PodLifecycleData::Running(5)];
    let expected_lifecycle_data = vec![new_lifecycle_data.clone()];

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);

    tracer.index.insert(DEPL_GVK.clone(), owner_ns_name.clone(), TEST_DEPL_HASH);
    tracer.pod_owners = mock_pod_owners_map(&pod_ns_name, EMPTY_POD_SPEC_HASH, &owner_ns_name, init_lifecycle_data, 0);

    tracer
        .record_pod_lifecycle(&pod_ns_name, Some(test_pod), vec![owner_ref], &new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer
            .pod_owners
            .lifecycle_data_for(&DEPL_GVK, &owner_ns_name, EMPTY_POD_SPEC_HASH),
        Some(&expected_lifecycle_data)
    );
}
