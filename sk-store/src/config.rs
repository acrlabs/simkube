use std::collections::HashMap;
use std::fs::File;
use std::ops::Not;

use serde::{
    Deserialize,
    Serialize,
};
use sk_core::k8s::GVK;
use tracing::*;

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
}
