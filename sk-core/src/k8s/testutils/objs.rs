use kube::discovery::ApiResource;
use rstest::*;
use serde_json::json;

use crate::prelude::*;

// If the fixture objects below change, these hash values will need to be updated
pub const TEST_DEPL_HASH: u64 = 3664028200602729212;
pub const TEST_DS_HASH: u64 = 16161139027557399432;

#[fixture]
pub fn test_deployment(#[default(TEST_DEPLOYMENT)] name: &str) -> DynamicObject {
    DynamicObject::new(&name, &ApiResource::from_gvk(&DEPL_GVK))
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"replicas": 42}}))
}

#[fixture]
pub fn test_daemonset(#[default(TEST_DAEMONSET)] name: &str) -> DynamicObject {
    DynamicObject::new(&name, &ApiResource::from_gvk(&DS_GVK))
        .within(TEST_NAMESPACE)
        .data(json!({"spec": {"updateStrategy": {"type": "onDelete"}}}))
}

#[fixture]
pub fn test_service_account(#[default(TEST_SERVICE_ACCOUNT)] name: &str) -> DynamicObject {
    DynamicObject::new(&name, &ApiResource::from_gvk(&SVC_ACCOUNT_GVK)).within(TEST_NAMESPACE)
}
