use super::*;

#[rstest]
fn test_new_sk_object_store_invalid() {
    let _ = SkObjectStore::new("oracle3://foo/bar").unwrap_err();
}

#[rstest]
fn test_new_sk_object_store() {
    let store = SkObjectStore::new("s3://foo/bar").unwrap();
    assert_eq!(store.scheme(), ObjectStoreScheme::AmazonS3);
}
