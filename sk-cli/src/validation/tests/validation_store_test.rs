use assertables::*;

use super::*;

#[rstest]
fn test_validate_trace(test_validation_store: ValidationStore, mut annotated_trace: AnnotatedTrace) {
    let summary = test_validation_store.validate_trace(&mut annotated_trace, false).unwrap();

    for event in annotated_trace.iter() {
        if event.data.applied_objs.len() > 1 {
            assert_eq!(event.annotations[1], vec![TEST_VALIDATOR_CODE]);
        } else {
            for annotation in &event.annotations {
                assert_is_empty!(annotation);
            }
        }
    }

    assert_eq!(*summary.annotations.get(&TEST_VALIDATOR_CODE).unwrap(), 1);
    assert_eq!(summary.patches, 0);
}

#[rstest]
fn test_fix_trace(test_validation_store: ValidationStore, mut annotated_trace: AnnotatedTrace) {
    let summary = test_validation_store.validate_trace(&mut annotated_trace, true).unwrap();

    for event in annotated_trace.iter() {
        println!("{:?}", event.data);
        if event.data.applied_objs.len() > 1 {
            assert_eq!(event.data.applied_objs[1].data.get("foo").unwrap(), "bar");
        }
        for annotation in &event.annotations {
            assert_is_empty!(annotation);
        }
    }

    assert_eq!(*summary.annotations.get(&TEST_VALIDATOR_CODE).unwrap(), 1);
    assert_eq!(summary.patches, 1);
}
