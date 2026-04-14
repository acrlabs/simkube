use std::collections::HashSet;
use std::marker::PhantomData;

use const_format::formatcp;
use corev1::{
    ConfigMap,
    Secret,
    ServiceAccount,
};
use json_patch_ext::prelude::*;
use k8s_openapi::Resource;
use sk_core::k8s::GVK;
use sk_core::prelude::*;
use sk_store::{
    TraceEvent,
    TracerConfig,
};

use crate::validation::validator::{
    Diagnostic,
    Validator,
    ValidatorType,
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

pub struct MissingResource<T: Resource> {
    pub(super) seen_resources: HashSet<String>,
    ptrs: Vec<&'static str>,

    _resource_type: PhantomData<T>,
}

impl<T: Resource> MissingResource<T> {
    // The list of pointer strings are relative to the PodSpec (they will have podSpecTemplatePath
    // prepended to them); they should start with a leading '/'
    pub(super) fn new(ptrs: Vec<&'static str>) -> MissingResource<T> {
        MissingResource {
            seen_resources: HashSet::new(),
            ptrs,

            _resource_type: PhantomData,
        }
    }

    fn record_resources(&mut self, event: &TraceEvent) {
        for obj in &event.applied_objs {
            if let Some(ref type_meta) = obj.types
                && type_meta.kind == T::KIND
            {
                self.seen_resources.insert(obj.namespaced_name());
            }
        }
        for obj in &event.deleted_objs {
            if let Some(ref type_meta) = obj.types
                && type_meta.kind == T::KIND
            {
                self.seen_resources.remove(&obj.namespaced_name());
            }
        }
    }
}

impl<T: Resource> Diagnostic for MissingResource<T> {
    fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        // First we check all the objects in this event and record any resources we see (and remove
        // any resources that got deleted); this way if the resource and the pod referencing it are
        // created at the same time we don't fail (maybe we should, though?  not sure, anyways it's
        // fine for now).
        self.record_resources(event);

        let mut indices = Vec::new();
        for (i, obj) in event.applied_objs.iter().enumerate() {
            let gvk = GVK::from_dynamic_obj(obj)?;
            if let Some(pod_spec_template_paths) = config.pod_spec_template_paths(&gvk) {
                for pod_spec_template_path in pod_spec_template_paths {
                    let ptrs: Vec<_> = self
                        .ptrs
                        .iter()
                        .map(|pstr| format_ptr!("{pod_spec_template_path}{pstr}"))
                        .collect();
                    let matched_values = ptrs.iter().flat_map(|ptr| matches(ptr, &obj.data));

                    for (_, res) in matched_values {
                        // if we're demanding a resource for a pod, we assume the resource is
                        // namespaced (may be an invalid assumption in the future)
                        let resource_ns = &obj.namespace().unwrap();
                        let resource_name = res.as_str().unwrap();

                        if !self.seen_resources.contains(&format!("{resource_ns}/{resource_name}")) {
                            indices.push(i);
                        }
                    }
                }
            }
        }

        Ok(indices)
    }
}

// TODO (SK-204) replace the SKEL strings with the magic pts variable
const SERVICE_ACCOUNT_SKEL: &str = r#"
# these paths are only correct for Deployments, adjust as needed for other resources
remove(spec.template.spec.serviceAccount);
remove(spec.template.spec.serviceAccountName);
"#;
pub fn service_account_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "service_account_missing",
        help: resource_help!(ServiceAccount::KIND, "resource"),
        skel_suggestion: SERVICE_ACCOUNT_SKEL,
        diagnostic: Box::new(MissingResource::<ServiceAccount>::new(vec![
            // serviceAccount is deprecated but still supported (for now)
            "/spec/serviceAccount",
            "/spec/serviceAccountName",
        ])),
    }
}

const SECRET_ENVVAR_SKEL: &str = r#"
# these paths are only correct for Deployments, adjust as needed for other resources
remove($x := spec.template.spec.containers[*].env[*] | exists($x.valueFrom.secretKeyRef), $x);
remove($x := spec.template.spec.containers[*].envFrom[*] | exists($x.secretRef), $x);
"#;
pub fn secret_envvar_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "envvar_secret_missing",
        help: resource_help!(Secret::KIND, "environment variable"),
        skel_suggestion: SECRET_ENVVAR_SKEL,
        diagnostic: Box::new(MissingResource::<Secret>::new(vec![
            "/spec/containers/*/env/*/valueFrom/secretKeyRef/key",
            "/spec/containers/*/envFrom/*/secretRef/name",
        ])),
    }
}

const CONFIGMAP_ENVVAR_SKEL: &str = r#"
# these paths are only correct for Deployments, adjust as needed for other resources
remove($x := spec.template.spec.containers[*].env[*] | exists($x.valueFrom.configMapKeyRef), $x);
remove($x := spec.template.spec.containers[*].envFrom[*] | exists($x.configMapRef), $x);
"#;
pub fn configmap_envvar_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "envvar_configmap_missing",
        help: resource_help!(ConfigMap::KIND, "environment variable"),
        skel_suggestion: CONFIGMAP_ENVVAR_SKEL,
        diagnostic: Box::new(MissingResource::<ConfigMap>::new(vec![
            "/spec/containers/*/env/*/valueFrom/configMapKeyRef/key",
            "/spec/containers/*/envFrom/*/configMapRef/name",
        ])),
    }
}

const SECRET_VOLUME_SKEL: &str = r#"
# these paths are only correct for Deployments, adjust as needed for other resources
remove($x := spec.template.spec.volumes[*] | exists($x.secret)
    && $y := spec.template.spec.containers[*].volumeMounts[*] | $y.name == $x.name,
    $y);
remove($x := spec.template.spec.volumes[*] | exists($x.secret), $x);
"#;
pub fn secret_volume_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "volume_secret_missing",
        help: resource_help!(Secret::KIND, "volume"),
        skel_suggestion: SECRET_VOLUME_SKEL,
        diagnostic: Box::new(MissingResource::<Secret>::new(vec!["/spec/volumes/*/secret/secretName"])),
    }
}

const CONFIGMAP_VOLUME_SKEL: &str = r#"
# these paths are only correct for Deployments, adjust as needed for other resources
remove($x := spec.template.spec.volumes[*] | exists($x.configMap)
    && $y := spec.template.spec.containers[*].volumeMounts[*] | $y.name == $x.name,
    $y);
remove($x := spec.template.spec.volumes[*] | exists($x.configMap), $x);
"#;
pub fn configmap_volume_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "volume_configmap_missing",
        help: resource_help!(ConfigMap::KIND, "volume"),
        skel_suggestion: CONFIGMAP_VOLUME_SKEL,
        diagnostic: Box::new(MissingResource::<ConfigMap>::new(vec!["/spec/volumes/*/configMap/name"])),
    }
}
