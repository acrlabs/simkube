use std::collections::HashMap;
use std::fs::File;
use std::ops::Not;

use serde::{
    Deserialize,
    Serialize,
};
use sk_core::constants::GVK_POD_SPEC_TEMPLATE_PATHS;
use sk_core::k8s::GVK;
use tracing::*;

use crate::errors::ConfigError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackedObjectConfigWithDeprecatedFields {
    #[deprecated]
    pub pod_spec_template_path: Option<String>,
    pub pod_spec_template_paths: Option<Vec<String>>,

    #[serde(default)]
    pub track_lifecycle: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", from = "TrackedObjectConfigWithDeprecatedFields")]
pub struct TrackedObjectConfig {
    pub pod_spec_template_paths: Option<Vec<String>>,

    #[serde(skip_serializing_if = "<&bool>::not")]
    pub track_lifecycle: bool,
}

impl From<TrackedObjectConfigWithDeprecatedFields> for TrackedObjectConfig {
    fn from(input: TrackedObjectConfigWithDeprecatedFields) -> Self {
        let mut output = TrackedObjectConfig {
            pod_spec_template_paths: input.pod_spec_template_paths.clone(),
            track_lifecycle: input.track_lifecycle,
        };

        #[allow(deprecated)]
        if let Some(pstp) = input.pod_spec_template_path {
            warn!(
                "tracked object config field podSpecTemplatePath is deprecated \
                    and will be removed in a future version of SimKube.  Please use \
                    podSpecTemplatePaths instead."
            );

            if input.pod_spec_template_paths.as_ref().is_some_and(|p| !p.is_empty()) {
                warn!(
                    "both podSpecTemplatePath and podSpecTemplatePaths are set; \
                        ignoring the deprecated field."
                );
            } else {
                output.pod_spec_template_paths = Some(vec![pstp]);
            }
        }

        output
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracerConfig {
    pub tracked_objects: HashMap<GVK, TrackedObjectConfig>,
}

impl TracerConfig {
    pub fn normalize(mut self) -> Result<Self, ConfigError> {
        let mut normalized_objects = HashMap::new();
        for (gvk, mut obj) in self.tracked_objects {
            let default = GVK_POD_SPEC_TEMPLATE_PATHS.get(&gvk).cloned();
            let resolved_paths = match obj.pod_spec_template_paths {
                // User provided paths
                Some(paths) if !paths.is_empty() => {
                    if let Some(default) = &default
                        && paths != *default
                    {
                        return Err(ConfigError::InvalidPath(gvk.clone()));
                    }
                    paths
                },
                // No user paths, use default if available
                _ => {
                    if let Some(default) = &default {
                        if default.is_empty() {
                            return Err(ConfigError::MissingPath(gvk.clone()));
                        }
                        default.iter().map(|s| s.to_string()).collect()
                    } else {
                        return Err(ConfigError::MissingPath(gvk.clone()));
                    }
                },
            };
            obj.pod_spec_template_paths = Some(resolved_paths);
            normalized_objects.insert(gvk, obj);
        }
        self.tracked_objects = normalized_objects;
        Ok(self)
    }

    pub fn load(filename: &str) -> anyhow::Result<TracerConfig> {
        Ok(serde_yaml::from_reader(File::open(filename)?)?)
    }

    pub fn pod_spec_template_paths(&self, gvk: &GVK) -> Option<&[String]> {
        self.tracked_objects.get(gvk)?.pod_spec_template_paths.as_deref()
    }

    pub fn track_lifecycle_for(&self, gvk: &GVK) -> bool {
        self.tracked_objects.get(gvk).is_some_and(|obj| obj.track_lifecycle)
    }
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use sk_testutils::*;

    use super::*;

    #[rstest]
    #[case::none(None, vec!["/foo/bar".into()])]
    #[case::empty(Some(vec![]), vec!["/foo/bar".into()])]
    #[case::full(Some(vec!["/asdf".into()]), vec!["/asdf".into()])]
    fn test_deprecated_config(#[case] pod_spec_template_paths: Option<Vec<String>>, #[case] expected: Vec<String>) {
        let gvk = GVK::new("fake", "v1", "Resource");
        let mut config_yml = "
---
trackedObjects:
  fake/v1.Resource:
    podSpecTemplatePath: /foo/bar
"
        .to_string();

        if let Some(pstps) = pod_spec_template_paths
            && pstps.len() > 0
        {
            let pstp = pstps[0].clone();
            config_yml.push_str(&format!("    podSpecTemplatePaths:\n      - {pstp}"));
        }

        let config: TracerConfig = serde_yaml::from_str(&config_yml).unwrap();

        assert_eq!(config.tracked_objects[&gvk].pod_spec_template_paths, Some(expected));
    }

    #[rstest]
    fn test_correct_config() {
        let gvk = GVK::new("fake", "v1", "Resource");
        let config_yml = "
---
trackedObjects:
  fake/v1.Resource:
    podSpecTemplatePaths:
      - /foo/bar
"
        .to_string();

        let config: TracerConfig = serde_yaml::from_str(&config_yml).unwrap();
        assert_eq!(config.tracked_objects[&gvk].pod_spec_template_paths, Some(vec!["/foo/bar".into()]));
    }

    enum Expected {
        Ok(Vec<&'static str>),
        InvalidPath,
        MissingPath,
    }

    fn config_with(gvk: &GVK, paths: Option<Vec<&str>>) -> TracerConfig {
        let mut map = HashMap::new();
        map.insert(
            gvk.clone(),
            TrackedObjectConfig {
                pod_spec_template_paths: paths.map(|pstps| pstps.into_iter().map(|pstp| pstp.to_string()).collect()),
                ..Default::default()
            },
        );

        TracerConfig { tracked_objects: map }
    }

    #[rstest]
    #[case::known_gvk_with_valid_paths(("batch","v1","CronJob"), Some(vec!["/spec/jobTemplate/spec/template"]), Expected::Ok(vec!["/spec/jobTemplate/spec/template"]))]
    #[case::known_gvk_with_invalid_paths(("batch","v1","CronJob"), Some(vec!["/invalid/path"]), Expected::InvalidPath)]
    #[case::known_gvk_with_empty_paths(("apps","v1","DaemonSet"), Some(vec![]), Expected::Ok(vec!["/spec/template"]))]
    #[case::unknown_gvk_with_paths(("fake","v1","Resource"), Some(vec!["/foo/bar"]), Expected::Ok(vec!["/foo/bar"]))]
    #[case::unknown_gvk_with_empty_paths(("fake","v1","Resource"), Some(vec![]), Expected::MissingPath)]
    #[case::unknown_gvk_with_none_paths(("fake","v1","Resource"), None, Expected::MissingPath)]
    // Test all supported defaults in sk-core::constants::GVK_POD_SPEC_TEMPLATE_PATHS
    #[case::cronjob_none_paths(("batch","v1","CronJob"), None, Expected::Ok(vec!["/spec/jobTemplate/spec/template"]))]
    #[case::daemonset_none_paths(("apps","v1","DaemonSet"), None, Expected::Ok(vec!["/spec/template"]))]
    #[case::deployment_none_paths(("apps","v1","Deployment"), None, Expected::Ok(vec!["/spec/template"]))]
    #[case::job_none_paths(("batch","v1","Job"), None, Expected::Ok(vec!["/spec/template"]))]
    #[case::replicaset_none_paths(("apps","v1","ReplicaSet"), None, Expected::Ok(vec!["/spec"]))]
    #[case::statefulset_none_paths(("apps","v1","StatefulSet"), None, Expected::Ok(vec!["/spec/template"]))]

    fn test_normalize(
        #[case] input_gvk: (&str, &str, &str),
        #[case] input_paths: Option<Vec<&str>>,
        #[case] expected: Expected,
    ) {
        let gvk = GVK::new(input_gvk.0, input_gvk.1, input_gvk.2);
        let config = config_with(&gvk, input_paths);
        let result = config.normalize();

        match expected {
            Expected::InvalidPath => {
                assert_matches!(result, Err(ConfigError::InvalidPath(..)))
            },
            Expected::MissingPath => {
                assert_matches!(result, Err(ConfigError::MissingPath(..)))
            },
            Expected::Ok(expected_paths) => {
                let validated = result.expect("expected success");
                let actual = validated.tracked_objects[&gvk].pod_spec_template_paths.as_ref().unwrap();

                let expected: Vec<String> = expected_paths.into_iter().map(|s| s.to_string()).collect();
                assert_eq!(actual, &expected)
            },
        }
    }
}
