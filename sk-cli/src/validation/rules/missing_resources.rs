use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::{
    Arc,
    RwLock,
};

use const_format::formatcp;
use corev1::{
    Secret,
    ServiceAccount,
};
use json_patch_ext::prelude::*;
use k8s_openapi::Resource;
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

// Defined as a macro so we can re-use it in format! macros
macro_rules! resource_help {
    ($res:expr) => {
        formatcp!(
            r#"A Pod needs a {resource} that is not present in
the trace file.  The simulation will fail because pods cannot be created
if the {resource} does not exist."#,
            resource = $res
        )
    };
}

#[derive(Clone, Copy)]
pub(super) enum MissingResourceType {
    EnvVar,
    TopLevel,
    #[allow(dead_code)]
    Volume,
}

pub struct MissingResource<T: Resource> {
    pub(super) seen_resources: HashSet<String>,
    ptrs: Vec<&'static str>,
    type_: MissingResourceType,

    _resource_type: PhantomData<T>,
}

impl<T: Resource> MissingResource<T> {
    // The list of pointer strings are relative to the PodSpec (they will have podSpecTemplatePath
    // prepended to them); they should start with a leading '/'
    pub(super) fn new(ptrs: Vec<&'static str>, type_: MissingResourceType) -> MissingResource<T> {
        MissingResource {
            seen_resources: HashSet::new(),
            ptrs,
            type_,

            _resource_type: PhantomData,
        }
    }

    fn record_resources(&mut self, event: &mut AnnotatedTraceEvent) {
        for obj in &event.data.applied_objs {
            if let Some(ref type_meta) = obj.types {
                if type_meta.kind == T::KIND {
                    self.seen_resources.insert(obj.namespaced_name());
                }
            }
        }
        for obj in &event.data.deleted_objs {
            if let Some(ref type_meta) = obj.types {
                if type_meta.kind == T::KIND {
                    self.seen_resources.remove(&obj.namespaced_name());
                }
            }
        }
    }
}

impl<T: Resource> Diagnostic for MissingResource<T> {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent, config: &TracerConfig) -> CheckResult {
        // First we check all the objects in this event and record any resources we see (and remove
        // any resources that got deleted); this way if the resource and the pod referencing it are
        // created at the same time we don't fail (maybe we should, though?  not sure, anyways it's
        // fine for now).
        self.record_resources(event);

        let mut patches = vec![];
        for (i, obj) in event.data.applied_objs.iter().enumerate() {
            let gvk = GVK::from_dynamic_obj(obj)?;
            if let Some(pod_spec_template_path) = config.pod_spec_template_path(&gvk) {
                let ptrs: Vec<_> = self
                    .ptrs
                    .iter()
                    .map(|pstr| format_ptr!("{pod_spec_template_path}{pstr}"))
                    .collect();
                let matched_values = ptrs.iter().flat_map(|ptr| matches(ptr, &obj.data));

                // If this obj references a resource that doesn't exist at this point in
                // time, there are two possible fixes:
                //
                // 1) remove the reference to the resource from the pod template spec (recommended, because the pod
                //    won't exist and can't actually _do_ anything anyways), or
                // 2) add the resource object in at the beginning of the simulation
                for (path, res) in matched_values {
                    // if we're demanding a resource for a pod, we assume the resource is
                    // namespaced (may be an invalid assumption in the future)
                    let resource_ns = &obj.namespace().unwrap();
                    let resource_name = res.as_str().unwrap();

                    if !self.seen_resources.contains(&format!("{resource_ns}/{resource_name}")) {
                        let (remove_patch, add_patch) =
                            make_remove_add_patches(T::type_meta(), self.type_, resource_ns, resource_name, path);
                        patches.push((i, vec![remove_patch, add_patch]));
                    }
                }
            }
        }

        Ok(patches)
    }

    fn reset(&mut self) {
        self.seen_resources.clear();
    }
}

fn make_remove_add_patches(
    type_meta: TypeMeta,
    missing_type: MissingResourceType,
    resource_ns: &str,
    resource_name: &str,
    path: PointerBuf,
) -> (AnnotatedTracePatch, AnnotatedTracePatch) {
    let remove_ops = match missing_type {
        MissingResourceType::EnvVar => unimplemented!(),
        MissingResourceType::TopLevel => vec![remove_operation(path)],
        MissingResourceType::Volume => unimplemented!(),
    };
    (
        AnnotatedTracePatch {
            locations: PatchLocations::ObjectReference(type_meta.clone(), format!("{resource_ns}/{resource_name}")),
            ops: remove_ops,
        },
        AnnotatedTracePatch {
            locations: PatchLocations::InsertAt(
                0,
                TraceAction::ObjectApplied,
                type_meta,
                metav1::ObjectMeta {
                    namespace: Some(resource_ns.into()),
                    name: Some(resource_name.into()),
                    ..Default::default()
                },
            ),
            ops: vec![],
        },
    )
}


pub fn service_account_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "service_account_missing",
        help: resource_help!(ServiceAccount::KIND),
        diagnostic: Arc::new(RwLock::new(MissingResource::<ServiceAccount>::new(
            vec![
                // serviceAccount is deprecated but still supported (for now)
                "/spec/serviceAccount",
                "/spec/serviceAccountName",
            ],
            MissingResourceType::TopLevel,
        ))),
    }
}

pub fn secret_envvar_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "envvar_secret_missing",
        help: resource_help!(Secret::KIND),
        diagnostic: Arc::new(RwLock::new(MissingResource::<Secret>::new(
            vec!["/spec/containers/*/env/*/valueFrom/secretKeyRef"],
            MissingResourceType::EnvVar,
        ))),
    }
}
