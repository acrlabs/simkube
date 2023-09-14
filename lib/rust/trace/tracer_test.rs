use std::collections::{
    HashMap,
    VecDeque,
};

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use rstest::*;
use serde_json::Value;

use super::*;
use crate::config::TracerConfig;
use crate::util::namespaced_name;

const TESTING_NAMESPACE: &str = "test";

#[fixture]
fn tracer() -> Tracer {
    return Tracer {
        config: TracerConfig { tracked_objects: vec![] },
        events: VecDeque::new(),
        tracked_objs: HashMap::new(),
        version: 0,
    };
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
        data: Value::Null,
    }
}

#[rstest]
#[tokio::test]
async fn test_create_obj(mut tracer: Tracer, test_obj: DynamicObject) {
    let ns_name = namespaced_name(&test_obj);
    let ts: i64 = 1234;

    // test idempotency, if we create the same obj twice nothing should change
    tracer.create_obj(&test_obj, ts);
    tracer.create_obj(&test_obj, 2445);

    assert_eq!(tracer.tracked_objs.len(), 1);
    assert_eq!(tracer.tracked_objs[&ns_name], 0);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].created_objs.len(), 1);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_create_objs(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2"];
    let ts = vec![1234, 3445];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();

    for i in 0..objs.len() {
        tracer.create_obj(&objs[i], ts[i]);
    }

    assert_eq!(tracer.tracked_objs.len(), objs.len());
    for p in objs.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_objs[&ns_name], 0);
    }
    assert_eq!(tracer.events.len(), 2);

    for i in 0..objs.len() {
        assert_eq!(tracer.events[i].created_objs.len(), 1);
        assert_eq!(tracer.events[i].deleted_objs.len(), 0);
        assert_eq!(tracer.events[i].ts, ts[i]);
    }
}

#[rstest]
#[tokio::test]
async fn test_delete_obj(mut tracer: Tracer, test_obj: DynamicObject) {
    let ns_name = namespaced_name(&test_obj);
    let ts: i64 = 1234;

    tracer.tracked_objs.insert(ns_name.clone(), 0);

    // test idempotency, if we delete the same obj twice nothing should change
    tracer.delete_obj(&test_obj, ts);
    tracer.delete_obj(&test_obj, 2445);

    assert_eq!(tracer.tracked_objs.len(), 0);
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].created_objs.len(), 0);
    assert_eq!(tracer.events[0].deleted_objs.len(), 1);
    assert_eq!(tracer.events[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_objs_all_new(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts: i64 = 1234;

    // Calling it twice shouldn't change the tracked objs, but should increase the version twice
    tracer.update_all_objs(objs.clone(), ts);
    tracer.update_all_objs(objs.clone(), 2445);

    assert_eq!(tracer.tracked_objs.len(), objs.len());
    for p in objs.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_objs[&ns_name], 1);
    }
    assert_eq!(tracer.events.len(), 1);
    assert_eq!(tracer.events[0].created_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts);
    assert_eq!(tracer.version, 2);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_objs_with_created_obj(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3", "obj4"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs, but should increase the version twice
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(fewer_objs.clone(), ts[0]);
    tracer.update_all_objs(objs.clone(), ts[1]);

    assert_eq!(tracer.tracked_objs.len(), objs.len());
    for p in fewer_objs.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_objs[&ns_name], 1);
    }
    assert_eq!(tracer.events.len(), 2);
    assert_eq!(tracer.events[0].created_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts[0]);
    assert_eq!(tracer.events[1].created_objs.len(), 1);
    assert_eq!(tracer.events[1].deleted_objs.len(), 0);
    assert_eq!(tracer.events[1].ts, ts[1]);
    assert_eq!(tracer.version, 2);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_objs_with_deleted_obj(mut tracer: Tracer) {
    let obj_names = vec!["obj1", "obj2", "obj3"];
    let objs: Vec<_> = obj_names.iter().map(|p| test_obj("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked objs, but should increase the version twice
    tracer.update_all_objs(objs.clone(), ts[0]);
    let mut fewer_objs = objs.clone();
    fewer_objs.pop();
    tracer.update_all_objs(fewer_objs.clone(), ts[1]);

    assert_eq!(tracer.tracked_objs.len(), fewer_objs.len());
    for p in fewer_objs.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_objs[&ns_name], 1);
    }
    assert_eq!(tracer.events.len(), 2);
    assert_eq!(tracer.events[0].created_objs.len(), 3);
    assert_eq!(tracer.events[0].deleted_objs.len(), 0);
    assert_eq!(tracer.events[0].ts, ts[0]);
    assert_eq!(tracer.events[1].created_objs.len(), 0);
    assert_eq!(tracer.events[1].deleted_objs.len(), 1);
    assert_eq!(tracer.events[1].ts, ts[1]);
    assert_eq!(tracer.version, 2);
}
