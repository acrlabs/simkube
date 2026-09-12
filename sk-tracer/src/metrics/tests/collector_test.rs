use futures::{
    StreamExt,
    stream,
};
use kube::runtime::watcher::Event;

use super::*;

#[fixture]
fn metrics_collector() -> Collector {
    rustls::crypto::ring::default_provider().install_default().unwrap();

    let (ready_tx, _): (mpsc::Sender<bool>, mpsc::Receiver<bool>) = mpsc::channel(1);
    let (metrics_tx, _): (Sender, Receiver) = mpsc::unbounded_channel();
    let node_stream = stream::pending().boxed();

    Collector {
        http_client: reqwest::Client::new(),
        metrics_tx,
        node_scrapers: HashMap::new(),
        node_stream,
        scrape_interval_seconds: 42,

        service_account_token: "fake-service-account-token".into(),

        init_node_buffer: HashMap::new(),
        is_ready: false,
        ready_tx,
    }
}

#[rstest(tokio::test)]
async fn test_collector_handle_apply_then_delete(mut metrics_collector: Collector, test_node: corev1::Node) {
    metrics_collector
        .handle_node_event(&Event::Apply(test_node.clone()))
        .await
        .unwrap();
    assert_len_eq_x!(&metrics_collector.node_scrapers, 1);

    metrics_collector.handle_node_event(&Event::Delete(test_node)).await.unwrap();
    assert_len_eq_x!(&metrics_collector.node_scrapers, 0);
}

#[rstest(tokio::test)]
async fn test_collector_handle_delete_empty(mut metrics_collector: Collector, test_node: corev1::Node) {
    metrics_collector.handle_node_event(&Event::Delete(test_node)).await.unwrap();
    assert_len_eq_x!(&metrics_collector.node_scrapers, 0);
}

#[rstest(tokio::test)]
async fn test_collector_handle_init(mut metrics_collector: Collector) {
    let node1 = test_node("test_node1".into());
    let node2 = test_node("test_node2".into());
    let node3 = test_node("test_node3".into());
    metrics_collector.handle_node_event(&Event::Apply(node1.clone())).await.unwrap();
    metrics_collector.handle_node_event(&Event::Apply(node2)).await.unwrap();

    metrics_collector.handle_node_event(&Event::Init).await.unwrap();
    metrics_collector.handle_node_event(&Event::InitApply(node1)).await.unwrap();
    metrics_collector.handle_node_event(&Event::InitApply(node3)).await.unwrap();
    metrics_collector.handle_node_event(&Event::InitDone).await.unwrap();

    assert_bag_eq!(
        &metrics_collector.node_scrapers.keys().collect::<Vec<_>>(),
        &[&"test_node1".into(), &"test_node3".into()]
    );
    assert_is_empty!(&metrics_collector.init_node_buffer);
}
