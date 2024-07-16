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
pub struct ApiSet {
    client: kube::Client,
    resources: HashMap<GVK, (ApiResource, ApiCapabilities)>,
    apis: HashMap<GVK, kube::Api<DynamicObject>>,
    namespaced_apis: HashMap<(GVK, String), kube::Api<DynamicObject>>,
}

impl ApiSet {
    pub fn new(client: kube::Client) -> ApiSet {
        ApiSet {
            client,
            resources: HashMap::new(),
            apis: HashMap::new(),
            namespaced_apis: HashMap::new(),
        }
    }

    pub async fn api_for(&mut self, gvk: &GVK) -> anyhow::Result<(&kube::Api<DynamicObject>, ApiCapabilities)> {
        let (ar, cap) = self.api_meta_for(gvk).await?.clone();
        match self.apis.entry(gvk.clone()) {
            Entry::Occupied(e) => Ok((e.into_mut(), cap)),
            Entry::Vacant(e) => {
                let api = kube::Api::all_with(self.client.clone(), &ar);
                Ok((e.insert(api), cap))
            },
        }
    }

    pub async fn namespaced_api_for(&mut self, gvk: &GVK, ns: String) -> anyhow::Result<&kube::Api<DynamicObject>> {
        let ar = self.api_meta_for(gvk).await?.0.clone();
        match self.namespaced_apis.entry((gvk.clone(), ns)) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let api = kube::Api::namespaced_with(self.client.clone(), &e.key().1, &ar);
                Ok(e.insert(api))
            },
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
