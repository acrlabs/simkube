use std::sync::Arc;

use bytes::Bytes;
use object_store::ObjectStoreScheme;
use sk_api::v1::ExportRequest;
use sk_core::external_storage::ObjectStoreWrapper;
use tokio::sync::Mutex;

use crate::store::TraceStore;

pub async fn export_helper(
    req: &ExportRequest,
    store: Arc<Mutex<TraceStore>>,
    object_store: &(dyn ObjectStoreWrapper + Sync),
) -> anyhow::Result<Vec<u8>> {
    let trace_data = { store.lock().await.export(req.start_ts, req.end_ts, &req.filters).await? };

    match object_store.scheme() {
        // If we're writing to a cloud provider, we want to write from the location that the
        // tracer's running from, ostensibly to minimize transport costs.
        ObjectStoreScheme::AmazonS3 | ObjectStoreScheme::GoogleCloudStorage | ObjectStoreScheme::MicrosoftAzure => {
            object_store.put(Bytes::from(trace_data)).await?;
            Ok(vec![])
        },

        // On the other hand, if we're trying to write to local storage (or something else), it's
        // not going to do any good to write to local storage of the _tracer_, so we return all the
        // data and let the client do something with it.
        _ => Ok(trace_data),
    }
}
