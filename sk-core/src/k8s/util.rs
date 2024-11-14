use std::collections::BTreeMap;

use kube::api::{
    DynamicObject,
    Resource,
    TypeMeta,
};
use serde_json as json;

use super::*;
use crate::errors::*;
use crate::prelude::*;

pub fn add_common_metadata<K>(sim_name: &str, owner: &K, meta: &mut metav1::ObjectMeta)
where
    K: Resource<DynamicType = ()>,
{
    let labels = &mut meta.labels.get_or_insert(BTreeMap::new());
    labels.insert(SIMULATION_LABEL_KEY.into(), sim_name.into());
    labels.insert(APP_KUBERNETES_IO_NAME_KEY.into(), meta.name.clone().unwrap());

    meta.owner_references.get_or_insert(vec![]).push(metav1::OwnerReference {
        api_version: K::api_version(&()).into(),
        kind: K::kind(&()).into(),
        name: owner.name_any(),

        // if the delete propagation policy is set to foreground, this will block
        // the owner from being deleted until this object is deleted
        // (note _both_ must be set, otherwise it doesn't work)
        //
        // https://kubernetes.io/docs/concepts/architecture/garbage-collection/#foreground-deletion
        block_owner_deletion: Some(true),

        // Kubernetes "should" always set this and I'm tired of all the
        // error propogation trying to check for this induces
        uid: owner.uid().unwrap(),
        ..Default::default()
    });
}

pub fn build_deletable(gvk: &GVK, ns_name: &str) -> DynamicObject {
    let (ns, name) = split_namespaced_name(ns_name);
    DynamicObject {
        metadata: metav1::ObjectMeta {
            namespace: Some(ns),
            name: Some(name),
            ..Default::default()
        },
        types: Some(gvk.into_type_meta()),
        data: json::Value::Null,
    }
}

pub fn build_containment_label_selector(key: &str, labels: Vec<String>) -> metav1::LabelSelector {
    metav1::LabelSelector {
        match_expressions: Some(vec![metav1::LabelSelectorRequirement {
            key: key.into(),
            operator: "In".into(),
            values: Some(labels),
        }]),
        ..Default::default()
    }
}

pub fn build_global_object_meta<K>(name: &str, sim_name: &str, owner: &K) -> metav1::ObjectMeta
where
    K: Resource<DynamicType = ()>,
{
    build_object_meta_helper(None, name, sim_name, owner)
}

pub fn build_object_meta<K>(namespace: &str, name: &str, sim_name: &str, owner: &K) -> metav1::ObjectMeta
where
    K: Resource<DynamicType = ()>,
{
    build_object_meta_helper(Some(namespace.into()), name, sim_name, owner)
}

pub fn sanitize_obj(obj: &mut DynamicObject, api_version: &str, kind: &str) {
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

fn build_object_meta_helper<K>(namespace: Option<String>, name: &str, sim_name: &str, owner: &K) -> metav1::ObjectMeta
where
    K: Resource<DynamicType = ()>,
{
    let mut meta = metav1::ObjectMeta {
        namespace,
        name: Some(name.into()),
        ..Default::default()
    };

    add_common_metadata(sim_name, owner, &mut meta);
    meta
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
