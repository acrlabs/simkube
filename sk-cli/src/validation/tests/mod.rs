mod annotated_trace_test;
mod validation_store_test;

use std::collections::BTreeMap;
use std::sync::{
    Arc,
    RwLock,
};

use json_patch_ext::prelude::*;
use rstest::*;
use sk_core::prelude::*;
use sk_store::{
    TraceEvent,
    TracerConfig,
};
use sk_testutils::*;

use super::annotated_trace::AnnotatedTraceEvent;
use super::validator::{
    CheckResult,
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
            applied_objs: vec![
                test_deployment("test_depl1"),
                test_deployment(&("test_depl2".to_string() + &"x".repeat(100))),
                test_deployment(&("test_depl3".to_string() + &"x".repeat(100))),
            ],
            deleted_objs: vec![],
        }),
        AnnotatedTraceEvent::new(TraceEvent {
            ts: 5,
            applied_objs: vec![],
            deleted_objs: vec![test_deployment("test_depl1")],
        }),
    ])
}

struct TestDiagnostic {}

impl Diagnostic for TestDiagnostic {
    fn check_next_event(&mut self, evt: &mut AnnotatedTraceEvent, _: &TracerConfig) -> CheckResult {
        if evt.data.applied_objs.len() > 1 && evt.data.applied_objs[1].data.get("foo").is_none() {
            Ok(vec![(
                1,
                vec![AnnotatedTracePatch {
                    locations: PatchLocations::Everywhere,
                    ops: vec![add_operation(format_ptr!("/foo"), "bar".into())],
                }],
            )])
        } else {
            Ok(vec![])
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
        diagnostic: Arc::new(RwLock::new(TestDiagnostic {})),
    }
}

#[fixture]
pub fn test_validation_store(test_validator: Validator) -> ValidationStore {
    let validators = BTreeMap::from([(TEST_VALIDATOR_CODE, test_validator)]);
    ValidationStore { validators }
}

pub fn annotated_trace_from_json(trace_type: &str) -> AnnotatedTrace {
    let exported_trace = exported_trace_from_json(trace_type);
    AnnotatedTrace::new_with_events(
        exported_trace
            .events()
            .iter()
            .cloned()
            .map(|e| AnnotatedTraceEvent::new(e))
            .collect(),
    )
}
