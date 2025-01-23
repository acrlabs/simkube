<!--
template: docs.html
-->

# Making API Changes

## SimKube Custom Resource Definition changes

The Simulation CRD is auto-generated from the Rust structs in `./lib/api/v1/(simulations|simulation_roots).rs`
If these structs change, you will need to regenerate the CRD yaml by running `make crd`; the resulting CRDs are stored
in `./k8s/raw/`.  A pre-commit check as well as a GitHub action will complain if you have not updated the CRD yaml
before committing.

## SimKube API changes

The SimKube API (used by `sk-tracer` and `skctl`, and possibly others in the future) is generated from an OpenAPI v3
specification in `./api/v1/simkube.yml`.  I haven't _yet_ figured out how to wire that up to [Rocket](https://rocket.rs)
(the Rust library we're using for handling HTTP requests), so for right now we're just using the model definition output
from this file.  This process is currently quite manual.  The steps look something like the following:

1. `make api`
2. In `lib/api/v1/*.rs`, add `use super::*` to the top of each generated file
3. In `lib/api/v1/*.rs`, replace all the k8s-generated types with the correct imports from `k8s-openapi`.

Once you've made all of these changes, you will need to check the diff quite carefully to ensure that nothing is broken.

### Example modifications for generated code

This generated output:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportFilters {
    #[serde(rename = "excluded_namespaces")]
    pub excluded_namespaces: Vec<String>,
    #[serde(rename = "excluded_labels")]
    pub excluded_labels: Vec<crate::models::ExportFiltersExcludedLabelsInner>,
    #[serde(rename = "exclude_daemonsets")]
    pub exclude_daemonsets: bool,
}

impl ExportFilters {
    pub fn new(excluded_namespaces: Vec<String>, excluded_labels: Vec<crate::models::ExportFiltersExcludedLabelsInner>, exclude_daemonsets: bool) -> ExportFilters {
        ExportFilters {
            excluded_namespaces,
            excluded_labels,
            exclude_daemonsets,
        }
    }
}
```

should be transformed into:

```rust
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportFilters {
    #[serde(rename = "excluded_namespaces")]
    pub excluded_namespaces: Vec<String>,
    #[serde(rename = "excluded_labels")]
    pub excluded_labels: Vec<metav1::LabelSelector>,
    #[serde(rename = "exclude_daemonsets")]
    pub exclude_daemonsets: bool,
}

impl ExportFilters {
    pub fn new(
        excluded_namespaces: Vec<String>,
        excluded_labels: Vec<metav1::LabelSelector>,
        exclude_daemonsets: bool,
    ) -> ExportFilters {
        ExportFilters {
            excluded_namespaces,
            excluded_labels,
            exclude_daemonsets,
        }
    }
}
```
