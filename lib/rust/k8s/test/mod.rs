mod pod_lifecycle_test;
mod util_test;

use rstest::*;

use super::macros::*;
use super::*;

#[fixture]
fn pod() -> corev1::Pod {
    corev1::Pod {
        metadata: metav1::ObjectMeta {
            labels: klabel!("foo" = "bar"),
            ..Default::default()
        },
        spec: Some(corev1::PodSpec { ..Default::default() }),
        status: Some(corev1::PodStatus { ..Default::default() }),
    }
}
