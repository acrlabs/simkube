use std::sync::{
    Arc,
    Mutex,
};

use sk_core::external_storage::{
    MockObjectStoreWrapper,
    ObjectStoreScheme,
    SkObjectStore,
};

use super::*;

#[fixture]
fn trace_store() -> Arc<Mutex<TraceStore>> {
    Arc::new(Mutex::new(TraceStore::new(Default::default())))
}

#[rstest]
#[tokio::test]
async fn test_export_helper_cloud(trace_store: Arc<Mutex<TraceStore>>) {
    let req = ExportRequest {
        start_ts: 0,
        end_ts: 1,
        export_path: "s3://foo/bar".into(),
        filters: Box::new(Default::default()),
    };
    let mut object_store = MockObjectStoreWrapper::new();
    object_store.expect_put().returning(|_| Ok(())).once();
    object_store.expect_scheme().returning(|| ObjectStoreScheme::AmazonS3).once();

    let res = export_helper(&req, &trace_store, &object_store).await.unwrap();
    assert!(res.len() == 0);
}

#[rstest]
#[tokio::test]
async fn test_export_helper_local(trace_store: Arc<Mutex<TraceStore>>) {
    let export_path = "memory:/foo";

    let req = ExportRequest {
        start_ts: 0,
        end_ts: 1,
        export_path: export_path.into(),
        filters: Box::new(Default::default()),
    };
    let object_store = SkObjectStore::new(export_path).unwrap();

    let res = export_helper(&req, &trace_store, &object_store).await.unwrap();
    assert!(res.len() > 0);
}
