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
use std::path::{
    absolute,
    PathBuf,
};

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
#[cfg(feature = "mock")]
use mockall::automock;
use object_store::path::Path;
use object_store::{
    DynObjectStore,
    ObjectStoreScheme,
    PutPayload,
};
use reqwest::Url;

use crate::errors::*;

#[cfg_attr(feature = "mock", automock)]
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
        let (scheme, path) = parse_path(path_str)?;
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
            _ => unimplemented!(),
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

fn parse_path(path_str: &str) -> anyhow::Result<(ObjectStoreScheme, Path)> {
    let url = match Url::parse(path_str) {
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let path = absolute_strip_dots(path_str)?;
            Url::from_file_path(path).map_err(|e| anyhow!("could not create URL from file path: {e:?}"))?
        },
        res => res?,
    };

    Ok(ObjectStoreScheme::parse(&url)?)
}

fn absolute_strip_dots(path_str: &str) -> anyhow::Result<PathBuf> {
    // We have to use `absolute` here in the event that the path doesn't exist locally,
    // e.g., we're specifying a path inside the driver container.  `Path::canonicalize`
    // requires the path to exist locally.  Unfortunately, `absolute` does not strip `..`,
    // and ObjectStoreScheme::parse will not parse `..`, so we have to do that ourselves.
    let orig_path = absolute(PathBuf::from(path_str))?;
    let mut new_path = PathBuf::new();

    for component in orig_path.iter() {
        if component == ".." {
            if !new_path.pop() {
                bail!("malformed relative path: {path_str}");
            }
        } else {
            new_path.push(component);
        }
    }

    Ok(new_path)
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use assertables::*;
    use sk_testutils::*;

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

    #[rstest]
    #[case::with_base("file:///tmp/foo")]
    #[case::without_base("/tmp/foo")]
    #[case::absolute_with_dots("/tmp/../foo/bar/../../baz")]
    #[case::relative("foo")]
    #[case::relative_path_with_dots("../foo")]
    fn test_new_sk_object_store_local_path(#[case] path: &str) {
        let store = SkObjectStore::new(path).unwrap();
        assert_eq!(store.scheme(), ObjectStoreScheme::Local);
    }

    #[rstest]
    fn test_new_sk_object_store_invalid_path() {
        let res = SkObjectStore::new("/foo/../..");
        assert_err!(res);
    }
}
