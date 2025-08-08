use serde::{
    Deserialize,
    Serialize,
};
use sk_core::k8s::dyn_obj_type_str;
use sk_core::prelude::*;
use tracing::*;


#[derive(Clone, Copy, Debug)]
pub enum TraceAction {
    ObjectApplied,
    ObjectDeleted,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TraceEvent {
    pub ts: i64,
    pub applied_objs: Vec<DynamicObject>,
    pub deleted_objs: Vec<DynamicObject>,
}

impl TraceEvent {
    pub fn len(&self) -> usize {
        self.applied_objs.len() + self.deleted_objs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.applied_objs.is_empty() && self.deleted_objs.is_empty()
    }
}

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
