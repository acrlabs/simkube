use serde::{
    Deserialize,
    Serialize,
};

use crate::prelude::*;

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
