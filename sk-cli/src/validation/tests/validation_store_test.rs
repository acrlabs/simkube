use assertables::*;

use super::*;

#[rstest]
fn test_validate_trace(mut test_validation_store: ValidationStore, mut annotated_trace: AnnotatedTrace) {
    test_validation_store.validate_trace(&mut annotated_trace);

    for evt in annotated_trace.iter() {
        if evt.data.applied_objs.len() > 1 {
            assert_eq!(evt.annotations[1], vec![TEST_VALIDATOR_CODE]);
        } else {
            for annotation in &evt.annotations {
                assert_is_empty!(annotation);
            }
        }
    }

    assert_eq!(annotated_trace.summary_for(&TEST_VALIDATOR_CODE).unwrap(), 1);
}
