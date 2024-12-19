use std::collections::{
    BTreeMap,
    HashSet,
};
use std::sync::{
    Arc,
    RwLock,
};

use json_patch_ext::prelude::*;
use sk_core::k8s::GVK;
use sk_core::prelude::*;
use sk_store::{
    TraceAction,
    TracerConfig,
};

use crate::validation::validator::{
    CheckResult,
    Diagnostic,
    Validator,
    ValidatorType,
};
use crate::validation::{
    AnnotatedTraceEvent,
    AnnotatedTracePatch,
    PatchLocations,
};

const HELP: &str = r#"A pod needs a service account that is not present in
the trace file.  The simulation will fail because pods cannot be created
if their service account does not exist."#;

#[derive(Default)]
pub struct ServiceAccountMissing {
    pub(crate) seen_service_accounts: HashSet<String>,
}

impl ServiceAccountMissing {
    fn record_service_accounts(&mut self, event: &mut AnnotatedTraceEvent) {
        for obj in &event.data.applied_objs {
            if let Some(ref type_meta) = obj.types {
                if type_meta.kind == SVC_ACCOUNT_KIND {
                    self.seen_service_accounts.insert(obj.namespaced_name());
                }
            }
        }
        for obj in &event.data.deleted_objs {
            if let Some(ref type_meta) = obj.types {
                if type_meta.kind == SVC_ACCOUNT_KIND {
                    self.seen_service_accounts.remove(&obj.namespaced_name());
                }
            }
        }
    }
}

impl Diagnostic for ServiceAccountMissing {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent, config: &TracerConfig) -> CheckResult {
        // First we check all the objects in this event and record any service accounts we see
        // (and remove any service accounts that got deleted); this way if the service account
        // and the pod referencing it are created at the same time we don't fail (maybe we should,
        // though?  not sure, anyways it's fine for now).
        self.record_service_accounts(event);

        let mut patches = vec![];
        for (i, obj) in event.data.applied_objs.iter().enumerate() {
            let gvk = GVK::from_dynamic_obj(obj)?;
            if let Some(pod_spec_template_path) = config.pod_spec_template_path(&gvk) {
                let ptrs = [
                    // serviceAccount is deprecated but still supported (for now)
                    format_ptr!("{pod_spec_template_path}/spec/serviceAccount"),
                    format_ptr!("{pod_spec_template_path}/spec/serviceAccountName"),
                ];

                // If this obj references a service account that doesn't exist at this point in
                // time, there are two possible fixes:
                //
                // 1) remove the reference to the service account from the pod template spec (recommended, because
                //    the pod won't exist and can't actually _do_ anything anyways), or
                // 2) add the service account object in at the beginning of the simulation
                if let Some(sa) = ptrs.iter().filter_map(|ptr| ptr.resolve(&obj.data).ok()).next() {
                    // if we're demanding a service account, we must have a namespace and a name,
                    // these unwraps should be safe
                    let svc_account = sa.as_str().unwrap();
                    let svc_account_ns = &obj.namespace().unwrap();

                    if !self.seen_service_accounts.contains(&format!("{svc_account_ns}/{svc_account}")) {
                        patches.push((
                            i,
                            vec![
                                construct_remove_svc_account_ref_patch(obj, &ptrs),
                                construct_add_svc_account_patch(svc_account_ns, svc_account),
                            ],
                        ));
                    }
                }
            }
        }

        Ok(BTreeMap::from_iter(patches))
    }

    fn reset(&mut self) {
        self.seen_service_accounts.clear();
    }
}

fn construct_remove_svc_account_ref_patch(obj: &DynamicObject, ptrs: &[PointerBuf]) -> AnnotatedTracePatch {
    AnnotatedTracePatch {
        locations: PatchLocations::ObjectReference(obj.types.clone().unwrap_or_default(), obj.namespaced_name()),
        ops: ptrs.iter().map(|ptr| remove_operation(ptr.clone())).collect(),
    }
}

fn construct_add_svc_account_patch(svc_account_ns: &str, svc_account: &str) -> AnnotatedTracePatch {
    AnnotatedTracePatch {
        locations: PatchLocations::InsertAt(
            0,
            TraceAction::ObjectApplied,
            SVC_ACCOUNT_GVK.into_type_meta(),
            metav1::ObjectMeta {
                name: Some(svc_account.into()),
                namespace: Some(svc_account_ns.into()),
                ..Default::default()
            },
        ),
        ops: vec![],
    }
}

pub fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "service_account_missing",
        help: HELP,
        diagnostic: Arc::new(RwLock::new(ServiceAccountMissing::default())),
    }
}
