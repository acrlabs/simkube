use std::collections::HashMap;

use assertables::*;
use serde_json::json;

use super::*;

#[fixture]
fn test_shell_pod(mut test_pod: corev1::Pod) -> corev1::Pod {
    // overwrite the defaults in test_pod
    test_pod.metadata.owner_references = Some(vec![metav1::OwnerReference {
        api_version: "tortoise/v1".into(),
        kind: "Shell".into(),
        name: "the-tortoise-shell".into(),
        uid: "yuioyoiuy".into(),
        ..Default::default()
    }]);

    test_pod
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_cached(
    mut test_pod: corev1::Pod,
    rs_owner_ref: metav1::OwnerReference,
    depl_owner_ref: metav1::OwnerReference,
) {
    // overwrite the defaults in test_pod
    test_pod.metadata.owner_references = Some(vec![rs_owner_ref.clone()]);
    let expected_owners = vec![rs_owner_ref, depl_owner_ref];

    let (_, client) = make_fake_apiserver();
    let owners = HashMap::from([((corev1::Pod::gvk(), test_pod.namespaced_name()), expected_owners.clone())]);
    let mut cache = OwnersCache::new_from_parts(DynamicApiSet::new(client), owners);

    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;
    assert_iter_eq!(res, expected_owners);
}

#[rstest(tokio::test)]
async fn test_compute_owners_for(
    mut test_pod: corev1::Pod,
    rs_owner_ref: metav1::OwnerReference,
    depl_owner_ref: metav1::OwnerReference,
) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle_multiple(2, |when, then| {
        when.path("/apis/apps/v1");
        then.json_body(apps_v1_discovery());
    });

    let rs_owner = depl_owner_ref.clone();
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

    // overwrite the defaults in test_pod
    test_pod.metadata.owner_references = Some(vec![rs_owner_ref.clone()]);
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_pod).await;

    assert_iter_eq!(res, vec![rs_owner_ref, depl_owner_ref]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_bad_ref(mut test_pod: corev1::Pod) {
    let (fake_apiserver, client) = make_fake_apiserver();

    // overwrite the defaults in test_pod
    test_pod.metadata.owner_references = Some(vec![metav1::OwnerReference {
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
async fn test_compute_owners_for_api_not_found(test_shell_pod: corev1::Pod) {
    let (mut fake_apiserver, client) = make_fake_apiserver();
    fake_apiserver.handle_not_found("/apis/tortoise/v1".into());
    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_shell_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_list_fails(test_shell_pod: corev1::Pod) {
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

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_shell_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}

#[rstest(tokio::test)]
async fn test_compute_owners_for_too_many(test_shell_pod: corev1::Pod) {
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

    let mut cache = OwnersCache::new(DynamicApiSet::new(client));
    let res = cache.compute_owners_for(&corev1::Pod::gvk(), &test_shell_pod).await;

    assert_iter_eq!(res, vec![]);
    fake_apiserver.assert();
}
