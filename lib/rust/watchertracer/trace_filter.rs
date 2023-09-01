use chrono::Utc;
use k8s_openapi::api::core::v1 as corev1;
use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
use serde::Deserialize;

use crate::util::pod_matches_selector;
use crate::watchertracer::TraceEvent;

#[derive(Deserialize, Debug)]
pub struct ExportFilter {
    #[serde(default = "default_start_time")]
    pub start_time: i64,

    #[serde(default = "default_end_time")]
    pub end_time: i64,

    #[serde(default = "default_excluded_namespaces")]
    pub excluded_namespaces: Vec<String>,

    #[serde(default)]
    pub excluded_labels: Vec<metav1::LabelSelector>,

    #[serde(default = "default_exclude_daemonsets")]
    pub exclude_daemonsets: bool,
}

pub fn filter_event(evt: &TraceEvent, f: &ExportFilter) -> Option<TraceEvent> {
    // Inclusive start time, exclusive end time, inequalities are reversed
    // because we're checking if the event falls _outside_ the window
    if evt.ts < f.start_time || evt.ts >= f.end_time {
        return None;
    }

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

fn pod_matches_filter(pod: &corev1::Pod, f: &ExportFilter) -> bool {
    pod.metadata.namespace.as_ref().is_some_and(|ns| f.excluded_namespaces.contains(ns))
        || pod
            .metadata
            .owner_references
            .as_ref()
            .is_some_and(|owners| owners.iter().any(|owner| &owner.kind == "DaemonSet"))
        || f.excluded_labels.iter().any(|sel| pod_matches_selector(pod, sel).unwrap())
}

fn default_start_time() -> i64 {
    Utc::now().timestamp() - 15 * 60
}

fn default_end_time() -> i64 {
    Utc::now().timestamp()
}

fn default_excluded_namespaces() -> Vec<String> {
    vec!["kube-system".into()]
}

fn default_exclude_daemonsets() -> bool {
    true
}
