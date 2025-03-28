use std::collections::hash_map::Entry;
use std::collections::HashMap;

use kube::api::{
    ApiResource,
    DynamicObject,
};
use kube::discovery::ApiCapabilities;

use crate::k8s::GVK;

// An ApiSet object caches a list of ApiResources returned by the k8s server so that we don't have
// to repeatedly make "discovery" calls against the apiserver.
pub struct DynamicApiSet {
    client: kube::Client,
    resources: HashMap<GVK, (ApiResource, ApiCapabilities)>,
    apis: HashMap<GVK, kube::Api<DynamicObject>>,
    namespaced_apis: HashMap<(GVK, String), kube::Api<DynamicObject>>,
}

impl DynamicApiSet {
    pub fn new(client: kube::Client) -> DynamicApiSet {
        DynamicApiSet {
            client,
            resources: HashMap::new(),
            apis: HashMap::new(),
            namespaced_apis: HashMap::new(),
        }
    }

    pub async fn unnamespaced_api_by_gvk(
        &mut self,
        gvk: &GVK,
    ) -> anyhow::Result<(&kube::Api<DynamicObject>, ApiCapabilities)> {
        let (ar, cap) = self.api_meta_for(gvk).await?.clone();
        match self.apis.entry(gvk.clone()) {
            Entry::Occupied(e) => Ok((e.into_mut(), cap)),
            Entry::Vacant(e) => {
                let api = kube::Api::all_with(self.client.clone(), &ar);
                Ok((e.insert(api), cap))
            },
        }
    }

    pub async fn api_for_obj(&mut self, obj: &DynamicObject) -> anyhow::Result<&kube::Api<DynamicObject>> {
        let gvk = GVK::from_dynamic_obj(obj)?;
        let ar = self.api_meta_for(&gvk).await?.0.clone();
        match &obj.metadata.namespace {
            Some(ns) => match self.namespaced_apis.entry((gvk, ns.into())) {
                Entry::Occupied(e) => Ok(e.into_mut()),
                Entry::Vacant(e) => {
                    let api = kube::Api::namespaced_with(self.client.clone(), &e.key().1, &ar);
                    Ok(e.insert(api))
                },
            },
            None => Ok(self.unnamespaced_api_by_gvk(&gvk).await?.0),
        }
    }

    async fn api_meta_for(&mut self, gvk: &GVK) -> anyhow::Result<&(ApiResource, ApiCapabilities)> {
        match self.resources.entry(gvk.clone()) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let api_meta = kube::discovery::pinned_kind(&self.client, e.key()).await?;
                Ok(e.insert(api_meta))
            },
        }
    }
}
