use insta::assert_snapshot;

use super::*;
use crate::validation::{
    AnnotatedTrace,
    VALIDATORS,
};

#[rstest]
fn itest_validate_all_rules() {
    let mut annotated_trace = AnnotatedTrace::new_from_test_json("validation_trace");
    let summary = VALIDATORS.validate_trace(&mut annotated_trace, true).unwrap();
    let events: Vec<_> = annotated_trace.events.iter().map(|a_event| a_event.data.clone()).collect();
    let snapshot = format!("{summary}\n\n{events:#?}");
    assert_snapshot!(snapshot);
}
