---
project: SimKube
template: docs.html
---

# Making API Changes

## SimKube Custom Resource Definition changes

The Simulation CRD is auto-generated from the Golang struct in `./lib/go/api/v1/simulation_types.go` using the
[controller-gen](https://book.kubebuilder.io/reference/controller-gen.html) utility.  The resulting CRDs are stored in
`./k8s/raw/`, and then Rust structs are generated from the resulting CRD using
[kopium](https://github.com/kube-rs/kopium).  This _should_ all be done automagically by running `make crd`, but kopium
is listed as unstable, so check the diff output carefully.

## SimKube API changes

The SimKube API (used by `sk-tracer` and `skctl`, and possibly others in the future) is generated from an OpenAPI v3
specification in `./api/v1/simkube.yml`.  I haven't _yet_ figured out how to wire that up to [Rocket](https://rocket.rs)
(the Rust library we're using for handling HTTP requests), so for right now we're just using the model definition output
from this file.  This process is currently quite manual.  The steps look something like the following:

1. `make api`
2. In `lib/go/api/v1/*.go`, replace all the k8s-generated types with the correct imports from the Kubernetes API (make
   sure to do this not just in the model files but also in `utils.go`.
3. In `lib/go/api/v1/*.go`, make sure to update the package name from `openapi` to `v1`.
4. In `lib/go/api/v1/*.go`, annotate the generated objects with `//+kubebuilder:object:generate=false` to keep
   `controller-gen` from trying to interpret these files as custom resource types.
5. In `lib/rust/api/v1/*.rs`, add `use super::*` to the top of each generated file
6. In `lib/rust/api/v1/*.rs`, replace all the k8s-generated types with the correct imports from `k8s-openapi`.

Once you've made all of these changes, you will need to check the diff quite carefully to ensure that nothing is broken.

### Example modifications for generated Golang code

This generated output:

```go
package openapi

import (
    "encoding/json"
)

// ExportFilters struct for ExportFilters
type ExportFilters struct {
    ExcludedNamespaces []string                       `json:"excludedNamespaces"`
    ExcludedLabels []ExportFiltersExcludedLabelsInner `json:"excludedLabels"`
    ExcludeDaemonsets bool                            `json:"excludeDaemonsets"`
}

...
```

should be transformed into:

```go
package v1

import (
    "encoding/json"

    metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

//+kubebuilder:object:generate=false

// ExportFilters struct for ExportFilters
type ExportFilters struct {
    ExcludedNamespaces []string           `json:"excludedNamespaces"`
    ExcludedLabels []metav1.LabelSelector `json:"excludedLabels"`
    ExcludeDaemonsets bool                `json:"excludeDaemonsets"`
}
```

### Example modifications for generated Rust code

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
