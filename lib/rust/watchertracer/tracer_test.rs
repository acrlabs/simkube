use std::collections::{
    HashMap,
    VecDeque,
};

use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use rstest::*;

use super::*;
use crate::util::namespaced_name;

const TESTING_NAMESPACE: &str = "test";

#[fixture]
fn tracer() -> Tracer {
    return Tracer {
        trace: VecDeque::new(),
        tracked_pods: HashMap::new(),
        version: 0,
    };
}

#[fixture]
fn test_pod(#[default(TESTING_NAMESPACE)] namespace: &str, #[default("pod")] name: &str) -> corev1::Pod {
    corev1::Pod {
        metadata: metav1::ObjectMeta {
            namespace: Some(namespace.into()),
            name: Some(name.into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[rstest]
#[tokio::test]
async fn test_create_pod(mut tracer: Tracer, test_pod: corev1::Pod) {
    let ns_name = namespaced_name(&test_pod);
    let ts: i64 = 1234;

    // test idempotency, if we create the same pod twice nothing should change
    tracer.create_pod(&test_pod, ts);
    tracer.create_pod(&test_pod, 2445);

    assert_eq!(tracer.tracked_pods.len(), 1);
    assert_eq!(tracer.tracked_pods[&ns_name], 0);
    assert_eq!(tracer.trace.len(), 1);
    assert_eq!(tracer.trace[0].created_pods.len(), 1);
    assert_eq!(tracer.trace[0].deleted_pods.len(), 0);
    assert_eq!(tracer.trace[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_create_pods(mut tracer: Tracer) {
    let pod_names = vec!["pod1", "pod2"];
    let ts = vec![1234, 3445];
    let pods: Vec<_> = pod_names.iter().map(|p| test_pod("test", p)).collect();

    for i in 0..pods.len() {
        tracer.create_pod(&pods[i], ts[i]);
    }

    assert_eq!(tracer.tracked_pods.len(), pods.len());
    for p in pods.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_pods[&ns_name], 0);
    }
    assert_eq!(tracer.trace.len(), 2);

    for i in 0..pods.len() {
        assert_eq!(tracer.trace[i].created_pods.len(), 1);
        assert_eq!(tracer.trace[i].deleted_pods.len(), 0);
        assert_eq!(tracer.trace[i].ts, ts[i]);
    }
}

#[rstest]
#[tokio::test]
async fn test_delete_pod(mut tracer: Tracer, test_pod: corev1::Pod) {
    let ns_name = namespaced_name(&test_pod);
    let ts: i64 = 1234;

    tracer.tracked_pods.insert(ns_name.clone(), 0);

    // test idempotency, if we delete the same pod twice nothing should change
    tracer.delete_pod(&test_pod, ts);
    tracer.delete_pod(&test_pod, 2445);

    assert_eq!(tracer.tracked_pods.len(), 0);
    assert_eq!(tracer.trace.len(), 1);
    assert_eq!(tracer.trace[0].created_pods.len(), 0);
    assert_eq!(tracer.trace[0].deleted_pods.len(), 1);
    assert_eq!(tracer.trace[0].ts, ts);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_pods_all_new(mut tracer: Tracer) {
    let pod_names = vec!["pod1", "pod2", "pod3"];
    let pods: Vec<_> = pod_names.iter().map(|p| test_pod("test", p)).collect();
    let ts: i64 = 1234;

    // Calling it twice shouldn't change the tracked pods, but should increase the version twice
    tracer.update_all_pods(pods.clone(), ts);
    tracer.update_all_pods(pods.clone(), 2445);

    assert_eq!(tracer.tracked_pods.len(), pods.len());
    for p in pods.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_pods[&ns_name], 1);
    }
    assert_eq!(tracer.trace.len(), 1);
    assert_eq!(tracer.trace[0].created_pods.len(), 3);
    assert_eq!(tracer.trace[0].deleted_pods.len(), 0);
    assert_eq!(tracer.trace[0].ts, ts);
    assert_eq!(tracer.version, 2);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_pods_with_created_pod(mut tracer: Tracer) {
    let pod_names = vec!["pod1", "pod2", "pod3", "pod4"];
    let pods: Vec<_> = pod_names.iter().map(|p| test_pod("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked pods, but should increase the version twice
    let mut fewer_pods = pods.clone();
    fewer_pods.pop();
    tracer.update_all_pods(fewer_pods.clone(), ts[0]);
    tracer.update_all_pods(pods.clone(), ts[1]);

    assert_eq!(tracer.tracked_pods.len(), pods.len());
    for p in fewer_pods.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_pods[&ns_name], 1);
    }
    assert_eq!(tracer.trace.len(), 2);
    assert_eq!(tracer.trace[0].created_pods.len(), 3);
    assert_eq!(tracer.trace[0].deleted_pods.len(), 0);
    assert_eq!(tracer.trace[0].ts, ts[0]);
    assert_eq!(tracer.trace[1].created_pods.len(), 1);
    assert_eq!(tracer.trace[1].deleted_pods.len(), 0);
    assert_eq!(tracer.trace[1].ts, ts[1]);
    assert_eq!(tracer.version, 2);
}

#[rstest]
#[tokio::test]
async fn test_recreate_tracked_pods_with_deleted_pod(mut tracer: Tracer) {
    let pod_names = vec!["pod1", "pod2", "pod3"];
    let pods: Vec<_> = pod_names.iter().map(|p| test_pod("test", p)).collect();
    let ts = vec![1234, 2445];

    // Calling it twice shouldn't change the tracked pods, but should increase the version twice
    tracer.update_all_pods(pods.clone(), ts[0]);
    let mut fewer_pods = pods.clone();
    fewer_pods.pop();
    tracer.update_all_pods(fewer_pods.clone(), ts[1]);

    assert_eq!(tracer.tracked_pods.len(), fewer_pods.len());
    for p in fewer_pods.iter() {
        let ns_name = namespaced_name(p);
        assert_eq!(tracer.tracked_pods[&ns_name], 1);
    }
    assert_eq!(tracer.trace.len(), 2);
    assert_eq!(tracer.trace[0].created_pods.len(), 3);
    assert_eq!(tracer.trace[0].deleted_pods.len(), 0);
    assert_eq!(tracer.trace[0].ts, ts[0]);
    assert_eq!(tracer.trace[1].created_pods.len(), 0);
    assert_eq!(tracer.trace[1].deleted_pods.len(), 1);
    assert_eq!(tracer.trace[1].ts, ts[1]);
    assert_eq!(tracer.version, 2);
}
