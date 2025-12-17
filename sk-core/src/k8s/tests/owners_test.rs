use std::collections::HashMap;

use assertables::*;
use k8s_openapi::Metadata;
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

    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;
    assert_iter_eq!(res, expected_owners);
}

#[rstest(tokio::test)]
async fn test_compute_owners_for(
    mut test_pod: corev1::Pod,
    rsref: metav1::OwnerReference,
    deplref: metav1::OwnerReference,
) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle_multiple(2, |when, then| {
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
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![rsref, deplref]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_bad_ref(mut test_pod: corev1::Pod) {
    let (fake_apiserver, client) = make_fake_apiserver();

    test_pod.metadata_mut().owner_references = Some(vec![metav1::OwnerReference {
        api_version: "tortoise/asdf/bar".into(),
        kind: "Shell".into(),
        name: "the-tortoise-shell".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }]);

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_api_not_found(mut test_pod: corev1::Pod) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle_not_found("/apis/tortoise/v1".into());

    test_pod.metadata_mut().owner_references = Some(vec![metav1::OwnerReference {
        api_version: "tortoise/v1".into(),
        kind: "Shell".into(),
        name: "the-tortoise-shell".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }]);

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_list_fails(mut test_pod: corev1::Pod) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(|when, then| {
        when.path("/apis/tortoise/v1");
        then.json_body(json!({
            "kind":"APIResourceList",
            "apiVersion":"v1",
            "groupVersion":"tortoise/v1",
            "resources":[
                {
                    "name":"shells",
                    "singularName":"shell",
                    "namespaced":true,
                    "kind":"Shell",
                    "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                    "storageVersionHash":"asdf",
                },
            ]}
        ));
    });

    fake_apiserver.handle_not_found("/apis/tortoise/v1/shells".into());

    test_pod.metadata_mut().owner_references = Some(vec![metav1::OwnerReference {
        api_version: "tortoise/v1".into(),
        kind: "Shell".into(),
        name: "the-tortoise-shell".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }]);

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_too_many(mut test_pod: corev1::Pod) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle(|when, then| {
        when.path("/apis/tortoise/v1");
        then.json_body(json!({
            "kind":"APIResourceList",
            "apiVersion":"v1",
            "groupVersion":"tortoise/v1",
            "resources":[
                {
                    "name":"shells",
                    "singularName":"shell",
                    "namespaced":true,
                    "kind":"Shell",
                    "verbs":["create","delete","deletecollection","get","list","patch","update","watch"],
                    "storageVersionHash":"asdf",
                },
            ]}
        ));
    });

    fake_apiserver.handle(|when, then| {
        when.path("/apis/tortoise/v1/shells");
        then.json_body(json!({
            "metadata": {},
            "items": [
                {
                    "metadata": {
                        "namespace": TEST_NAMESPACE,
                        "name": TEST_DEPLOYMENT,
                    },
                },
                {
                    "metadata": {
                        "namespace": TEST_NAMESPACE,
                        "name": TEST_DEPLOYMENT,
                    },
                },
            ]
        }));
    });


    test_pod.metadata_mut().owner_references = Some(vec![metav1::OwnerReference {
        api_version: "tortoise/v1".into(),
        kind: "Shell".into(),
        name: "the-tortoise-shell".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }]);

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}
