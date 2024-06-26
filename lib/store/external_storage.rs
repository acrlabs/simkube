use async_trait::async_trait;
use bytes::Bytes;
use object_store::path::Path;
use object_store::{
    DynObjectStore,
    PutPayload,
};
use reqwest::Url;

use crate::errors::*;

/// We use the [object_store](https://docs.rs/object_store/latest/object_store/index.html) crate to
/// enable reading/writing from the three major cloud providers (AWS, Azure, GCP), as well as
/// to/from a local filesystem or an in-memory store.  Supposedly HTTP with WebDAV is supported as
/// well but that is completely untested.
///
/// The reader will load credentials from the environment to communicate with the cloud provider,
/// as follows (other auth mechanisms _may_ work as well but are currently untested):
///
/// ### AWS
///
/// Set the `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` environment variables, and pass in a
/// URL like `s3://bucket/path/to/resource`.
///
/// ### Azure
///
/// Set the `AZURE_STORAGE_ACCOUNT_NAME` and `AZURE_STORAGE_ACCOUNT_KEY` environment variables, and
/// pass in a URL like `azure://container/path/to/resources` (do not include the storage acocunt
/// name in the URL).
///
/// ### GCP
///
/// Set the `GOOGLE_SERVICE_ACCOUNT` environment variable to the path for your service account JSON
/// file (if you're running inside a container, you'll need that file injected as well).  Pass in a
/// URL like `gs://bucket/path/to/resource`.

// This code is copy-pasta'ed from the object_store library because it is currently private
// in that library.  This code can all be deleted if/once https://github.com/apache/arrow-rs/pull/5912
// is merged.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObjectStoreScheme {
    Local,
    Memory,
    AmazonS3,
    GoogleCloudStorage,
    MicrosoftAzure,
    Http,
}

impl ObjectStoreScheme {
    pub fn parse(url: &Url) -> anyhow::Result<(Self, Path)> {
        let strip_bucket = || Some(url.path().strip_prefix('/')?.split_once('/')?.1);

        let (scheme, path) = match (url.scheme(), url.host_str()) {
            ("file", None) => (Self::Local, url.path()),
            ("memory", None) => (Self::Memory, url.path()),
            ("s3" | "s3a", Some(_)) => (Self::AmazonS3, url.path()),
            ("gs", Some(_)) => (Self::GoogleCloudStorage, url.path()),
            ("az" | "adl" | "azure" | "abfs" | "abfss", Some(_)) => (Self::MicrosoftAzure, url.path()),
            ("http", Some(_)) => (Self::Http, url.path()),
            ("https", Some(host)) => {
                if host.ends_with("dfs.core.windows.net")
                    || host.ends_with("blob.core.windows.net")
                    || host.ends_with("dfs.fabric.microsoft.com")
                    || host.ends_with("blob.fabric.microsoft.com")
                {
                    (Self::MicrosoftAzure, url.path())
                } else if host.ends_with("amazonaws.com") {
                    match host.starts_with("s3") {
                        true => (Self::AmazonS3, strip_bucket().unwrap_or_default()),
                        false => (Self::AmazonS3, url.path()),
                    }
                } else if host.ends_with("r2.cloudflarestorage.com") {
                    (Self::AmazonS3, strip_bucket().unwrap_or_default())
                } else {
                    (Self::Http, url.path())
                }
            },
            _ => bail!("unrecognized url: {url}"),
        };

        Ok((scheme, Path::from_url_path(path)?))
    }
}

// End copy-pasta'ed code

#[cfg(feature = "testutils")]
use mockall::automock;

#[cfg_attr(feature = "testutils", automock)]
#[async_trait]
pub trait ObjectStoreWrapper {
    fn scheme(&self) -> ObjectStoreScheme;
    async fn put(&self, data: Bytes) -> EmptyResult;
    async fn get(&self) -> anyhow::Result<Bytes>;
}

#[derive(Debug)]
pub struct SkObjectStore {
    scheme: ObjectStoreScheme,
    store: Box<DynObjectStore>,
    path: Path,
}

impl SkObjectStore {
    pub fn new(path_str: &str) -> anyhow::Result<SkObjectStore> {
        let url = Url::parse(path_str)?;
        let (scheme, path) = ObjectStoreScheme::parse(&url)?;
        let store: Box<DynObjectStore> = match scheme {
            ObjectStoreScheme::Local => Box::new(object_store::local::LocalFileSystem::new()),
            ObjectStoreScheme::Memory => Box::new(object_store::memory::InMemory::new()),
            ObjectStoreScheme::AmazonS3 => {
                Box::new(object_store::aws::AmazonS3Builder::from_env().with_url(path_str).build()?)
            },
            ObjectStoreScheme::MicrosoftAzure => Box::new(
                object_store::azure::MicrosoftAzureBuilder::from_env()
                    .with_url(path_str)
                    .build()?,
            ),
            ObjectStoreScheme::GoogleCloudStorage => Box::new(
                object_store::gcp::GoogleCloudStorageBuilder::from_env()
                    .with_url(path_str)
                    .build()?,
            ),
            ObjectStoreScheme::Http => Box::new(object_store::http::HttpBuilder::new().with_url(path_str).build()?),
        };

        Ok(SkObjectStore { scheme, store, path })
    }
}

#[async_trait]
impl ObjectStoreWrapper for SkObjectStore {
    fn scheme(&self) -> ObjectStoreScheme {
        self.scheme.clone()
    }

    async fn put(&self, data: Bytes) -> EmptyResult {
        let payload = PutPayload::from_bytes(data);
        self.store.put(&self.path, payload).await?;
        Ok(())
    }

    async fn get(&self) -> anyhow::Result<Bytes> {
        Ok(self.store.get(&self.path).await?.bytes().await?)
    }
}
