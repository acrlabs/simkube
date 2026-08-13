use std::collections::HashMap;

use async_recursion::async_recursion;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
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

use crate::k8s::{
    DynamicApiSet,
    GVK,
    KubeResourceId,
    SkResourceExt,
};


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
    owners: HashMap<KubeResourceId, Vec<metav1::OwnerReference>>,
}

impl OwnersCache {
    pub fn new(apiset: DynamicApiSet) -> OwnersCache {
        OwnersCache { apiset, owners: HashMap::new() }
    }

    pub fn new_from_parts(
        apiset: DynamicApiSet,
        owners: HashMap<KubeResourceId, Vec<metav1::OwnerReference>>,
    ) -> OwnersCache {
        OwnersCache { apiset, owners }
    }

    // Recursively look up all of the owning objects for a given Kubernetes object
    #[async_recursion]
    pub async fn compute_owners_for(
        &mut self,
        obj: &(impl Resource + SkResourceExt + Sync),
    ) -> Vec<metav1::OwnerReference> {
        let resource_id = obj.resource_id();

        debug!("computing owner references for {resource_id}");
        if let Some(owners) = self.owners.get(&resource_id) {
            debug!("found owners {owners:?} for {resource_id} in cache");
            return owners.clone();
        }

        // Requires that the object's owner references haven't been sanitized away
        let mut owners = Vec::new();

        for rf in obj.owner_references() {
            let owner_gvk = match GVK::from_owner_ref(rf) {
                Ok(gvk) => gvk,
                Err(err) => {
                    error!("malformed owner reference {rf:?}: {err}");
                    continue;
                },
            };
            let (api, cap) = match self.apiset.unnamespaced_api_by_gvk(&owner_gvk).await {
                Ok((a, c)) => (a, c),
                Err(err) => {
                    // Just a warning because it may be some CRD we intentionally haven't installed
                    warn!("could not query {owner_gvk}: {err}; skipping ownerref");
                    continue;
                },
            };
            let sel = build_owner_selector(&rf.name, obj, cap);
            let items = match api.list(&sel).await {
                Ok(objlist) => objlist.items,
                Err(err) => {
                    error!("Could not list {owner_gvk}: {err}; skipping ownerref");
                    continue;
                },
            };

            if items.len() != 1 {
                error!("could not find single owner for {resource_id}, found {items:?}; skipping ownerref");
                continue;
            }

            owners.push(rf.clone());
            owners.extend(self.compute_owners_for(&items[0]).await);
        }

        debug!("computed owners {owners:?} for {resource_id}");
        self.owners.insert(resource_id, owners.clone());
        owners
    }

    pub async fn lookup_by_name_or_obj(
        &mut self,
        resource_id: &KubeResourceId,
        maybe_obj: Option<&(impl Resource + SkResourceExt + Sync)>,
    ) -> Vec<metav1::OwnerReference> {
        match (self.owners.get(resource_id), maybe_obj) {
            (Some(o), _) => o.clone(),
            (None, Some(obj)) => self.compute_owners_for(obj).await,
            _ => {
                error!("could not determine owner chain for {resource_id}");
                vec![]
            },
        }
    }
}

fn build_owner_selector(
    owner_name: &str,
    obj: &(impl Resource + SkResourceExt + Sync),
    owner_cap: ApiCapabilities,
) -> ListParams {
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
