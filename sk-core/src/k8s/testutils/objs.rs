use kube::api::{
    DynamicObject,
    GroupVersionKind,
};
use kube::discovery::ApiResource;
use rstest::*;

use crate::prelude::*;

#[fixture]
pub fn test_deployment(#[default(TEST_DEPLOYMENT)] name: &str) -> DynamicObject {
    DynamicObject::new(
        &name,
        &ApiResource::from_gvk(&GroupVersionKind::gvk("core".into(), "v1".into(), "deployment".into())),
    )
    .within(TEST_NAMESPACE)
}
