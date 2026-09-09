use k8s_openapi::api::core::v1 as corev1;

pub fn internal_node_ip(node: &corev1::Node) -> Option<String> {
    // N.B.: If there are multiple internal ip addresses for the node, this will
    // only pick the first one.  I don't know if/how frequently that happens but
    // we can figure that out when if it becomes a problem
    node.status
        .as_ref()?
        .addresses
        .as_ref()?
        .iter()
        .filter(|address| address.type_ == "InternalIP")
        .cloned()
        .map(|address| address.address)
        .next()
}

#[cfg(test)]
mod tests {
    use assertables::*;
    use rstest::rstest;
    use sk_testutils::*;

    use super::*;

    #[rstest]
    fn test_internal_node_ip(test_node: corev1::Node) {
        assert_some_eq_x!(internal_node_ip(&test_node), TEST_NODE_IP_ADDR);
    }

    #[rstest]
    fn test_internal_node_ip_no_internal_ip(mut test_node: corev1::Node) {
        test_node.status.as_mut().unwrap().addresses = Some(vec![corev1::NodeAddress {
            address: "42.42.42.42".into(),
            type_: "ExternalIP".into(),
        }]);
        assert_none!(internal_node_ip(&test_node));
    }
}
