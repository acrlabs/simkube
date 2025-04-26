mod missing_resources_test;
mod status_field_populated_test;
mod validate_all_test;

use std::collections::HashMap;

use rstest::*;
use sk_core::k8s::GVK;
use sk_core::prelude::*;
use sk_store::{
    TracerConfig,
    TrackedObjectConfig,
};
use sk_testutils::*;

use super::*;
use crate::validation::validator::Diagnostic;
use crate::validation::AnnotatedTraceEvent;

#[fixture]
fn test_trace_config() -> TracerConfig {
    TracerConfig {
        tracked_objects: HashMap::from([
            (
                DEPL_GVK.clone(),
                TrackedObjectConfig {
                    pod_spec_template_paths: Some(vec!["/spec/template".into()]),
                    ..Default::default()
                },
            ),
            (SVC_ACCOUNT_GVK.clone(), Default::default()),
        ]),
    }
}

#[fixture]
fn test_trace_config_two_pods() -> TracerConfig {
    TracerConfig {
        tracked_objects: HashMap::from([
            (
                GVK::new("fake", "v1", "TwoPods"),
                TrackedObjectConfig {
                    pod_spec_template_paths: Some(vec!["/spec/template1".into(), "/spec/template2".into()]),
                    ..Default::default()
                },
            ),
            (SVC_ACCOUNT_GVK.clone(), Default::default()),
        ]),
    }
}
