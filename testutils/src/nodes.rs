use rstest::fixture;
use sk_core::prelude::*;

use crate::constants::*;

#[fixture]
pub fn test_node(#[default(TEST_NODE.into())] name: String) -> corev1::Node {
    corev1::Node {
        metadata: metav1::ObjectMeta { name: Some(name), ..Default::default() },
        spec: Some(corev1::NodeSpec { ..Default::default() }),
        status: Some(corev1::NodeStatus {
            addresses: Some(vec![
                corev1::NodeAddress {
                    address: TEST_NODE_IP_ADDR.into(),
                    type_: "InternalIP".into(),
                },
                corev1::NodeAddress {
                    address: "42.42.42.42".into(),
                    type_: "ExternalIP".into(),
                },
            ]),
            ..Default::default()
        }),
    }
}
