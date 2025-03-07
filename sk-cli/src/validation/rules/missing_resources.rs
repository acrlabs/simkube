use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::{
    Arc,
    RwLock,
};

use const_format::formatcp;
use corev1::{
    ConfigMap,
    Secret,
    ServiceAccount,
};
use json_patch_ext::prelude::*;
use k8s_openapi::Resource;
use serde_json::json;
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
    ($res:expr, $typestr:literal) => {
        formatcp!(
            r#"A Pod needs a {resource} {typestr} that is not present in
the trace file.  The simulation will fail because pods cannot be created
if the {resource} does not exist."#,
            resource = $res,
            typestr = $typestr,
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
                    println!("{path:?}, {res}");
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
        MissingResourceType::EnvVar => {
            let env_index_path = get_env_index_path(&path);
            vec![remove_operation(env_index_path)]
        },
        MissingResourceType::TopLevel => vec![remove_operation(path)],
        MissingResourceType::Volume => {
            // Rather than trying to remove the volume reference from all of the
            // potential containers it might be present in, we just replace it
            // with an empty dir volume.  These unwraps should be safe based on the
            // paths we pass in from the validator (they're not user-generated in other words)
            let (remove_path, _) = path.split_back().unwrap();
            let (volume_root, _) = remove_path.split_back().unwrap();
            let empty_dir_path = volume_root.with_trailing_token("emptyDir");
            vec![remove_operation(remove_path.to_buf()), add_operation(empty_dir_path, json!({}))]
        },
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

fn get_env_index_path(path: &Pointer) -> PointerBuf {
    let (mut seen_parent, mut seen_index) = (false, false);
    PointerBuf::from_tokens(path.tokens().take_while(|t| {
        if seen_index {
            return false;
        } else if seen_parent {
            seen_index = true;
        } else if *t == Token::new("env") || *t == Token::new("envFrom") {
            seen_parent = true;
        }
        true
    }))
}

pub fn service_account_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "service_account_missing",
        help: resource_help!(ServiceAccount::KIND, "resource"),
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
        help: resource_help!(Secret::KIND, "environment variable"),
        diagnostic: Arc::new(RwLock::new(MissingResource::<Secret>::new(
            vec!["/spec/containers/*/env/*/valueFrom/secretKeyRef/key", "/spec/containers/*/envFrom/*/secretRef/name"],
            MissingResourceType::EnvVar,
        ))),
    }
}

pub fn configmap_envvar_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "envvar_configmap_missing",
        help: resource_help!(ConfigMap::KIND, "environment variable"),
        diagnostic: Arc::new(RwLock::new(MissingResource::<ConfigMap>::new(
            vec![
                "/spec/containers/*/env/*/valueFrom/configMapKeyRef/key",
                "/spec/containers/*/envFrom/*/configMapRef/name",
            ],
            MissingResourceType::EnvVar,
        ))),
    }
}

pub fn secret_volume_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "volume_secret_missing",
        help: resource_help!(Secret::KIND, "volume"),
        diagnostic: Arc::new(RwLock::new(MissingResource::<Secret>::new(
            vec!["/spec/volumes/*/secret/secretName"],
            MissingResourceType::Volume,
        ))),
    }
}

pub fn configmap_volume_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "volume_configmap_missing",
        help: resource_help!(ConfigMap::KIND, "volume"),
        diagnostic: Arc::new(RwLock::new(MissingResource::<ConfigMap>::new(
            vec!["/spec/volumes/*/configMap/name"],
            MissingResourceType::Volume,
        ))),
    }
}
