mod status_field_populated_test;
mod validation_store_test;

use std::collections::BTreeMap;

use rstest::*;
use sk_core::prelude::*;
use sk_store::TraceEvent;

use super::annotated_trace::AnnotatedTraceEvent;
use super::validator::{
    Diagnostic,
    Validator,
    ValidatorCode,
    ValidatorType,
};
use super::*;

const TEST_VALIDATOR_CODE: ValidatorCode = ValidatorCode(ValidatorType::Error, 9999);

#[fixture]
pub fn annotated_trace() -> AnnotatedTrace {
    AnnotatedTrace::new_with_events(vec![
        AnnotatedTraceEvent::new(TraceEvent { ts: 0, ..Default::default() }),
        AnnotatedTraceEvent::new(TraceEvent {
            ts: 1,
            applied_objs: vec![test_deployment("test_depl1")],
            deleted_objs: vec![],
        }),
        AnnotatedTraceEvent::new(TraceEvent {
            ts: 2,
            applied_objs: vec![test_deployment("test_depl1"), test_deployment("test_depl2")],
            deleted_objs: vec![],
        }),
        AnnotatedTraceEvent::new(TraceEvent {
            ts: 3,
            applied_objs: vec![],
            deleted_objs: vec![test_deployment("test_depl1")],
        }),
    ])
}

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
fn test_validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "test_validator",
        help: "HELP ME, I'M STUCK IN THE BORROW CHECKER",
        diagnostic: Box::new(TestDiagnostic {}),
    }
}

#[fixture]
pub fn test_validation_store(test_validator: Validator) -> ValidationStore {
    let mut test_store = ValidationStore { validators: BTreeMap::new() };
    test_store.register_with_code(TEST_VALIDATOR_CODE, test_validator);
    test_store
}
