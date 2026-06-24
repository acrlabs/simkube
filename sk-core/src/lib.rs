#![cfg_attr(coverage, feature(coverage_attribute))]
pub mod config;
pub mod constants;
pub mod errors;
pub mod event;
pub mod events;
pub mod external_storage;
pub mod hooks;
pub mod index;
pub mod jsonutils;
pub mod k8s;
pub mod logging;
pub mod macros;
pub mod pod_owners_map;
pub mod time;
pub mod trace;

pub mod prelude {
    pub use k8s_openapi::api::core::v1 as corev1;
    pub use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
    pub use kube::api::{
        DynamicObject,
        TypeMeta,
    };
    pub use kube::{
        CustomResourceExt,
        ResourceExt,
    };
    pub use sk_api::v1::{
        Simulation,
        SimulationRoot,
    };

    pub use crate::config::{
        TracerConfig,
        TrackedObjectConfig,
    };
    pub use crate::constants::*;
    pub use crate::errors::EmptyResult;
    pub use crate::event::{
        TraceAction,
        TraceEvent,
        append_event,
    };
    pub use crate::events::SkEventRecorder;
    pub use crate::k8s::{
        KubeResourceExt,
        OpenApiResourceExt,
    };
    pub use crate::pod_owners_map::{
        PodLifecyclesMap,
        PodOwnersMap,
    };
    pub use crate::trace::ExportedTrace;
}
