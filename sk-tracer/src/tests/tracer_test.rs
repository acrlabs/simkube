use object_store::ObjectStoreScheme;
use sk_core::external_storage::{
    MockObjectStoreWrapper,
    SkObjectStore,
};
use sk_store::TracerConfig;

use super::*;

#[fixture]
fn store() -> Arc<Mutex<TraceStore>> {
    Arc::new(Mutex::new(TraceStore::new(TracerConfig::default())))
}

#[rstest(tokio::test)]
async fn test_export_helper_cloud(store: Arc<Mutex<TraceStore>>) {
    let req = ExportRequest {
        start_ts: 0,
        end_ts: 1,
        export_path: "s3://foo/bar".into(),
        filters: Box::new(Default::default()),
    };
    let mut object_store = MockObjectStoreWrapper::new();
    object_store.expect_put().returning(|_| Ok(())).once();
    object_store.expect_scheme().returning(|| ObjectStoreScheme::AmazonS3).once();

    let res = export_helper(&req, store, &object_store).await.unwrap();
    assert!(res.len() == 0);
}

#[rstest(tokio::test)]
async fn test_export_helper_local(store: Arc<Mutex<TraceStore>>) {
    let export_path = "memory:/foo";

    let req = ExportRequest {
        start_ts: 0,
        end_ts: 1,
        export_path: export_path.into(),
        filters: Box::new(Default::default()),
    };
    let object_store = SkObjectStore::new(export_path).unwrap();

    let res = export_helper(&req, store, &object_store).await.unwrap();
    assert!(res.len() > 0);
}
