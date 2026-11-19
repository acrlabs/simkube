use sk_core::prelude::*;

use crate::validation::validator::{
    Diagnostic,
    Validator,
    ValidatorType,
};

const HELP: &str = r#"Indicates that some "simkube.io/..." labels or annotations
are present in the trace file; you likely took this trace from a simulated cluster.
It probably will work fine anyways but you may wish to clean these up before running."#;

const SKEL_SUGGESTION: &str = r#"
# WARNING, some of this syntax is currently unsupported by SKEL, but will be soon!
remove(metadata.labels."simkube.io/...");
remove(metadata.annotations."simkube.io/...");
remove(@pst.labels."simkube.io/...");
remove(@pst.annotations."simkube.io/...");
remove(@pst.nodeSelector."simkube.io/...");
remove($x := @pst.tolerations[*] | $x.key == "simkube.io/...", $x);
"#;

const PST_OBJECT_PATHS: &[&str] = &["/metadata/annotations", "/metadata/labels", "/spec/nodeSelector"];

#[derive(Default)]
pub struct SimkubeLabels {}

impl Diagnostic for SimkubeLabels {
    fn check_next_event(&mut self, event: &TraceEvent, config: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        let mut indices = Vec::new();
        for (i, obj) in event.applied_objs.iter().enumerate() {
            let gvk = GVK::from_dynamic_obj(obj)?;
            let mut matches = obj.labels().keys().any(|k| k.starts_with("simkube.io"))
                || obj.annotations().keys().any(|k| k.starts_with("simkube.io"));

            if !matches && let Some(pod_spec_template_paths) = config.pod_spec_template_paths(&gvk) {
                for pod_spec_template_path in pod_spec_template_paths {
                    for subpath in PST_OBJECT_PATHS {
                        let ptr_str = format!("{pod_spec_template_path}{subpath}");
                        if let Some(node_sel) = obj.data.pointer(&ptr_str)
                            && node_sel
                                .as_object()
                                .expect("{subpath} should be an object")
                                .keys()
                                .any(|k| k.starts_with("simkube.io"))
                        {
                            matches = true;
                            break;
                        }
                    }

                    let tolerations_pointer = format!("{pod_spec_template_path}/spec/tolerations");
                    if let Some(tolerations) = obj.data.pointer(&tolerations_pointer)
                        && tolerations.as_array().expect("tolerations should be an array").iter().any(|t| {
                            t.get("key")
                                .unwrap_or_default()
                                .as_str()
                                .expect("toleration key should be a string")
                                .starts_with("simkube.io")
                        })
                    {
                        matches = true;
                    }
                }
            }

            if matches {
                indices.push(i);
            }
        }

        Ok(indices)
    }
}

pub fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "simkube_labels",
        help: HELP,
        skel_suggestion: SKEL_SUGGESTION,
        diagnostic: Box::new(SimkubeLabels::default()),
    }
}
