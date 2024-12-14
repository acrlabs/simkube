use std::sync::{
    Arc,
    RwLock,
};

use json_patch_ext::prelude::*;
use lazy_static::lazy_static;

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

const HELP: &str = r#"Indicates that the status field of a Kubernetes object in
the trace is non-empty; status fields are updated by their controlling objects
and shouldn't be applied "by hand".  This is probably "fine" but it would be
better to clean them up (and also they take up a lot of space."#;

#[derive(Default)]
pub struct StatusFieldPopulated {}

lazy_static! {
    static ref FIX: AnnotatedTracePatch = AnnotatedTracePatch {
        locations: PatchLocations::Everywhere,
        ops: vec![remove_operation(format_ptr!("/status"))],
    };
}

impl Diagnostic for StatusFieldPopulated {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent) -> CheckResult {
        Ok(event
            .data
            .applied_objs
            .iter()
            .enumerate()
            .filter_map(|(i, obj)| {
                if obj
                    .data
                    .as_object()
                    .expect("DynamicObject data should be a map")
                    .get("status")
                    .is_some_and(|v| !v.is_null())
                {
                    return Some((i, vec![FIX.clone()]));
                }
                None
            })
            .collect())
    }

    fn reset(&mut self) {}
}

pub fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "status_field_populated",
        help: HELP,
        diagnostic: Arc::new(RwLock::new(StatusFieldPopulated::default())),
    }
}
