use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::{
    Resource,
    ResourceExt,
};

use crate::constants::SIMULATION_LABEL_KEY;
use crate::error::{
    SimKubeError,
    SimKubeResult,
};

pub fn add_common_fields<K>(sim_name: &str, owner: &K, obj: &mut impl Resource) -> SimKubeResult<()>
where
    K: Resource<DynamicType = ()>,
{
    obj.labels_mut().insert(SIMULATION_LABEL_KEY.into(), sim_name.into());
    obj.owner_references_mut().push(metav1::OwnerReference {
        api_version: K::api_version(&()).into(),
        kind: K::kind(&()).into(),
        name: owner.name_any(),
        uid: owner.uid().ok_or(SimKubeError::FieldNotFound)?,
        ..metav1::OwnerReference::default()
    });

    return Ok(());
}

pub fn label_for(key: &str, val: &str) -> String {
    return format!("{}={}", key, val);
}

pub fn namespaced_name(obj: &impl Resource) -> String {
    return match obj.namespace() {
        Some(ns) => format!("{}/{}", ns, obj.name_any()),
        None => obj.name_any().clone(),
    };
}

pub fn split_namespaced_name(name: &str) -> (String, String) {
    match name.split_once('/') {
        Some((namespace, name)) => (namespace.into(), name.into()),
        None => ("".into(), name.into()),
    }
}
