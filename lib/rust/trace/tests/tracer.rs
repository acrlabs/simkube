use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use serde_json::json;

use super::*;
use crate::k8s::KubeResourceExt;

const TESTING_NAMESPACE: &str = "test";
const EMPTY_SPEC_HASH: u64 = 15130871412783076140;

#[fixture]
fn tracer() -> Tracer {
    Default::default()
}

#[fixture]
fn test_obj(#[default(TESTING_NAMESPACE)] namespace: &str, #[default("obj")] name: &str) -> DynamicObject {
    DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some(namespace.into()),
            name: Some(name.into()),
            ..Default::default()
        },
        types: None,
        data: json!({"spec": {}}),
    }
}

#[rstest]
#[tokio::test]
async fn test_create_or_update_obj(mut tracer: Tracer, test_obj: DynamicObject) {
    let ns_name = test_obj.namespaced_name();
    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_or_update_obj(&test_obj, ts, None);
    tracer.create_or_update_obj(&test_obj, 2445, None);

    assert_eq!(tracer.index.len(), 1);
    assert_eq!(tracer.index[&ns_name], EMPTY_SPEC_HASH);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 1);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_create_or_update_objs(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2"];
    let ts = vec![1234, 3445];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();

    for i in 0..objs.len() {
        tracer.create_or_update_obj(&objs[i], ts[i], None);
    }

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_SPEC_HASH);
    }
    assert_eq!(tracer.events.len(), 2);

    for i in 0..objs.len() {
        assert_eq!(tracer.events[i].applied_objs.len(), 1);
        assert_eq!(tracer.events[i].deleted_objs.len(), 0);
        assert_eq!(tracer.events[i].ts, ts[i]);
    }
}

#[rstest]
#[tokio::test]
async fn test_delete_obj(mut tracer: Tracer, test_obj: DynamicObject) {
    let ns_name = test_obj.namespaced_name();
    let ts: i64 = 1234;

    tracer.index.insert(ns_name.clone(), EMPTY_SPEC_HASH);

    tracer.delete_obj(&test_obj, ts);

    assert_eq!(tracer.index.len(), 0);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 0);
    assert_eq!(tracer.events[0].deleted_objs.len(), 1);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_recreate_index_all_new(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts: i64 = 1234;

    // Calling it twice shouldn't change the tracked objs
    tracer.update_all_objs(&objs, ts);
    tracer.update_all_objs(&objs, 2445);

    assert_eq!(tracer.index.len(), objs.len());
    for p in objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_SPEC_HASH);
    }
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_recreate_index_with_created_obj(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3", "obj4"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(&fewer_objs, ts[0]);
    tracer.update_all_objs(&objs, ts[1]);

    assert_eq!(tracer.index.len(), objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_SPEC_HASH);
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
#[tokio::test]
async fn test_recreate_index_with_deleted_obj(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs
    tracer.update_all_objs(&objs, ts[0]);
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(&fewer_objs, ts[1]);

    assert_eq!(tracer.index.len(), fewer_objs.len());
    for p in fewer_objs.iter() {
        let ns_name = p.namespaced_name();
        assert_eq!(tracer.index[&ns_name], EMPTY_SPEC_HASH);
    }
    assert_eq!(tracer.events.len(), 2);
    assert_eq!(tracer.events[0].applied_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts[0]);
    assert_eq!(tracer.events[1].applied_objs.len(), 0);
    assert_eq!(tracer.events[1].deleted_objs.len(), 1);
    assert_eq!(tracer.events[1].ts, ts[1]);
}
