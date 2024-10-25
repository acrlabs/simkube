mod status_field_populated_test;
mod validation_store_test;

use rstest::*;
use sk_core::k8s::testutils::test_deployment;
use sk_store::TraceEvent;

use super::annotated_trace::AnnotatedTraceEvent;
use super::*;

#[fixture]
pub fn annotated_trace() -> AnnotatedTrace {
    AnnotatedTrace {
        events: vec![
            AnnotatedTraceEvent {
                data: TraceEvent { ts: 0, ..Default::default() },
                ..Default::default()
            },
            AnnotatedTraceEvent {
                data: TraceEvent {
                    ts: 1,
                    applied_objs: vec![test_deployment("test_depl1")],
                    deleted_objs: vec![],
                },
                ..Default::default()
            },
            AnnotatedTraceEvent {
                data: TraceEvent {
                    ts: 2,
                    applied_objs: vec![test_deployment("test_depl1"), test_deployment("test_depl2")],
                    deleted_objs: vec![],
                },
                ..Default::default()
            },
            AnnotatedTraceEvent {
                data: TraceEvent {
                    ts: 3,
                    applied_objs: vec![],
                    deleted_objs: vec![test_deployment("test_depl1")],
                },
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
