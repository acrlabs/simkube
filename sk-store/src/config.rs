use std::collections::HashMap;
use std::fs::File;
use std::ops::Not;

use serde::{
    Deserialize,
    Serialize,
};
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
        self.tracked_objects = normalize_pod_spec_template_paths(self.tracked_objects)?;
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

fn normalize_pod_spec_template_paths(
    objects: HashMap<GVK, TrackedObjectConfig>,
) -> Result<HashMap<GVK, TrackedObjectConfig>, ConfigError> {
    let mut normalized_objects = HashMap::new();
    for (gvk, mut obj) in objects {
        let default = default_path(&gvk);

        let normalized_paths = normalize_paths_for_gvk(&gvk, default, obj.pod_spec_template_paths)?;
        obj.pod_spec_template_paths = normalized_paths;
        normalized_objects.insert(gvk, obj);
    }
    Ok(normalized_objects)
}

fn normalize_paths_for_gvk(
    gvk: &GVK,
    default: Option<&'static str>,
    paths: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, ConfigError> {
    match (default, paths) {
        // Known GVK, no user provided path, use default path
        (Some(default_path), None) => Ok(Some(vec![default_path.to_string()])),
        // Known GVK, empty vec, use default path
        (Some(default_path), Some(p)) if p.is_empty() => Ok(Some(vec![default_path.to_string()])),
        // Known GVK, user provided paths, must match default
        (Some(default_path), Some(p)) => {
            let mut normalized = Vec::new();

            for path in p {
                if path != default_path {
                    return Err(ConfigError::InvalidPath {
                        gvk: gvk.clone(),
                        path: path.clone(),
                        expected: default_path.to_string(),
                    });
                }
                normalized.push(default_path.to_string());
            }
            Ok(Some(normalized))
        },
        // Unknown GVK, no user provided paths, return error
        (None, None) => Err(ConfigError::MissingPath { gvk: gvk.clone() }),
        // handle empty vec
        (None, Some(p)) if p.is_empty() => Err(ConfigError::MissingPath { gvk: gvk.clone() }),
        // Unknown GVK, user provided paths, accept as is
        (None, Some(p)) => Ok(Some(p)),
    }
}

fn default_path(gvk: &GVK) -> Option<&'static str> {
    match (gvk.group.as_str(), gvk.version.as_str(), gvk.kind.as_str()) {
        // List of supported GVKs where PodTemplatePaths are not required to be user provided
        ("batch", "v1", "CronJob") => Some("/spec/jobTemplate/spec/template"),
        ("apps", "v1", "DaemonSet") => Some("/spec/template"),
        ("apps", "v1", "Deployment") => Some("/spec/template"),
        ("batch", "v1", "Job") => Some("/spec/template"),
        ("apps", "v1", "ReplicaSet") => Some("/spec"),
        ("apps", "v1", "StatefulSet") => Some("/spec/template"),
        _ => None,
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

    #[rstest]
    #[case::cronjob(vec!["batch","v1","CronJob"], Some("/spec/jobTemplate/spec/template"))]
    #[case::daemonset(vec!["apps","v1","DaemonSet"], Some("/spec/template"))]
    #[case::deployment(vec!["apps","v1","Deployment"], Some("/spec/template"))]
    #[case::job(vec!["batch","v1","Job"], Some("/spec/template"))]
    #[case::replicaset(vec!["apps","v1","ReplicaSet"], Some("/spec"))]
    #[case::statefulset(vec!["apps","v1","StatefulSet"], Some("/spec/template"))]
    #[case::random(vec!["fake","v1","Resource"], None)]

    fn test_default_path(#[case] input: Vec<&str>, #[case] expected: Option<&str>) {
        let new_gvk = GVK::new(input[0], input[1], input[2]);
        let default = default_path(&new_gvk);
        assert_eq!(default, expected)
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
    #[case::known_gvk_with_valid_path(("batch","v1","CronJob"), Some(vec!["/spec/JobTemplate/spec/template"]), Expected::Ok(vec!["/spec/jobTemplate/spec/template"]))]
    #[case::known_gvk_with_invalid_path(("batch","v1","CronJob"), Some(vec!["/invalid/path"]), Expected::InvalidPath)]
    #[case::known_gvk_with_no_path(("apps","v1","DaemonSet"), Some(vec![]), Expected::Ok(vec!["/spec/template"]))]
    #[case::unknown_gvk_with_path(("fake","v1","Resource"), Some(vec!["/foo/bar"]), Expected::Ok(vec!["/foo/bar"]))]
    #[case::unknown_gvk_with_no_path(("fake","v1","Resource"), Some(vec![]), Expected::MissingPath)]

    fn test_validate(
        #[case] input_gvk: (&str, &str, &str),
        #[case] input_paths: Option<Vec<&str>>,
        #[case] expected: Expected,
    ) {
        let gvk = GVK::new(input_gvk.0, input_gvk.1, input_gvk.2);
        let config = config_with(&gvk, input_paths);
        let result = config.normalize();

        match expected {
            Expected::InvalidPath => {
                assert_matches!(result, Err(ConfigError::InvalidPath { .. }), "expected InvalidPath, got {:?}", result)
            },
            Expected::MissingPath => {
                assert_matches!(result, Err(ConfigError::MissingPath { .. }), "expected MissingPath, got {:?}", result)
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
