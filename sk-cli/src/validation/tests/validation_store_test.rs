use assertables::*;

use super::*;

#[rstest]
fn test_validate_trace(mut test_validation_store: ValidationStore, trace: ExportedTrace) {
    let all_annotations = test_validation_store.validate_trace(&trace).unwrap();

    for (i, (event, _)) in trace.iter().enumerate() {
        if event.applied_objs.len() > 1 {
            assert_bag_eq!(all_annotations[&i][&TEST_VALIDATOR_CODE], [1]);
        } else {
            assert_is_empty!(all_annotations[&i][&TEST_VALIDATOR_CODE]);
        }
    }
}
