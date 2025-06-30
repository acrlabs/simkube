use assertables::*;
use json_patch_ext::prelude::*;
use serde_json::json;
use sk_store::TraceAction;

use super::*;
use crate::validation::annotated_trace::find_or_create_event_at_ts;
use crate::validation::AnnotatedTracePatch;


#[rstest]
fn test_apply_patch_everywhere(mut annotated_trace: AnnotatedTrace) {
    annotated_trace
        .apply_patch(AnnotatedTracePatch {
            locations: PatchLocations::Everywhere,
            ops: vec![add_operation(format_ptr!("/foo"), "bar".into())],
        })
        .unwrap();

    for event in annotated_trace.iter() {
        for obj in event.data.applied_objs.iter().chain(event.data.deleted_objs.iter()) {
            assert_eq!(obj.data.get("foo").unwrap(), "bar");
        }
    }
}

#[rstest]
fn test_apply_patch_object_reference(mut annotated_trace: AnnotatedTrace) {
    annotated_trace
        .apply_patch(AnnotatedTracePatch {
            locations: PatchLocations::ObjectReference(
                DEPL_GVK.into_type_meta(),
                format!("{TEST_NAMESPACE}/test_depl1"),
            ),
            ops: vec![add_operation(format_ptr!("/foo"), "bar".into())],
        })
        .unwrap();

    for event in annotated_trace.iter() {
        for obj in event.data.applied_objs.iter().chain(event.data.deleted_objs.iter()) {
            if obj.metadata.name == Some("test_depl1".into()) {
                assert_eq!(obj.data.get("foo").unwrap(), "bar");
            } else {
                assert_none!(obj.data.get("foo"));
            }
        }
    }
}

#[rstest]
#[case(TraceAction::ObjectApplied)]
#[case(TraceAction::ObjectDeleted)]
fn test_apply_patch_insert_at(mut annotated_trace: AnnotatedTrace, #[case] action: TraceAction) {
    annotated_trace
        .apply_patch(AnnotatedTracePatch {
            locations: PatchLocations::InsertAt(
                3,
                action,
                DS_GVK.into_type_meta(),
                Box::new(metav1::ObjectMeta {
                    namespace: Some(TEST_NAMESPACE.into()),
                    name: Some(TEST_DAEMONSET.into()),
                    ..Default::default()
                }),
            ),
            ops: vec![add_operation(format_ptr!("/spec"), json!({"minReadySeconds": 5}))],
        })
        .unwrap();

    let obj_vec = match action {
        TraceAction::ObjectApplied => &annotated_trace.events[3].data.applied_objs,
        TraceAction::ObjectDeleted => &annotated_trace.events[3].data.deleted_objs,
    };

    assert_eq!(obj_vec[0].metadata.name, Some(TEST_DAEMONSET.into()));
    assert_eq!(obj_vec[0].metadata.namespace, Some(TEST_NAMESPACE.into()));
}

#[rstest]
#[case(0, 0, 4)]
#[case(1, 1, 4)]
#[case(3, 3, 5)]
#[case(7, 4, 5)]
fn test_find_or_create_event_at_ts(
    mut annotated_trace: AnnotatedTrace,
    #[case] ts: i64,
    #[case] expected_idx: usize,
    #[case] expected_len: usize,
) {
    let event_idx = find_or_create_event_at_ts(&mut annotated_trace.events, ts);
    assert_eq!(expected_idx, event_idx);
    assert_len_eq_x!(annotated_trace.events, expected_len);
}

#[rstest]
fn test_find_or_create_event_at_ts_empty() {
    let event_idx = find_or_create_event_at_ts(&mut vec![], 5);
    assert_eq!(0, event_idx);
}
