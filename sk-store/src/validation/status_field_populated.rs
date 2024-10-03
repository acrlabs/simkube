use super::{
    Validator,
    ValidatorType,
};
use crate::TraceEvent;
fn check(_events: &[TraceEvent]) -> Vec<(usize, usize)> {
    vec![]
}

pub(super) fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "status_field_populated".into(),
        help: r#"
Indicates that the status field of a Kubernetes object in the trace is
non-empty; status fields are updated by their controlling objects and shouldn't
be applied "by hand".  This is probably "fine" but it would be better to clean
them up (and also they take up a lot of space.
"#
        .into(),
        check: Box::new(check),
    }
}
