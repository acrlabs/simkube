use std::collections::HashMap;

use async_recursion::async_recursion;
use kube::Resource;
use kube::api::ListParams;
use kube::discovery::{
    ApiCapabilities,
    Scope,
};
use tracing::*;

use super::*;
use crate::k8s::{
    DynamicApiSet,
    format_gvk_name,
};
use crate::prelude::*;

// TODO I really want a way to mock out the OwnersCache, because
// any tests that depend on it implicitly now have to depend on tokio
// and also the fake_apiserver, which is cumbersome to deal with; unfortunately,
// the &(impl Resource) argument to the traits makes mockall really unhappy;
// I also tried re-architecting it so that we just passed in the object's
// list of OwnerRefs, but then it turns out that the #[async_recursion] bit
// _also_ makes mockall really unhappy.  I think if we're going to mock
// this we'll have to implement the mock ourselves.
pub struct OwnersCache {
    apiset: DynamicApiSet,
    owners: HashMap<(GVK, String), Vec<metav1::OwnerReference>>,
}

impl OwnersCache {
    pub fn new(apiset: DynamicApiSet) -> OwnersCache {
        OwnersCache { apiset, owners: HashMap::new() }
    }

    pub fn new_from_parts(
        apiset: DynamicApiSet,
        owners: HashMap<(GVK, String), Vec<metav1::OwnerReference>>,
    ) -> OwnersCache {
        OwnersCache { apiset, owners }
    }

    // Recursively look up all of the owning objects for a given Kubernetes object
    #[async_recursion]
    pub async fn compute_owners_for(
        &mut self,
        gvk: &GVK,
        obj: &(impl Resource + Sync),
    ) -> anyhow::Result<Vec<metav1::OwnerReference>> {
        let ns_name = obj.namespaced_name();
        let gvk_name = format_gvk_name(gvk, &ns_name);
        debug!("computing owner references for {gvk_name}");

        let key = (gvk.clone(), ns_name.clone());
        if let Some(owners) = self.owners.get(&key) {
            debug!("found owners {owners:?} for {gvk_name} in cache");
            return Ok(owners.clone());
        }

        // Requires that the object's owner references haven't been sanitized away
        let mut owners = Vec::from(obj.owner_references());

        for rf in obj.owner_references() {
            let owner_gvk = GVK::from_owner_ref(rf)?;
            let (api, cap) = self.apiset.unnamespaced_api_by_gvk(&owner_gvk).await?;
            let sel = build_owner_selector(&rf.name, obj, cap);
            let resp = api.list(&sel).await?;
            if resp.items.len() != 1 {
                bail!("could not find single owner for {gvk_name}, found {:?}", resp.items);
            }

            let owner = &resp.items[0];
            owners.extend(self.compute_owners_for(&owner_gvk, owner).await?);
        }

        debug!("computed owners {owners:?} for {gvk_name}");
        self.owners.insert(key, owners.clone());
        Ok(owners)
    }

    pub async fn lookup_by_name_or_obj(
        &mut self,
        gvk: &GVK,
        ns_name: &str,
        maybe_obj: Option<&(impl Resource + Sync)>,
    ) -> anyhow::Result<Vec<metav1::OwnerReference>> {
        match (self.owners.get(&(gvk.clone(), ns_name.into())), maybe_obj) {
            (Some(o), _) => Ok(o.clone()),
            (None, Some(obj)) => self.compute_owners_for(gvk, obj).await,
            _ => bail!("could not determine owner chain for {}", ns_name),
        }
    }
}

fn build_owner_selector(owner_name: &str, obj: &(impl Resource + Sync), owner_cap: ApiCapabilities) -> ListParams {
    let sel = match owner_cap.scope {
        Scope::Cluster => Some(format!("metadata.name={owner_name}")),
        Scope::Namespaced => {
            // if it's namespaced, the namespace field should be populated, so the unwrap is
            // safe/should never trigger
            Some(format!("metadata.namespace={},metadata.name={}", obj.namespace().unwrap(), owner_name))
        },
    };
    ListParams { field_selector: sel, ..Default::default() }
}
