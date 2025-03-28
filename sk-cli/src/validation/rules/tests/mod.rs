mod missing_resources_test;
mod status_field_populated_test;
mod validate_all_test;

use std::collections::HashMap;

use rstest::*;
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
                    pod_spec_template_path: Some("/spec/template".into()),
                    ..Default::default()
                },
            ),
            (SVC_ACCOUNT_GVK.clone(), Default::default()),
        ]),
    }
}
