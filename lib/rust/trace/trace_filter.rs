use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use kube::api::DynamicObject;
use serde::Deserialize;

use super::TraceEvent;
use crate::k8s::obj_matches_selector;

#[derive(Deserialize, Debug, Clone)]
pub struct TraceFilter {
    pub excluded_namespaces: Vec<String>,
    pub excluded_labels: Vec<metav1::LabelSelector>,
    pub exclude_daemonsets: bool,
}

impl TraceFilter {
    pub fn blank() -> TraceFilter {
        TraceFilter {
            excluded_namespaces: vec![],
            excluded_labels: vec![],
            exclude_daemonsets: false,
        }
    }
}

pub fn filter_event(evt: &TraceEvent, f: &TraceFilter) -> Option<TraceEvent> {
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

fn obj_matches_filter(obj: &DynamicObject, f: &TraceFilter) -> bool {
    obj.metadata
        .namespace
        .as_ref()
        .is_some_and(|ns| f.excluded_namespaces.contains(ns))
        || obj
            .metadata
            .owner_references
            .as_ref()
            .is_some_and(|owners| owners.iter().any(|owner| &owner.kind == "DaemonSet"))
        || f.excluded_labels.iter().any(|sel| obj_matches_selector(obj, sel).unwrap())
}
