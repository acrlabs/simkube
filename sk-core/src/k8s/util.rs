use std::collections::BTreeMap;

use kube::api::Resource;
use serde_json::{
    Map,
    Value,
};

use super::*;
use crate::constants::*;
use crate::errors::*;

const MAX_LABEL_LENGTH: usize = 63;

pub fn add_common_metadata<K>(sim_name: &str, owner: &K, meta: &mut metav1::ObjectMeta)
where
    K: Resource<DynamicType = ()>,
{
    let labels = &mut meta.labels.get_or_insert_default();
    labels.insert(SIMULATION_LABEL_KEY.into(), truncate_label(sim_name.into()));
    labels.insert(
        APP_KUBERNETES_IO_NAME_KEY.into(),
        truncate_label(meta.name.clone().unwrap()), // everything should have a name (???)
    );

    meta.owner_references.get_or_insert_default().push(metav1::OwnerReference {
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
        data: Value::Null,
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

pub fn build_pod_self_owner_reference(pod_name: String) -> metav1::OwnerReference {
    metav1::OwnerReference {
        api_version: POD_GVK.version.clone(),
        kind: POD_GVK.kind.clone(),
        name: pod_name,
        ..Default::default()
    }
}

pub fn dyn_obj_spec(obj: &DynamicObject) -> Option<&Map<String, Value>> {
    obj.data
        .as_object()
        .and_then(|data| data.get("spec").and_then(|spec| spec.as_object()))
}

pub fn dyn_obj_spec_mut(obj: &mut DynamicObject) -> Option<&mut Map<String, Value>> {
    obj.data
        .as_object_mut()
        .and_then(|data| data.get_mut("spec").and_then(|spec| spec.as_object_mut()))
}

pub fn dyn_obj_type_str(obj: &DynamicObject) -> String {
    obj.types
        .as_ref()
        .map(|tm| format!("{}.{}", tm.api_version, tm.kind))
        .unwrap_or("<unknown type>".into())
}

pub fn format_gvk_name(gvk: &GVK, ns_name: &str) -> String {
    format!("{gvk}:{ns_name}")
}

pub fn sanitize_obj<T: kube::Resource>(obj: &mut T) {
    // N.B. We do not sanitize owner references here, since we need them
    // to compute owner chains in the TraceStore
    obj.meta_mut().creation_timestamp = None;
    obj.meta_mut().deletion_timestamp = None;
    obj.meta_mut().deletion_grace_period_seconds = None;
    obj.meta_mut().generation = None;
    obj.meta_mut().managed_fields = None;
    obj.meta_mut().resource_version = None;
    obj.meta_mut().uid = None;

    obj.annotations_mut().remove(LAST_APPLIED_CONFIG_LABEL_KEY);
    obj.annotations_mut().remove(DEPL_REVISION_LABEL_KEY);
}

pub fn pod_is_running(pod: &corev1::Pod) -> bool {
    matches!(pod.status.as_ref(), Some(corev1::PodStatus{phase: Some(phase), ..}) if phase == "Running")
}

pub fn split_namespaced_name(name: &str) -> (String, String) {
    match name.split_once('/') {
        Some((namespace, name)) => (namespace.into(), name.into()),
        None => ("".into(), name.into()),
    }
}

pub fn truncate_label(mut value: String) -> String {
    if value.len() > MAX_LABEL_LENGTH {
        value.truncate(MAX_LABEL_LENGTH - 4);
        value.push_str("XXXX");
    }
    value
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
