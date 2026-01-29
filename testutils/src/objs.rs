use kube::discovery::ApiResource;
use rstest::fixture;
use serde_json::json;
use sk_core::prelude::*;

use crate::constants::*;

// If the fixture objects below change, these hash values will need to be updated
pub const TEST_DEPL_HASH: u64 = 3664028200602729212;
pub const TEST_DS_HASH: u64 = 16161139027557399432;

#[fixture]
pub fn test_deployment(#[default(TEST_DEPLOYMENT)] name: &str) -> DynamicObject {
    DynamicObject::new(name, &ApiResource::from_gvk(&DEPL_GVK))
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"replicas": 42}}))
}

#[fixture]
pub fn test_daemonset(#[default(TEST_DAEMONSET)] name: &str) -> DynamicObject {
    DynamicObject::new(name, &ApiResource::from_gvk(&DS_GVK))
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"updateStrategy": {"type": "onDelete"}}}))
}

#[fixture]
pub fn test_service_account(#[default(TEST_SERVICE_ACCOUNT)] name: &str) -> DynamicObject {
    DynamicObject::new(name, &ApiResource::from_gvk(&SVC_ACCOUNT_GVK)).within(TEST_NAMESPACE)
}

#[fixture]
pub fn test_two_pods_obj() -> DynamicObject {
    DynamicObject {
        types: Some(TypeMeta {
            api_version: "fake/v1".into(),
            kind: "TwoPods".into(),
        }),
        metadata: metav1::ObjectMeta {
            namespace: Some(TEST_NAMESPACE.into()),
            name: Some("two-pod-object".into()),
            ..Default::default()
        },
        data: json!({
            "spec": {
                "template1": {
                    "spec": {"containers": [{"ports": [42]}]},
                },
                "template2": {
                    "spec": {
                        "containers": [{"ports": [42]}],
                        "nodeSelector": {"foo": "bar"},
                        "tolerations": [{"key": "asdf", "value": "qwerty"}],
                    },
                },
            }
        }),
    }
}
