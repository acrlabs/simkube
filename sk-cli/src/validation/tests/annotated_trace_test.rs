use assertables::*;
use json_patch_ext::prelude::*;

use super::*;

#[rstest]
fn test_apply_patch_everywhere(mut annotated_trace: AnnotatedTrace) {
    annotated_trace
        .apply_patch(AnnotatedTracePatch {
            locations: PatchLocations::Everywhere,
            op: add_operation(format_ptr!("/foo"), "bar".into()),
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
            op: add_operation(format_ptr!("/foo"), "bar".into()),
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
