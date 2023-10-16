use async_recursion::async_recursion;
use cached::{
    Cached,
    SizedCache,
};
use kube::api::ListParams;
use kube::discovery::{
    ApiCapabilities,
    Scope,
};
use kube::{
    Resource,
    ResourceExt,
};
use tracing::*;

use super::*;
use crate::k8s::ApiSet;

pub(crate) const CACHE_SIZE: usize = 10000;

pub struct OwnersCache {
    apiset: ApiSet,
    owners: SizedCache<String, Vec<metav1::OwnerReference>>,
}

impl OwnersCache {
    pub fn new(apiset: ApiSet) -> OwnersCache {
        OwnersCache { apiset, owners: SizedCache::with_size(CACHE_SIZE) }
    }

    // Recursively look up all of the owning objects for a given Kubernetes object
    #[async_recursion]
    pub async fn compute_owner_chain(
        &mut self,
        obj: &(impl Resource + Sync),
    ) -> anyhow::Result<Vec<metav1::OwnerReference>> {
        let ns_name = obj.namespaced_name();
        info!("computing owner references for {}", ns_name);

        if let Some(owners) = self.owners.cache_get(&ns_name) {
            info!("found owners for {} in cache", ns_name);
            return Ok(owners.clone());
        }

        let mut owners = Vec::from(obj.owner_references());

        for rf in obj.owner_references() {
            let owner_gvk = GVK::from_owner_ref(rf)?;
            let (api, cap) = self.apiset.api_for(&owner_gvk).await?;
            let sel = build_owner_selector(&rf.name, obj, cap);
            let resp = api.list(&sel).await?;
            if resp.items.len() != 1 {
                bail!("could not find single owner for {}, found {:?}", obj.namespaced_name(), resp.items);
            }

            let owner = &resp.items[0];
            owners.extend(self.compute_owner_chain(owner).await?);
        }

        self.owners.cache_set(ns_name.clone(), owners.clone());
        Ok(owners)
    }

    pub fn lookup(&mut self, ns_name: &str) -> Option<&Vec<metav1::OwnerReference>> {
        self.owners.cache_get(ns_name)
    }
}

fn build_owner_selector(owner_name: &str, obj: &(impl Resource + Sync), owner_cap: ApiCapabilities) -> ListParams {
    let sel = match owner_cap.scope {
        Scope::Cluster => Some(format!("metadata.name={}", owner_name)),
        Scope::Namespaced => {
            // if it's namespaced, the namespace field should be populated, so the unwrap is
            // safe/should never trigger
            Some(format!("metadata.namespace={},metadata.name={}", obj.namespace().unwrap(), owner_name))
        },
    };
    ListParams { field_selector: sel, ..Default::default() }
}

#[cfg(feature = "testutils")]
impl OwnersCache {
    pub fn new_from_parts(apiset: ApiSet, owners: SizedCache<String, Vec<metav1::OwnerReference>>) -> OwnersCache {
        OwnersCache { apiset, owners }
    }
}
