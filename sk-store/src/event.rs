use sk_core::event::{
    TraceAction,
    TraceEvent,
};
use sk_core::k8s::dyn_obj_type_str;
use sk_core::prelude::*;
use tracing::*;

pub(crate) fn append_event(event_list: &mut Vec<TraceEvent>, ts: i64, obj: &DynamicObject, action: TraceAction) {
    info!("{:?} @ {ts}: {} {}", action, dyn_obj_type_str(obj), obj.namespaced_name(),);

    let obj = obj.clone();
    match event_list.last_mut() {
        Some(evt) if evt.ts == ts => match action {
            TraceAction::ObjectApplied => evt.applied_objs.push(obj),
            TraceAction::ObjectDeleted => evt.deleted_objs.push(obj),
        },
        _ => {
            let evt = match action {
                TraceAction::ObjectApplied => TraceEvent { ts, applied_objs: vec![obj], ..Default::default() },
                TraceAction::ObjectDeleted => TraceEvent { ts, deleted_objs: vec![obj], ..Default::default() },
            };
            event_list.push(evt);
        },
    }
}
