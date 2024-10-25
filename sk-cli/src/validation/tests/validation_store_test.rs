use std::collections::BTreeMap;

use assertables::*;

use super::validator::{
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
    let code = "W9999";
    let mut test_store = ValidationStore { validators: BTreeMap::new() };
    test_store.register_with_code(code.into(), validator);

    test_store.validate_trace(&mut annotated_trace);

    for evt in annotated_trace.iter() {
        if evt.data.applied_objs.len() > 1 {
            assert_eq!(evt.annotations, vec![(1, code.into())]);
        } else {
            assert_is_empty!(evt.annotations);
        }
    }

    assert_eq!(annotated_trace.summary_for(code).unwrap(), 1);
}
