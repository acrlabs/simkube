use cached::{
    Cached,
    SizedCache,
};
use kube::ResourceExt;
use serde_json::json;
use tracing_test::traced_test;

use super::*;
use crate::testutils::fake::*;
use crate::testutils::*;

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_compute_owner_chain_cached(mut test_pod: corev1::Pod) {
    let rsref = metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "replicaset".into(),
        name: "test-rs".into(),
        uid: "asdfasdf".into(),
        ..Default::default()
    };
    let deplref = metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "deployment".into(),
        name: "test-depl".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    };

    test_pod.owner_references_mut().push(rsref.clone());
    let expected_owners = vec![rsref, deplref];

    let (_, apiset) = make_fake_apiserver();
    let mut owners = SizedCache::with_size(1000);
    owners.cache_set(test_pod.namespaced_name(), expected_owners.clone());
    let mut cache = OwnersCache::new_from_parts(apiset, owners);

    let res = cache.compute_owner_chain(&test_pod).await.unwrap();
    assert_eq!(res, expected_owners);
}

#[rstest]
#[traced_test]
#[tokio::test]
async fn test_compute_owner_chain(mut test_pod: corev1::Pod) {
    let rsref = metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "ReplicaSet".into(),
        name: "test-rs".into(),
        uid: "asdfasdf".into(),
        ..Default::default()
    };
    let deplref = metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: "test-depl".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    };

    let (mut fake_apiserver, apiset) = make_fake_apiserver();
    fake_apiserver.handle(|when, then| {
        when.path("/apis/apps/v1");
        then.json_body(apps_v1_discovery());
    });

    let rs_owner = deplref.clone();
    fake_apiserver.handle(move |when, then| {
        when.path("/apis/apps/v1/replicasets");
        then.json_body(json!({
            "metadata": {},
            "items": [
                {
                    "metadata": {
                        "namespace": TEST_NAMESPACE,
                        "name": "test-rs",
                        "ownerReferences": [rs_owner],
                    }
                },
            ],
        }));
    });

    fake_apiserver.handle(move |when, then| {
        when.path("/apis/apps/v1/deployments");
        then.json_body(json!({
            "metadata": {},
            "items": [
                {
                    "metadata": {
                        "namespace": TEST_NAMESPACE,
                        "name": "test-depl",
                    }
                },
            ],
        }));
    });
    fake_apiserver.build();

    let mut cache = OwnersCache::new(apiset);

    test_pod.owner_references_mut().push(rsref.clone());
    let res = cache.compute_owner_chain(&test_pod).await.unwrap();

    assert_eq!(res, vec![rsref, deplref]);
}