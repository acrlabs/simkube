use json_patch_ext::PointerBuf;
use sk_core::prelude::*;

use crate::validation::ValidatorType;
use crate::validation::rules::rules_core::{RulePath, RuleScope};
use crate::validation::validator::{Diagnostic, Validator};

// This Validator rule implements a generic deny list for keys
// any key paths provided will be raised in diagnostic
// this seaches the entire event object including metadata
pub(super) struct DenyList {
    paths: RulePath,
}

impl DenyList {
    pub(super) fn new(paths: RulePath) -> DenyList {
        DenyList { paths }
    }
}
impl Diagnostic for DenyList {
    fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        let mut indices = Vec::new();
        for (i, obj) in event.applied_objs.iter().enumerate() {
            let gvk = &GVK::from_dynamic_obj(obj)?;
            let scoped_paths: Vec<PointerBuf> = self.paths.get_scoped_paths(config, gvk);
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
const SIMKUBEIO_POD_HELP: &str = r#" Indicates that there is a simkube.io key in a trace event,
simkubeio keys are implementation details that should not appear in trace files"#;
pub fn simkube_io_deny_validator() -> Validator {
    Validator {
        type_: ValidatorType::Error,
        name: "simkube_io_found",
        help: SIMKUBEIO_POD_HELP,
        skel_suggestion: SIMKUBEIO_JOB_SKEL,
        diagnostic: Box::new(DenyList::new(
            RulePath::scoped_by(
                RulePath::multi(vec![
                    RulePath::templates().add(vec!["/spec/nodeSelector", "/spec/*/tolerations/key"]),
                    RulePath::root(),
                ])
                .add(vec!["/metadata/labels", "/metadata/annotations"]),
            )
            .add(vec![
                "/simkube.io~1kwok-stage-complete",
                "/simkube.io~1kwok-stage-complete-time",
                "/simkube.io~1kwok-stage-error-time",
                "/simkube.io~1kwok-stage-ready-delay",
                "/simkube.io~1kwok-stage-create-delay",
                "/simkube.io~1kwok-stage-ready-delay-jitter",
                "/simkube.io~1kwok-stage-create-delay-jitter",
            ]),
        )),
    }
}
