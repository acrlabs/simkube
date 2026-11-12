use json_patch_ext::PointerBuf;
use sk_core::prelude::*;

use crate::validation::ValidatorType;
use crate::validation::rules::rules_core::RuleScope;
use crate::validation::validator::{
    Diagnostic,
    Validator,
};

// This Validator rule implements a generic deny list for keys
// any key paths provided will be raised in diagnostic
// this seaches the entire event object including metadata
pub(super) struct DenyList {
    ptrs: Vec<&'static str>,
    scope: RuleScope,
}

impl DenyList {
    pub(super) fn new(scope: RuleScope, ptrs: Vec<&'static str>) -> DenyList {
        DenyList { scope, ptrs }
    }
}
impl Diagnostic for DenyList {
    fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        let mut indices = Vec::new();
        for (i, obj) in event.applied_objs.iter().enumerate() {
            let gvk = &GVK::from_dynamic_obj(obj)?;
            let scoped_paths: Vec<PointerBuf> = self.scope.get_scoped_paths(&self.ptrs, config, gvk);
            // dynamic object is 3 fields, a type, a metadata, and a data section, only the last one
            // is json, to search through the full event object we need to convert back to json
            let val = serde_json::to_value(obj)?;
            let matches = scoped_paths
                .iter()
                .flat_map(|pstr| json_patch_ext::prelude::matches(pstr, &val));
            if matches.count() > 0 {
                indices.push(i);
            }
        }

        Ok(indices)
    }
}

// There are more of these in core/constants but this should cover most of the high risk ones
const SIMKUBEIO_JOB_SKEL: &str = r#"
# these paths are only correct for Jobs, adjust as needed for other resources
# remove(kind == "Job", spec.template.metadata.labels."simkube.io/kwok-stage-complete");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-complete-time");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-error-time");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-create-delay");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-ready-delay");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-create-delay-jitter");
# remove(kind == "Job", spec.template.metadata.annotations."simkube.io/kwok-stage-ready-delay-jitter");
"#;
const SIMKUBEIO_POD_HELP: &str = r#" Indicates that there is a simkube.io label/annotation in a trace event resources pod template metadata,
these are created from information stored in the trace at simulation runtime and are used to manage the simulation,
They should not be present in the trace events themselves"#;
pub fn simkube_io_pod_template_deny_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "internal_simkubeio_in_pod_template",
        help: SIMKUBEIO_POD_HELP,
        skel_suggestion: SIMKUBEIO_JOB_SKEL,
        diagnostic: Box::new(DenyList::new(
            RuleScope::PodTemplates,
            vec![
                "/metadata/labels/simkube.io~1kwok-stage-complete",
                "/metadata/annotations/simkube.io~1kwok-stage-complete-time",
                "/metadata/annotations/simkube.io~1kwok-stage-error-time",
                "/metadata/annotations/simkube.io~1kwok-stage-ready-delay",
                "/metadata/annotations/simkube.io~1kwok-stage-create-delay",
                "/metadata/annotations/simkube.io~1kwok-stage-ready-delay-jitter",
                "/metadata/annotations/simkube.io~1kwok-stage-create-delay-jitter",
            ],
        )),
    }
}
const SIMKUBEIO_RESOURCES_SKEL: &str = r#"
# remove(metadata.labels."simkube.io/kwok-stage-complete");
# remove(metadata.annotations."simkube.io/kwok-stage-complete-time");
# remove(metadata.annotations."simkube.io/kwok-stage-error-time");
# remove(metadata.annotations."simkube.io/kwok-stage-create-delay");
# remove(metadata.annotations."simkube.io/kwok-stage-ready-delay");
# remove(metadata.annotations."simkube.io/kwok-stage-create-delay-jitter");
# remove(metadata.annotations."simkube.io/kwok-stage-ready-delay-jitter");
"#;
const SIMKUBEIO_RESOURCES_HELP: &str = r#" Indicates that there is a simkube.io label/annotation in a trace event's metadata,
these are created from information stored in the trace at simulation runtime and are used to manage the simulation,
They should not be present in the trace events themselves"#;
// This rule is probably not doing anything, as I dont think these labels ever get applied outside
// of pod specs
pub fn simkube_io_resource_deny_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "internal_simkubeio_in_resource_meta",
        help: SIMKUBEIO_RESOURCES_HELP,
        skel_suggestion: SIMKUBEIO_RESOURCES_SKEL,
        diagnostic: Box::new(DenyList::new(
            RuleScope::Empty,
            vec![
                "/metadata/labels/simkube.io~1kwok-stage-complete",
                "/metadata/annotations/simkube.io~1kwok-stage-complete-time",
                "/metadata/annotations/simkube.io~1kwok-stage-error-time",
                "/metadata/annotations/simkube.io~1kwok-stage-ready-delay",
                "/metadata/annotations/simkube.io~1kwok-stage-create-delay",
                "/metadata/annotations/simkube.io~1kwok-stage-ready-delay-jitter",
                "/metadata/annotations/simkube.io~1kwok-stage-create-delay-jitter",
            ],
        )),
    }
}
