use std::collections::hash_map::Entry;
use std::collections::HashMap;

use kube::api::{
    ApiResource,
    DynamicObject,
};

use crate::k8s::GVK;

pub struct ApiSet {
    client: kube::Client,
    resources: HashMap<GVK, ApiResource>,
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

    pub async fn api_for(&mut self, gvk: GVK) -> anyhow::Result<&kube::Api<DynamicObject>> {
        let ar = self.api_resource_for(gvk.clone()).await?.clone();
        match self.apis.entry(gvk) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let api = kube::Api::all_with(self.client.clone(), &ar);
                Ok(e.insert(api))
            },
        }
    }

    pub async fn namespaced_api_for(&mut self, gvk: GVK, ns: String) -> anyhow::Result<&kube::Api<DynamicObject>> {
        let ar = self.api_resource_for(gvk.clone()).await?.clone();
        match self.namespaced_apis.entry((gvk, ns)) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let api = kube::Api::namespaced_with(self.client.clone(), &e.key().1, &ar);
                Ok(e.insert(api))
            },
        }
    }

    pub fn client(&self) -> &kube::Client {
        &self.client
    }

    async fn api_resource_for(&mut self, gvk: GVK) -> anyhow::Result<&ApiResource> {
        match self.resources.entry(gvk) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let (ar, _) = kube::discovery::pinned_kind(&self.client, e.key()).await?;
                Ok(e.insert(ar))
            },
        }
    }
}
