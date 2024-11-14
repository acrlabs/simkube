pub mod constants;
pub mod errors;
pub mod external_storage;
pub mod hooks;
pub mod jsonutils;
pub mod k8s;
pub mod logging;
pub mod macros;
pub mod time;

pub mod prelude {
    pub use k8s_openapi::api::core::v1 as corev1;
    pub use k8s_openapi::apimachinery::pkg::apis::meta::v1 as metav1;
    pub use kube::{
        CustomResourceExt,
        ResourceExt,
    };
    pub use sk_api::v1::{
        Simulation,
        SimulationRoot,
    };

    pub use crate::constants::*;
    pub use crate::errors::EmptyResult;
    #[cfg(feature = "testutils")]
    pub use crate::k8s::testutils::*;
    pub use crate::k8s::KubeResourceExt;
}
