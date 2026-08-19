use std::collections::{
    HashMap,
    HashSet,
};

use assertables::*;
use kube::discovery::ApiResource;
use serde_json::json;
use sk_api::v1::ExportFilters;
use sk_core::k8s::{
    DynamicApiSet,
    PodLifecycleData,
};
use sk_core::prelude::*;

use super::*;
use crate::owners_index::OwnersIndex;
use crate::store::TraceStore;

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
                    // Kubernetes doesn't fill in type info here
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
                DEPLOYMENT_GVK.clone(),
                TrackedObjectConfig { track_lifecycle: true, ..Default::default() },
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
            None,
        )
        .await
        .unwrap();

    // Always an empty event at the beginning
    assert_eq!(events, vec![TraceEvent { ts: 1, ..Default::default() }]);
    assert_is_empty!(index);
}

#[rstest(tokio::test)]
async fn test_collect_events_finished_pod(mut tracer: TraceStore) {
    let pod1 = test_dynamic_pod("finished-before-start".into());
    let pod1_ns_name = format!("{TEST_NAMESPACE}/finished-before-start");
    let pod2 = test_dynamic_pod("running-before-start".into());
    let pod2_ns_name = format!("{TEST_NAMESPACE}/running-before-start");
    let pod3 = test_dynamic_pod("finished-after-start".into());
    let pod3_ns_name = format!("{TEST_NAMESPACE}/finished-after-start");
    let pod4 = test_dynamic_pod("running-after-start".into());
    let pod4_ns_name = format!("{TEST_NAMESPACE}/running-after-start");
    tracer.events = vec![
        // This pod gets filtered
        TraceEvent {
            ts: 1,
            applied_objs: vec![pod1.clone()],
            deleted_objs: vec![],
        },
        // These three pods don't
        TraceEvent {
            ts: 2,
            applied_objs: vec![pod2.clone()],
            deleted_objs: vec![],
        },
        TraceEvent {
            ts: 5,
            applied_objs: vec![pod3.clone()],
            deleted_objs: vec![],
        },
        TraceEvent {
            ts: 7,
            applied_objs: vec![pod4.clone()],
            deleted_objs: vec![],
        },
    ];

    tracer
        .owners_index
        .store_new_pod_lifecycle(&pod1_ns_name, &pod1.resource_id(), PodLifecycleData::Finished(1, 2))
        .unwrap();
    tracer
        .owners_index
        .store_new_pod_lifecycle(&pod2_ns_name, &pod2.resource_id(), PodLifecycleData::Running(1))
        .unwrap();
    tracer
        .owners_index
        .store_new_pod_lifecycle(&pod3_ns_name, &pod3.resource_id(), PodLifecycleData::Finished(5, 6))
        .unwrap();
    tracer
        .owners_index
        .store_new_pod_lifecycle(&pod4_ns_name, &pod4.resource_id(), PodLifecycleData::Running(7))
        .unwrap();

    let (events, _) = tracer.collect_events(3, 10, &Default::default(), false, None).await.unwrap();

    // Always an empty event at the beginning
    assert_eq!(
        events,
        vec![
            TraceEvent {
                ts: 3,
                applied_objs: vec![pod2.clone()],
                deleted_objs: vec![]
            },
            TraceEvent {
                ts: 5,
                applied_objs: vec![pod3.clone()],
                deleted_objs: vec![]
            },
            TraceEvent {
                ts: 7,
                applied_objs: vec![pod4.clone()],
                deleted_objs: vec![]
            },
        ]
    );
}

#[rstest(tokio::test)]
async fn test_collect_events_owned_by_tracked_object(mut tracer: TraceStore, test_deployment: DynamicObject) {
    let rs_api_version = ApiResource::from_gvk(&*REPLICASET_GVK);
    let mut replicaset = DynamicObject::new(TEST_REPLICASET, &rs_api_version)
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"replicas": 42}}));
    replicaset.owner_references_mut().push(metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: TEST_DEPLOYMENT.into(),
        ..Default::default()
    });

    tracer.create_or_update_obj(&test_deployment, 4).unwrap();
    tracer.create_or_update_obj(&replicaset, 5).unwrap();
    let (events, index) = tracer.collect_events(1, 10, &Default::default(), true, None).await.unwrap();

    // Only have the deployment in the index
    assert_len_eq_x!(index, 1);

    // Only events are the initial empty one, and the one that creates the deployment
    assert_len_eq_x!(&events, 2);
    assert_is_empty!(&events[0].applied_objs);
    assert_is_empty!(&events[0].deleted_objs);
    assert_eq!(events[1].ts, 4);
    assert_len_eq_x!(&events[1].applied_objs, 1);
    assert_is_empty!(&events[1].deleted_objs);
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
    let (events, objs) = tracer.collect_events(1, 10, &Default::default(), true, None).await.unwrap();

    // The first object was created before the collection started so the timestamp changes
    all_events[0].ts = 1;
    assert_eq!(events, all_events[0..4]);
    assert_eq!(
        objs,
        HashSet::from([
            KubeResourceId::new(DEPLOYMENT_GVK.clone(), format!("{TEST_NAMESPACE}/obj1")),
            KubeResourceId::new(DEPLOYMENT_GVK.clone(), format!("{TEST_NAMESPACE}/obj2")),
            KubeResourceId::new(DEPLOYMENT_GVK.clone(), format!("{TEST_NAMESPACE}/obj3")),
        ])
    );
}

#[rstest(tokio::test)]
async fn test_collect_events_with_skel(mut tracer: TraceStore) {
    let all_events: Vec<_> = [("obj1", 0), ("obj2", 1), ("obj3", 5), ("obj4", 10), ("obj5", 15)]
        .iter()
        .map(|(name, ts)| TraceEvent {
            ts: *ts,
            applied_objs: vec![test_deployment(name)],
            deleted_objs: vec![],
        })
        .collect();
    tracer.events = all_events.clone().into();

    // test_deployment() sets name which maps to metadata.name so we can test a basic
    // SKEL transformation by excluding events for a given metadata.name.
    let skel_str = r#"delete(metadata.name == "obj2");"#;
    let (events, objs) = tracer
        .collect_events(1, 10, &Default::default(), true, Some(skel_str))
        .await
        .unwrap();

    let names: Vec<String> = events
        .iter()
        .flat_map(|event| event.applied_objs.iter())
        .map(|obj| obj.name_any())
        .collect();

    // confirm the deleted item is not in events or index
    let obj2 = String::from("obj2");
    assert_not_contains!(names, &obj2);
    assert_not_contains!(objs, &KubeResourceId::new(DEPLOYMENT_GVK.clone(), obj2));
}

#[rstest(tokio::test)]
async fn test_create_or_update_obj(
    mut tracer: TraceStore,
    test_deployment: DynamicObject,
    owner_ref: metav1::OwnerReference,
) {
    tracer.config.tracked_objects.get_mut(&DEPLOYMENT_GVK).unwrap().skip_owned = true;

    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_or_update_obj(&test_deployment, ts).unwrap();
    tracer.create_or_update_obj(&test_deployment, 2445).unwrap();

    // add an ownerref and make sure this gets skipped
    let mut skip_deployment = test_deployment.clone();
    skip_deployment.metadata.name = Some("foo".into());
    skip_deployment.owner_references_mut().push(owner_ref);

    tracer.create_or_update_obj(&skip_deployment, 2445).unwrap();

    assert_len_eq_x!(&tracer.owners_index, 1);
    assert_some_eq_x!(tracer.owners_index.get_hash(&test_deployment.resource_id()), TEST_DEPL_HASH);
    assert_len_eq_x!(&tracer.events, 1);
    assert_len_eq_x!(&tracer.events[0].applied_objs, 1);
    assert_is_empty!(&tracer.events[0].deleted_objs);
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

    assert_len_eq!(&tracer.owners_index, &objs);
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_some_eq_x!(
            tracer
                .owners_index
                .get_hash(&KubeResourceId::new(DEPLOYMENT_GVK.clone(), ns_name.clone())),
            TEST_DEPL_HASH
        );
    }
    assert_eq!(tracer.events.len(), 2);

    for i in 0..objs.len() {
        assert_len_eq_x!(&tracer.events[i].applied_objs, 1);
        assert_is_empty!(tracer.events[i].deleted_objs);
        assert_eq!(tracer.events[i].ts, ts[i]);
    }
}

#[rstest(tokio::test)]
async fn test_delete_obj(mut tracer: TraceStore, test_deployment: DynamicObject) {
    let ts: i64 = 1234;

    tracer
        .owners_index
        .store_object(test_deployment.resource_id(), TEST_DEPL_HASH, 42);

    tracer.delete_obj(&test_deployment, ts).unwrap();

    assert_len_eq_x!(tracer.owners_index, 0);
    assert_len_eq_x!(&tracer.events, 1);
    assert_is_empty!(&tracer.events[0].applied_objs);
    assert_len_eq_x!(&tracer.events[0].deleted_objs, 1);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_already_stored_no_data(mut tracer: TraceStore) {
    let ns_name = format!("{TEST_NAMESPACE}/{TEST_POD}");
    let res = tracer.record_pod_lifecycle(&ns_name, &None, PodLifecycleData::Running(1)).await;
    assert_ok!(res);
    assert!(!tracer.owners_index.has_pod(&ns_name));
}

fn mock_owners_index_map(
    pod_ns_name: &str,
    owner_ns_name: &str,
    lifecycles: Vec<PodLifecycleData>,
    target_lifecycle_idx: usize,
) -> OwnersIndex {
    let mut owners = OwnersIndex::default();

    // Store every lifecycle under the same owner/hash so tests can model an owner with
    // lifecycle entries.

    // Only the lifecycle at `target_lifecycle_idx` is associated with `pod_ns_name`. The rest get
    // unique placeholder pod names so they can exist in the map without colliding with the pod name
    // being used in our test.

    for (idx, lifecycle) in lifecycles.into_iter().enumerate() {
        let pod_name = if idx == target_lifecycle_idx {
            pod_ns_name.to_string()
        } else {
            format!("{pod_ns_name}-{idx}-UNUSED")
        };

        owners
            .store_new_pod_lifecycle(
                &pod_name,
                &KubeResourceId::new(DEPLOYMENT_GVK.clone(), owner_ns_name.into()),
                lifecycle,
            )
            .unwrap();
    }

    owners
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_already_stored_no_pod(mut tracer: TraceStore) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let pod_seq_idx = 2;
    let init_lifecycle_data = vec![
        PodLifecycleData::Running(1),
        PodLifecycleData::Running(2),
        PodLifecycleData::Running(5),
        PodLifecycleData::Running(7),
    ];
    let mut expected_lifecycle_data = init_lifecycle_data.clone();
    expected_lifecycle_data[pod_seq_idx] = new_lifecycle_data.clone();

    let pod_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_POD);
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_DEPLOYMENT);
    tracer.owners_index = mock_owners_index_map(&pod_ns_name, &owner_ns_name, init_lifecycle_data, pod_seq_idx);
    tracer
        .record_pod_lifecycle(&pod_ns_name, &None, new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(tracer.owners_index.get_pod_lifecycles(&pod_ns_name), expected_lifecycle_data);
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_new_pod_no_tracked_owner(mut tracer: TraceStore, test_pod: corev1::Pod) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_DEPLOYMENT);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    tracer
        .record_pod_lifecycle(&ns_name, &Some(test_pod), new_lifecycle_data.clone())
        .await
        .unwrap();

    assert!(!tracer.owners_index.has_pod(&owner_ns_name));
}

#[rstest(tokio::test)]
#[case::track_lifecycle(true)]
#[case::dont_track_lifecycle(false)]
async fn test_record_pod_lifecycle_with_new_pod_hash(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    #[case] track_lifecycle: bool,
) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_DEPLOYMENT);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);

    tracer.config.tracked_objects.get_mut(&*DEPLOYMENT_GVK).unwrap().track_lifecycle = track_lifecycle;
    tracer
        .owners_index
        .store_object(KubeResourceId::new(DEPLOYMENT_GVK.clone(), owner_ns_name), TEST_DEPL_HASH, 42);
    tracer
        .record_pod_lifecycle(&ns_name, &Some(test_pod), new_lifecycle_data.clone())
        .await
        .unwrap();

    let lifecycle_data = tracer.owners_index.get_pod_lifecycles(&ns_name);
    if track_lifecycle {
        assert_eq!(lifecycle_data, vec![new_lifecycle_data.clone()]);
    } else {
        assert_is_empty!(lifecycle_data);
    }
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_new_pod_existing_hash(mut tracer: TraceStore, test_pod: corev1::Pod) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = PodLifecycleData::Running(5);
    let expected_lifecycle_data = vec![init_lifecycle_data.clone(), new_lifecycle_data.clone()];

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_DEPLOYMENT);

    let owner_id = KubeResourceId::new(DEPLOYMENT_GVK.clone(), owner_ns_name.clone());

    tracer.owners_index.store_object(owner_id.clone(), TEST_DEPL_HASH, 42);

    tracer
        .owners_index
        .store_new_pod_lifecycle("first-pod", &owner_id, init_lifecycle_data)
        .unwrap();

    tracer
        .record_pod_lifecycle(&pod_ns_name, &Some(test_pod), new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(tracer.owners_index.get_pod_lifecycles(&pod_ns_name), expected_lifecycle_data);
}

#[rstest(tokio::test)]
async fn test_record_pod_lifecycle_with_existing_pod(mut tracer: TraceStore, test_pod: corev1::Pod) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let init_lifecycle_data = vec![PodLifecycleData::Running(5)];
    let expected_lifecycle_data = vec![new_lifecycle_data.clone()];

    let pod_ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, TEST_DEPLOYMENT);

    tracer.owners_index.store_object(
        KubeResourceId::new(DEPLOYMENT_GVK.clone(), owner_ns_name.clone()),
        TEST_DEPL_HASH,
        42,
    );
    tracer.owners_index = mock_owners_index_map(&pod_ns_name, &owner_ns_name, init_lifecycle_data, 0);

    tracer
        .record_pod_lifecycle(&pod_ns_name, &Some(test_pod), new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(tracer.owners_index.get_pod_lifecycles(&pod_ns_name), expected_lifecycle_data,);
}

// All we're really testing here is that using the pod as its own owner gets through this
// logic successfully, we assume the other tests cover all the remaining cases
#[rstest(tokio::test)]
async fn test_record_bare_pod_lifecycle(mut tracer: TraceStore, mut test_pod: corev1::Pod) {
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    let expected_lifecycle_data = vec![new_lifecycle_data.clone()];

    test_pod.metadata.owner_references = None;
    let pod_ns_name = test_pod.namespaced_name();

    // Configure bare pod tracking
    tracer.owners_index.store_object(test_pod.resource_id(), 1, 42);
    tracer.config = TracerConfig {
        tracked_objects: HashMap::from([(
            POD_GVK.clone(),
            TrackedObjectConfig { track_lifecycle: true, ..Default::default() },
        )]),
    };

    tracer
        .record_pod_lifecycle(&pod_ns_name, &Some(test_pod), new_lifecycle_data)
        .await
        .unwrap();

    assert_eq!(tracer.owners_index.get_pod_lifecycles(&pod_ns_name), expected_lifecycle_data);
}
