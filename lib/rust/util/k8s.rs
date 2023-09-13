use std::collections::BTreeMap;

use json_patch::{
    patch,
    PatchErrorKind,
    PatchOperation,
    RemoveOperation,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::{
    DynamicObject,
    Resource,
    ResourceExt,
};
use tracing::*;

use crate::constants::SIMULATION_LABEL_KEY;
use crate::prelude::*;

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
        ..Default::default()
    });

    Ok(())
}

pub fn label_for(key: &str, val: &str) -> String {
    format!("{}={}", key, val)
}

pub fn namespaced_name(obj: &impl Resource) -> String {
    return match obj.namespace() {
        Some(ns) => format!("{}/{}", ns, obj.name_any()),
        None => obj.name_any().clone(),
    };
}

pub fn obj_matches_selector(obj: &impl Resource, sel: &metav1::LabelSelector) -> SimKubeResult<bool> {
    if let Some(exprs) = &sel.match_expressions {
        for expr in exprs {
            if !label_expr_match(obj.labels(), expr)? {
                return Ok(false);
            }
        }
    }

    if let Some(labels) = &sel.match_labels {
        for (k, v) in labels {
            if obj.labels().get(k) != Some(v) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub fn prefixed_ns(prefix: &str, obj: &impl Resource) -> String {
    format!("{}-{}", prefix, obj.namespace().unwrap())
}

pub fn strip_obj(obj: &mut DynamicObject, pod_spec_path: &str) -> SimKubeResult<()> {
    obj.metadata.uid = None;
    obj.metadata.resource_version = None;
    obj.metadata.managed_fields = None;
    obj.metadata.creation_timestamp = None;
    obj.metadata.deletion_timestamp = None;
    obj.metadata.owner_references = None;

    for suffix in &["nodeName", "serviceAccount", "serviceAccountName"] {
        let p = PatchOperation::Remove(RemoveOperation { path: format!("{}/{}", pod_spec_path, suffix) });
        if let Err(e) = patch(&mut obj.data, &[p]) {
            match e.kind {
                PatchErrorKind::InvalidPointer => {
                    debug!("could not find path {} for object {}, skipping", e.path, namespaced_name(obj));
                },
                _ => return Err(SimKubeError::JsonPatchError(e)),
            }
        }
    }

    Ok(())
}

pub fn split_namespaced_name(name: &str) -> (String, String) {
    match name.split_once('/') {
        Some((namespace, name)) => (namespace.into(), name.into()),
        None => ("".into(), name.into()),
    }
}

// The meanings of these operators is explained here:
// https://kubernetes.io/docs/concepts/overview/working-with-objects/labels/#set-based-requirement
pub(super) const OPERATOR_IN: &str = "In";
pub(super) const OPERATOR_NOT_IN: &str = "NotIn";
pub(super) const OPERATOR_EXISTS: &str = "Exists";
pub(super) const OPERATOR_DOES_NOT_EXIST: &str = "DoesNotExist";

pub(super) fn label_expr_match(
    obj_labels: &BTreeMap<String, String>,
    expr: &metav1::LabelSelectorRequirement,
) -> SimKubeResult<bool> {
    // LabelSelectorRequirement is considered invalid if the Operator is "In" or NotIn"
    // and there are no values; conversely for "Exists" and "DoesNotExist".
    return match expr.operator.as_str() {
        OPERATOR_IN => match obj_labels.get(&expr.key) {
            Some(v) => match &expr.values {
                Some(values) if !values.is_empty() => Ok(values.contains(v)),
                _ => Err(SimKubeError::MalformedLabelSelector),
            },
            None => Ok(false),
        },
        OPERATOR_NOT_IN => match obj_labels.get(&expr.key) {
            Some(v) => match &expr.values {
                Some(values) if !values.is_empty() => Ok(!values.contains(v)),
                _ => Err(SimKubeError::MalformedLabelSelector),
            },
            None => Ok(true),
        },
        OPERATOR_EXISTS => match &expr.values {
            Some(values) if !values.is_empty() => Err(SimKubeError::MalformedLabelSelector),
            _ => Ok(obj_labels.contains_key(&expr.key)),
        },
        OPERATOR_DOES_NOT_EXIST => match &expr.values {
            Some(values) if !values.is_empty() => Err(SimKubeError::MalformedLabelSelector),
            _ => Ok(!obj_labels.contains_key(&expr.key)),
        },
        _ => return Err(SimKubeError::MalformedLabelSelector),
    };
}
