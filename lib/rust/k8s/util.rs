use std::collections::BTreeMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::{
    ApiResource,
    DynamicObject,
    GroupVersionKind,
    Resource,
    ResourceExt,
};
use kube::discovery;
use kube::discovery::ApiCapabilities;
use serde_json as json;
use thiserror::Error;
use tracing::*;

use crate::constants::SIMULATION_LABEL_KEY;
use crate::errors::*;
use crate::jsonutils;

pub(super) const LAST_APPLIED_CONFIG_LABEL_KEY: &str = "kubectl.kubernetes.io/last-applied-configuration";
pub(super) const DEPL_REVISION_LABEL_KEY: &str = "deployment.kubernetes.io/revision";

err_impl! {KubernetesError,
    #[error("field not found in struct: {0}")]
    FieldNotFound(String),

    #[error("gvk not found: {0:?}")]
    GroupVersionKindNotFound(GroupVersionKind),

    #[error("malformed label selector: {0:?}")]
    MalformedLabelSelector(metav1::LabelSelectorRequirement),
}

pub fn add_common_fields<K>(sim_name: &str, owner: &K, obj: &mut impl Resource) -> anyhow::Result<()>
where
    K: Resource<DynamicType = ()>,
{
    obj.labels_mut().insert(SIMULATION_LABEL_KEY.into(), sim_name.into());
    obj.owner_references_mut().push(metav1::OwnerReference {
        api_version: K::api_version(&()).into(),
        kind: K::kind(&()).into(),
        name: owner.name_any(),
        uid: owner.uid().ok_or(KubernetesError::field_not_found("uid"))?,
        ..Default::default()
    });

    Ok(())
}

pub async fn get_api_resource(
    gvk: &GroupVersionKind,
    client: &kube::Client,
) -> anyhow::Result<(ApiResource, ApiCapabilities)> {
    let apigroup = discovery::group(client, &gvk.group).await?;
    apigroup
        .versioned_resources(&gvk.version)
        .iter()
        .find(|(res, _)| res.kind == gvk.kind)
        .cloned()
        .ok_or(KubernetesError::group_version_kind_not_found(gvk))
}

pub fn label_for(key: &str, val: &str) -> String {
    format!("{}={}", key, val)
}

pub fn make_deletable(ns_name: &str) -> DynamicObject {
    let (ns, name) = split_namespaced_name(ns_name);
    DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some(ns),
            name: Some(name),
            ..Default::default()
        },
        types: None,
        data: json::Value::Null,
    }
}

pub fn namespaced_name(obj: &impl Resource) -> String {
    match obj.namespace() {
        Some(ns) => format!("{}/{}", ns, obj.name_any()),
        None => obj.name_any().clone(),
    }
}

pub fn obj_matches_selector(obj: &impl Resource, sel: &metav1::LabelSelector) -> anyhow::Result<bool> {
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

pub fn strip_obj(obj: &mut DynamicObject, pod_spec_path: &str) {
    obj.metadata.creation_timestamp = None;
    obj.metadata.deletion_timestamp = None;
    obj.metadata.deletion_grace_period_seconds = None;
    obj.metadata.generation = None;
    obj.metadata.managed_fields = None;
    obj.metadata.owner_references = None;
    obj.metadata.resource_version = None;
    obj.metadata.uid = None;

    if let Some(a) = obj.metadata.annotations.as_mut() {
        a.remove(LAST_APPLIED_CONFIG_LABEL_KEY);
        a.remove(DEPL_REVISION_LABEL_KEY);
    }

    for key in &["nodeName", "serviceAccount", "serviceAccountName"] {
        if let Err(e) = jsonutils::patch_ext::remove(pod_spec_path, key, &mut obj.data) {
            debug!("could not patch object {}, skipping: {}", namespaced_name(obj), e);
        }
    }
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

fn label_expr_match(
    obj_labels: &BTreeMap<String, String>,
    expr: &metav1::LabelSelectorRequirement,
) -> anyhow::Result<bool> {
    // LabelSelectorRequirement is considered invalid if the Operator is "In" or NotIn"
    // and there are no values; conversely for "Exists" and "DoesNotExist".
    match expr.operator.as_str() {
        OPERATOR_IN => match obj_labels.get(&expr.key) {
            Some(v) => match &expr.values {
                Some(values) if !values.is_empty() => Ok(values.contains(v)),
                _ => bail!(KubernetesError::malformed_label_selector(expr)),
            },
            None => Ok(false),
        },
        OPERATOR_NOT_IN => match obj_labels.get(&expr.key) {
            Some(v) => match &expr.values {
                Some(values) if !values.is_empty() => Ok(!values.contains(v)),
                _ => bail!(KubernetesError::malformed_label_selector(expr)),
            },
            None => Ok(true),
        },
        OPERATOR_EXISTS => match &expr.values {
            Some(values) if !values.is_empty() => bail!(KubernetesError::malformed_label_selector(expr)),
            _ => Ok(obj_labels.contains_key(&expr.key)),
        },
        OPERATOR_DOES_NOT_EXIST => match &expr.values {
            Some(values) if !values.is_empty() => {
                bail!(KubernetesError::malformed_label_selector(expr));
            },
            _ => Ok(!obj_labels.contains_key(&expr.key)),
        },
        _ => bail!("malformed label selector expression: {:?}", expr),
    }
}
