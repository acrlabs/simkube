use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use serde::Deserialize;

use crate::util::pod_matches_selector;
use crate::watchertracer::TraceEvent;

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
        created_pods: evt.created_pods.iter().filter(|pod| !pod_matches_filter(pod, f)).cloned().collect(),
        deleted_pods: evt.deleted_pods.iter().filter(|pod| !pod_matches_filter(pod, f)).cloned().collect(),
    };

    if new_evt.created_pods.is_empty() && new_evt.deleted_pods.is_empty() {
        return None;
    }

    Some(new_evt)
}

fn pod_matches_filter(pod: &corev1::Pod, f: &TraceFilter) -> bool {
    pod.metadata.namespace.as_ref().is_some_and(|ns| f.excluded_namespaces.contains(ns))
        || pod
            .metadata
            .owner_references
            .as_ref()
            .is_some_and(|owners| owners.iter().any(|owner| &owner.kind == "DaemonSet"))
        || f.excluded_labels.iter().any(|sel| pod_matches_selector(pod, sel).unwrap())
}
