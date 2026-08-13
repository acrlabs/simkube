use std::fmt;

use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::ApiResource;
use kube::{
    Resource,
    ResourceExt,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::k8s::gvk::*;
use crate::k8s::util::label_expr_match;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct KubeResourceId {
    pub gvk: GVK,
    pub ns_name: String,
}

impl KubeResourceId {
    pub fn new(gvk: GVK, ns_name: String) -> KubeResourceId {
        KubeResourceId { gvk, ns_name }
    }

    pub fn from_owner_ref(owner: &metav1::OwnerReference, namespace: String) -> anyhow::Result<KubeResourceId> {
        let gvk = GVK::from_owner_ref(owner)?;
        let ns_name = format!("{namespace}/{}", owner.name);
        Ok(KubeResourceId { gvk, ns_name })
    }
}

impl fmt::Display for KubeResourceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.gvk, self.ns_name)
    }
}

pub trait SkResourceExt {
    fn matches(&self, sel: &metav1::LabelSelector) -> anyhow::Result<bool>;
    fn namespaced_name(&self) -> String;
    fn resource_id(&self) -> KubeResourceId;
}

trait ResourceGvk<T> {
    fn gvk(obj: &T) -> GVK;
}

impl<T: Resource<DynamicType = ()>> ResourceGvk<T> for () {
    fn gvk(_: &T) -> GVK {
        GVK::new(&T::group(&()), &T::version(&()), &T::kind(&()))
    }
}

impl<T: Resource<DynamicType = ApiResource> + DynamicSelfTyped> ResourceGvk<T> for ApiResource {
    fn gvk(obj: &T) -> GVK {
        GVK::from_dynamic_obj(obj).unwrap()
    }
}

impl<T> SkResourceExt for T
where
    T: Resource,
    T::DynamicType: ResourceGvk<T>,
{
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

    fn resource_id(&self) -> KubeResourceId {
        KubeResourceId {
            gvk: T::DynamicType::gvk(self),
            ns_name: self.namespaced_name(),
        }
    }
}

pub trait OpenApiResourceExt {
    fn gvk() -> GVK;
}

impl<T: k8s_openapi::Resource> OpenApiResourceExt for T {
    fn gvk() -> GVK {
        GVK::new(T::GROUP, T::VERSION, T::KIND)
    }
}
