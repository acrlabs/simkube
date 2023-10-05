use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use serde_json::json;

use super::*;
use crate::k8s::KubeResourceExt;
use crate::testutils::*;

const EMPTY_OBJ_HASH: u64 = 15130871412783076140;
const EMPTY_POD_SPEC_HASH: u64 = 16349339464234908611;
const DEPLOYMENT_NAME: &str = "the-deployment";

#[fixture]
fn tracer() -> TraceStore {
    Default::default()
}

#[fixture]
fn test_obj(#[default("obj")] name: &str) -> DynamicObject {
    DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some(TEST_NAMESPACE.into()),
            name: Some(name.into()),
            ..Default::default()
        },
        types: None,
        data: json!({"spec": {}}),
    }
}

#[fixture]
fn owner_ref() -> metav1::OwnerReference {
    metav1::OwnerReference { name: DEPLOYMENT_NAME.into(), ..Default::default() }
}

#[rstest]
fn test_create_or_update_obj(mut tracer: TraceStore, test_obj: DynamicObject) {
    let ns_name = test_obj.namespaced_name();
    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_or_update_obj(&test_obj, ts, None);
    tracer.create_or_update_obj(&test_obj, 2445, None);

    assert_eq!(tracer.index.len(), 1);
    assert_eq!(tracer.index[&ns_name], EMPTY_OBJ_HASH);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 1);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_create_or_update_objs(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2"];
    let ts = vec![1234, 3445];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj(p)).collect();

    for i in 0..objs.len() {
        tracer.create_or_update_obj(&objs[i], ts[i], None);
    }

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_OBJ_HASH);
    }
    assert_eq!(tracer.events.len(), 2);

    for i in 0..objs.len() {
        assert_eq!(tracer.events[i].applied_objs.len(), 1);
        assert_eq!(tracer.events[i].deleted_objs.len(), 0);
        assert_eq!(tracer.events[i].ts, ts[i]);
    }
}

#[rstest]
fn test_delete_obj(mut tracer: TraceStore, test_obj: DynamicObject) {
    let ns_name = test_obj.namespaced_name();
    let ts: i64 = 1234;

    tracer.index.insert(ns_name.clone(), EMPTY_OBJ_HASH);

    tracer.delete_obj(&test_obj, ts);

    assert_eq!(tracer.index.len(), 0);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 0);
    assert_eq!(tracer.events[0].deleted_objs.len(), 1);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_recreate_index_all_new(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj(p)).collect();
    let ts: i64 = 1234;

    // Calling it twice shouldn't change the tracked objs
    tracer.update_all_objs(&objs, ts);
    tracer.update_all_objs(&objs, 2445);

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_OBJ_HASH);
    }
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
fn test_recreate_index_with_created_obj(mut tracer: TraceStore) {
    let obj_names = vec!["obj1", "obj2", "obj3", "obj4"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj(p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(&fewer_objs, ts[0]);
    tracer.update_all_objs(&objs, ts[1]);

    assert_eq!(tracer.index.len(), objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_OBJ_HASH);
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
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj(p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs
    tracer.update_all_objs(&objs, ts[0]);
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(&fewer_objs, ts[1]);

    assert_eq!(tracer.index.len(), fewer_objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_OBJ_HASH);
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
fn test_record_pod_lifecycle_already_stored_no_data(mut tracer: TraceStore, owner_ref: metav1::OwnerReference) {
    assert!(matches!(
        tracer.record_pod_lifecycle("test/the-pod", None, vec![owner_ref], PodLifecycleData::Running(1)),
        Err(_)
    ));
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

    let ns_name = format!("{}/{}", TEST_NAMESPACE, "the-pod");
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(owner_ns_name.clone(), HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]))]),
        HashMap::from([(ns_name.clone(), (owner_ns_name.clone(), EMPTY_POD_SPEC_HASH, pod_seq_idx))]),
    );
    tracer
        .record_pod_lifecycle(&ns_name, None, vec![owner_ref], new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer.pod_owners.lifecycle_data_for(&owner_ns_name, &EMPTY_POD_SPEC_HASH),
        Some(expected_lifecycle_data)
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
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], new_lifecycle_data.clone())
        .unwrap();

    let unused_hash = 0;
    assert_eq!(tracer.pod_owners.lifecycle_data_for(&owner_ns_name, &unused_hash), None);
}

#[rstest]
fn test_record_pod_lifecycle_with_new_pod_type(
    mut tracer: TraceStore,
    test_pod: corev1::Pod,
    owner_ref: metav1::OwnerReference,
) {
    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);
    let new_lifecycle_data = PodLifecycleData::Finished(5, 45);
    tracer.index.insert(owner_ns_name.clone(), EMPTY_OBJ_HASH);
    tracer
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], new_lifecycle_data.clone())
        .unwrap();

    assert_eq!(
        tracer.pod_owners.lifecycle_data_for(&owner_ns_name, &EMPTY_POD_SPEC_HASH),
        Some(vec![new_lifecycle_data])
    );
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

    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);

    tracer.index.insert(owner_ns_name.clone(), EMPTY_OBJ_HASH);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(owner_ns_name.clone(), HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]))]),
        HashMap::from([("asdf".into(), (owner_ns_name.clone(), 1234, 0))]),
    );

    tracer
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer.pod_owners.lifecycle_data_for(&owner_ns_name, &EMPTY_POD_SPEC_HASH),
        Some(expected_lifecycle_data)
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

    let ns_name = test_pod.namespaced_name();
    let owner_ns_name = format!("{}/{}", TEST_NAMESPACE, owner_ref.name);

    tracer.index.insert(owner_ns_name.clone(), EMPTY_OBJ_HASH);
    tracer.pod_owners = PodOwnersMap::new_from_parts(
        HashMap::from([(owner_ns_name.clone(), HashMap::from([(EMPTY_POD_SPEC_HASH, init_lifecycle_data)]))]),
        HashMap::from([(ns_name.clone(), (owner_ns_name.clone(), EMPTY_POD_SPEC_HASH, 0))]),
    );

    tracer
        .record_pod_lifecycle(&ns_name, Some(test_pod), vec![owner_ref], new_lifecycle_data)
        .unwrap();

    assert_eq!(
        tracer.pod_owners.lifecycle_data_for(&owner_ns_name, &EMPTY_POD_SPEC_HASH),
        Some(expected_lifecycle_data)
    );
}
