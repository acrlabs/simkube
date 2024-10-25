use std::collections::BTreeMap;

use assertables::*;

use super::validation_store::{
    Diagnostic,
    Validator,
    ValidatorType,
};
use super::*;

struct TestDiagnostic {}

impl Diagnostic for TestDiagnostic {
    fn check_next_event(&mut self, evt: &mut AnnotatedTraceEvent) -> Vec<usize> {
        if evt.data.applied_objs.len() > 1 {
            vec![1]
        } else {
            vec![]
        }
    }

    fn reset(&mut self) {}
}

#[fixture]
fn validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "test_validator",
        help: "HELP ME, I'M STUCK IN THE BORROW CHECKER",
        diagnostic: Box::new(TestDiagnostic {}),
    }
}

#[rstest]
fn test_validate_trace(validator: Validator, mut annotated_trace: AnnotatedTrace) {
    let mut test_store = ValidationStore { validators: BTreeMap::new() };
    test_store.register(validator);

    test_store.validate_trace(&mut annotated_trace);

    for evt in annotated_trace.events {
        if evt.data.applied_objs.len() > 1 {
            assert_eq!(evt.annotations, vec![(1, "W0000".into())]);
        } else {
            assert_is_empty!(evt.annotations);
        }
    }

    assert_eq!(*annotated_trace.summary.get("W0000").unwrap(), 1);
}
