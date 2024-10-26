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
use sk_store::TracerConfig;

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

impl Diagnostic for ServiceAccountMissing {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent, config: &TracerConfig) -> CheckResult {
        for obj in &event.data.applied_objs {
            if let Some(ref type_meta) = obj.types {
                if &type_meta.kind == "ServiceAccount" {
                    self.seen_service_accounts.insert(obj.namespaced_name());
                }
            }
        }
        for obj in &event.data.deleted_objs {
            if let Some(ref type_meta) = obj.types {
                if &type_meta.kind == "ServiceAccount" {
                    self.seen_service_accounts.remove(&obj.namespaced_name());
                }
            }
        }

        let mut patches = vec![];
        for (i, obj) in event.data.applied_objs.iter().enumerate() {
            let gvk = GVK::from_dynamic_obj(obj)?;
            if let Some(pod_spec_template_path) = config.pod_spec_template_path(&gvk) {
                let sa_ptrs = [
                    // serviceAccount is deprecated but still supported (for now)
                    format_ptr!("{pod_spec_template_path}/spec/serviceAccount"),
                    format_ptr!("{pod_spec_template_path}/spec/serviceAccountName"),
                ];
                if let Some(sa) = sa_ptrs.iter().filter_map(|ptr| ptr.resolve(&obj.data).ok()).next() {
                    if !self.seen_service_accounts.contains(sa.as_str().expect("expected string")) {
                        let fix = AnnotatedTracePatch {
                            locations: PatchLocations::ObjectReference(
                                obj.types.clone().unwrap_or_default(),
                                obj.namespaced_name(),
                            ),
                            ops: sa_ptrs.iter().map(|ptr| remove_operation(ptr.clone())).collect(),
                        };
                        patches.push((i, vec![fix]));
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

pub fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "service_account_missing",
        help: HELP,
        diagnostic: Arc::new(RwLock::new(ServiceAccountMissing::default())),
    }
}
