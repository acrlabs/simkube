mod validation_store_test;

use std::collections::BTreeMap;

use sk_core::prelude::*;
use sk_store::TraceEvent;
use sk_testutils::*;

use super::validator::{
    Diagnostic,
    Validator,
    ValidatorCode,
    ValidatorType,
};
use super::*;

const TEST_VALIDATOR_CODE: ValidatorCode = ValidatorCode(ValidatorType::Error, 9999);

#[fixture]
pub fn trace() -> ExportedTrace {
    ExportedTrace::new_with_events(vec![
        TraceEvent { ts: 0, ..Default::default() },
        TraceEvent {
            ts: 1,
            applied_objs: vec![test_deployment("test_depl1")],
            deleted_objs: vec![],
        },
        TraceEvent {
            ts: 2,
            applied_objs: vec![
                test_deployment("test_depl1"),
                test_deployment(&("test_depl2".to_string() + &"x".repeat(100))),
                test_deployment(&("test_depl3".to_string() + &"x".repeat(100))),
            ],
            deleted_objs: vec![],
        },
        TraceEvent {
            ts: 5,
            applied_objs: vec![],
            deleted_objs: vec![test_deployment("test_depl1")],
        },
    ])
}

struct TestDiagnostic {}

impl Diagnostic for TestDiagnostic {
    fn check_next_event(&mut self, evt: &TraceEvent, _: &TracerConfig) -> anyhow::Result<Vec<usize>> {
        if evt.applied_objs.len() > 1 && evt.applied_objs[1].data.get("foo").is_none() {
            Ok(vec![1])
        } else {
            Ok(vec![])
        }
    }
}

#[fixture]
fn test_validator() -> Validator {
    Validator {
        type_: ValidatorType::Warning,
        name: "test_validator",
        help: "HELP ME, I'M STUCK IN THE BORROW CHECKER",
        skel_suggestion: "remove(*);",
        diagnostic: Box::new(TestDiagnostic {}),
    }
}

#[fixture]
pub fn test_validation_store(test_validator: Validator) -> ValidationStore {
    let validators = BTreeMap::from([(TEST_VALIDATOR_CODE, test_validator)]);
    ValidationStore { validators }
}
