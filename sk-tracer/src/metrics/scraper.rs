use std::time::Duration;

use prometheus_parse::Scrape;
use rand::random_range;
use sk_core::prelude::*;
use tokio::time::sleep;
use tracing::*;

use super::*;

impl<'a> MetricScraper<'a> {
    pub(crate) fn new(
        http_client: reqwest::Client,
        target_url: String,
        metric_names: &'a [&'a str],
        scrape_interval_seconds: u64,
        token: String,
        metrics_tx: Sender,
    ) -> Self {
        Self {
            http_client,
            target_url,
            metric_names,
            scrape_interval_seconds,
            token,
            metrics_tx,
        }
    }

    pub(super) async fn start(&self) -> ! {
        info!("Starting metrics scraper loop for {}", self.target_url);

        let scrape_interval = Duration::from_secs(self.scrape_interval_seconds);
        let jitter_duration = Duration::from_secs(random_range(..self.scrape_interval_seconds));
        sleep(jitter_duration).await;

        loop {
            match self.scrape().await {
                Ok(()) => (),
                Err(e) => {
                    error!("could not scrape metrics: {e}");
                    error!("caused by: {:?}", e.source());
                },
            }
            sleep(scrape_interval).await;
        }
    }

    pub(super) async fn scrape(&self) -> EmptyResult {
        let body = self
            .http_client
            .get(&self.target_url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let metrics = Scrape::parse(body.lines().map(|l| Ok(l.into())))?;
        let tracked_metrics = metrics
            .samples
            .iter()
            .filter(|sample| self.metric_names.contains(&sample.metric.as_str()))
            .cloned()
            .collect();

        self.metrics_tx.send(tracked_metrics)?;

        Ok(())
    }
}
