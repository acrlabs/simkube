use super::annotated_trace::AnnotatedTraceEvent;
use super::validation_store::{
    Diagnostic,
    Validator,
    ValidatorType,
};

const HELP: &str = r#"Indicates that the status field of a Kubernetes object in
the trace is non-empty; status fields are updated by their controlling objects
and shouldn't be applied "by hand".  This is probably "fine" but it would be
better to clean them up (and also they take up a lot of space."#;

#[derive(Default)]
pub(super) struct StatusFieldPopulated {}

impl Diagnostic for StatusFieldPopulated {
    fn check_next_event(&mut self, event: &mut AnnotatedTraceEvent) -> Vec<usize> {
        event
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
                    return Some(i);
                }
                None
            })
            .collect()
    }

    fn reset(&mut self) {}
}

pub(super) fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "status_field_populated",
        help: HELP,
        diagnostic: Box::new(StatusFieldPopulated::default()),
    }
}
