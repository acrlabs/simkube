use std::collections::HashMap;

use serde_json::json;

use super::*;

#[fixture]
fn rsref() -> metav1::OwnerReference {
    metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "ReplicaSet".into(),
        name: TEST_REPLICASET.into(),
        uid: "asdfasdf".into(),
        ..Default::default()
    }
}

#[fixture]
fn deplref() -> metav1::OwnerReference {
    metav1::OwnerReference {
        api_version: "apps/v1".into(),
        kind: "Deployment".into(),
        name: TEST_DEPLOYMENT.into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_cached(
    mut test_pod: corev1::Pod,
    rsref: metav1::OwnerReference,
    deplref: metav1::OwnerReference,
) {
    test_pod.owner_references_mut().push(rsref.clone());
    let expected_owners = vec![rsref, deplref];

    let (_, client) = make_fake_apiserver();
    let owners = HashMap::from([((corev1::Pod::gvk(), test_pod.namespaced_name()), expected_owners.clone())]);
    let mut cache = OwnersCache::new_from_parts(DynamicApiSet::new(client), owners);

    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await.unwrap();
    assert_eq!(res, expected_owners);
}

#[rstest(tokio::test)]
async fn test_compute_owners_for(
    mut test_pod: corev1::Pod,
    rsref: metav1::OwnerReference,
    deplref: metav1::OwnerReference,
) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
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
                        "name": TEST_REPLICASET,
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
                        "name": TEST_DEPLOYMENT,
                    }
                },
            ],
        }));
    });

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));

    test_pod.owner_references_mut().push(rsref.clone());
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await.unwrap();

    assert_eq!(res, vec![rsref, deplref]);
}
