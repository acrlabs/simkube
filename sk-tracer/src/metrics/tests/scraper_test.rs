use httpmock::prelude::*;
use prometheus_parse::Scrape;

use super::*;

const FAKE_METRICS_ENDPOINT: &str = "/fake-metrics";

#[rstest(tokio::test)]
async fn test_scrape() {
    rustls::crypto::ring::default_provider().install_default().unwrap();

    let sample_strings = [
        "http_requests_total{method=\"post\",code=\"200\"} 42 1395066363000".to_string(),
        "some_metric{label=\"baz\"} 42 1395066393000".to_string(),
        "some_other_metric{label=\"baz\"} 42 1395066463000".to_string(),
        "http_requests_total{method=\"post\",code=\"404\"} 4242 1395066363000".to_string(),
    ];
    let expected_samples = Scrape::parse(
        sample_strings
            .iter()
            .filter(|s| s.starts_with("http_requests_total"))
            .cloned()
            .map(Ok),
    )
    .unwrap()
    .samples
    .clone();

    let server = MockServer::start();
    server.mock(|when, then| {
        when.path(FAKE_METRICS_ENDPOINT);
        then.body(sample_strings.join("\n"));
    });
    let (metrics_tx, mut metrics_rx): (Sender, Receiver) = mpsc::unbounded_channel();
    let url = server.url(FAKE_METRICS_ENDPOINT);

    let http_client = reqwest::Client::new();
    let scraper = MetricScraper::new(http_client, url, &["http_requests_total"], 42, "fake-token".into(), metrics_tx);
    scraper.scrape().await.unwrap();
    let metrics = metrics_rx.recv().await.unwrap();
    assert_eq!(metrics, expected_samples);
}
