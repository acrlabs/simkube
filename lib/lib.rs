pub mod api;
mod config;
mod constants;
pub mod errors;
pub mod jsonutils;
pub mod k8s;
pub mod logging;
pub mod macros;
pub mod metrics;
pub mod sim;
pub mod store;
pub mod time;
pub mod util;
pub mod watch;

pub mod prelude {
    pub use k8s_openapi::api::core::v1 as corev1;
    pub use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
    pub use tracing::*;

    pub use crate::api::v1::{
        ExportFilters,
        ExportRequest,
        Simulation,
        SimulationMetricsConfig,
        SimulationRoot,
        SimulationRootSpec,
        SimulationSpec,
        SimulationState,
        SimulationStatus,
    };
    pub use crate::config::*;
    pub use crate::constants::*;
    pub use crate::errors::EmptyResult;
    pub use crate::k8s::{
        KubeResourceExt,
        PodExt,
        PodLifecycleData,
    };
    pub use crate::logging;
    pub use crate::time::{
        Clockable,
        UtcClock,
    };
}

#[cfg(feature = "testutils")]
pub mod testutils;
