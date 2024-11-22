use std::collections::VecDeque;
use std::ops::Index;

use sk_core::prelude::*;
use tracing::*;

use crate::{
    TraceAction,
    TraceEvent,
};

#[derive(Default)]
pub struct TraceEventList(VecDeque<TraceEvent>);

impl TraceEventList {
    pub(crate) fn append(&mut self, ts: i64, obj: &DynamicObject, action: TraceAction) {
        info!(
            "{:?} @ {ts}: {} {}",
            action,
            obj.types
                .clone()
                .map(|tm| format!("{}.{}", tm.api_version, tm.kind))
                .unwrap_or("<unknown type>".into()),
            obj.namespaced_name(),
        );

        let obj = obj.clone();
        match self.0.back_mut() {
            Some(evt) if evt.ts == ts => match action {
                TraceAction::ObjectApplied => evt.applied_objs.push(obj),
                TraceAction::ObjectDeleted => evt.deleted_objs.push(obj),
            },
            _ => {
                let evt = match action {
                    TraceAction::ObjectApplied => TraceEvent { ts, applied_objs: vec![obj], ..Default::default() },
                    TraceAction::ObjectDeleted => TraceEvent { ts, deleted_objs: vec![obj], ..Default::default() },
                };
                self.0.push_back(evt);
            },
        }
    }

    pub(crate) fn back(&self) -> Option<&TraceEvent> {
        self.0.back()
    }

    pub(crate) fn front(&self) -> Option<&TraceEvent> {
        self.0.front()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

impl Index<usize> for TraceEventList {
    type Output = TraceEvent;

    fn index(&self, i: usize) -> &Self::Output {
        &self.0[i]
    }
}

impl From<VecDeque<TraceEvent>> for TraceEventList {
    fn from(v: VecDeque<TraceEvent>) -> TraceEventList {
        TraceEventList(v)
    }
}

impl From<Vec<TraceEvent>> for TraceEventList {
    fn from(v: Vec<TraceEvent>) -> TraceEventList {
        TraceEventList(v.into())
    }
}

#[cfg(test)]
impl FromIterator<TraceEvent> for TraceEventList {
    fn from_iter<T: IntoIterator<Item = TraceEvent>>(ii: T) -> Self {
        TraceEventList(ii.into_iter().collect())
    }
}
