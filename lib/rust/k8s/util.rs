use std::collections::BTreeMap;

use kube::api::{
    DynamicObject,
    ListParams,
    Resource,
    ResourceExt,
    TypeMeta,
};
use serde_json as json;
use tracing::*;

use super::*;
use crate::constants::SIMULATION_LABEL_KEY;
use crate::errors::*;
use crate::jsonutils;

pub fn add_common_metadata<K>(sim_name: &str, owner: &K, meta: &mut metav1::ObjectMeta) -> EmptyResult
where
    K: Resource<DynamicType = ()>,
{
    meta.labels
        .get_or_insert(BTreeMap::new())
        .insert(SIMULATION_LABEL_KEY.into(), sim_name.into());
    meta.owner_references.get_or_insert(vec![]).push(metav1::OwnerReference {
        api_version: K::api_version(&()).into(),
        kind: K::kind(&()).into(),
        name: owner.name_any(),
        uid: owner.uid().ok_or(KubernetesError::field_not_found("uid"))?,
        ..Default::default()
    });
    Ok(())
}

pub fn build_deletable(ns_name: &str) -> DynamicObject {
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

pub fn build_global_object_meta<K>(name: &str, sim_name: &str, owner: &K) -> anyhow::Result<metav1::ObjectMeta>
where
    K: Resource<DynamicType = ()>,
{
    build_object_meta_helper(None, name, sim_name, owner)
}

pub fn build_object_meta<K>(
    namespace: &str,
    name: &str,
    sim_name: &str,
    owner: &K,
) -> anyhow::Result<metav1::ObjectMeta>
where
    K: Resource<DynamicType = ()>,
{
    build_object_meta_helper(Some(namespace.into()), name, sim_name, owner)
}

pub fn label_selector(key: &str, value: &str) -> ListParams {
    ListParams {
        label_selector: Some(format!("{}={}", key, value)),
        ..Default::default()
    }
}

pub fn list_params_for(namespace: &str, name: &str) -> ListParams {
    ListParams {
        field_selector: Some(format!("metadata.namespace={},metadata.name={}", namespace, name)),
        ..Default::default()
    }
}

pub fn sanitize_obj(obj: &mut DynamicObject, pod_spec_path: &str, api_version: &str, kind: &str) {
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
            debug!("could not patch object {}, skipping: {}", obj.namespaced_name(), e);
        }
    }

    obj.types = Some(TypeMeta { api_version: api_version.into(), kind: kind.into() });
}

pub fn split_namespaced_name(name: &str) -> (String, String) {
    match name.split_once('/') {
        Some((namespace, name)) => (namespace.into(), name.into()),
        None => ("".into(), name.into()),
    }
}

impl<T: Resource> KubeResourceExt for T {
    fn namespaced_name(&self) -> String {
        match self.namespace() {
            Some(ns) => format!("{}/{}", ns, self.name_any()),
            None => self.name_any().clone(),
        }
    }

    fn matches(&self, sel: &metav1::LabelSelector) -> anyhow::Result<bool> {
        if let Some(exprs) = &sel.match_expressions {
            for expr in exprs {
                if !label_expr_match(self.labels(), expr)? {
                    return Ok(false);
                }
            }
        }

        if let Some(labels) = &sel.match_labels {
            for (k, v) in labels {
                if self.labels().get(k) != Some(v) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

fn build_object_meta_helper<K>(
    namespace: Option<String>,
    name: &str,
    sim_name: &str,
    owner: &K,
) -> anyhow::Result<metav1::ObjectMeta>
where
    K: Resource<DynamicType = ()>,
{
    let mut meta = metav1::ObjectMeta {
        namespace,
        name: Some(name.into()),
        ..Default::default()
    };

    add_common_metadata(sim_name, owner, &mut meta)?;
    Ok(meta)
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
