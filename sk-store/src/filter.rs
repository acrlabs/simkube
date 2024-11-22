use sk_api::v1::ExportFilters;
use sk_core::prelude::*;

use super::TraceEvent;

pub fn filter_event(evt: &TraceEvent, f: &ExportFilters) -> Option<TraceEvent> {
    let new_evt = TraceEvent {
        ts: evt.ts,
        applied_objs: evt
            .applied_objs
            .iter()
            .filter(|obj| !obj_matches_filter(obj, f))
            .cloned()
            .collect(),
        deleted_objs: evt
            .deleted_objs
            .iter()
            .filter(|obj| !obj_matches_filter(obj, f))
            .cloned()
            .collect(),
    };

    if new_evt.applied_objs.is_empty() && new_evt.deleted_objs.is_empty() {
        return None;
    }

    Some(new_evt)
}

fn obj_matches_filter(obj: &DynamicObject, f: &ExportFilters) -> bool {
    obj.metadata
        .namespace
        .as_ref()
        .is_some_and(|ns| f.excluded_namespaces.contains(ns))
        || obj
            .metadata
            .owner_references
            .as_ref()
            .is_some_and(|owners| owners.iter().any(|owner| &owner.kind == "DaemonSet"))
        // TODO: maybe don't call unwrap here?  Right now we panic if the user specifies
        // an invalid label selector.  Or, maybe it doesn't matter once we write the CLI
        // tool.
        || f.excluded_labels.iter().any(|sel| obj.matches(sel).unwrap())
}
