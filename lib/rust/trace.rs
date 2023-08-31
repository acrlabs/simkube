use reqwest::Url;

use crate::prelude::*;

pub enum Scheme {
    AmazonS3,
    Local,
}

pub fn fetch_from_s3(_path: String) -> SimKubeResult<()> {
    todo!();
}

pub fn save_to_s3() {
    todo!();
}

pub fn storage_type(path: &Url) -> SimKubeResult<Scheme> {
    match path.scheme() {
        "s3" => Ok(Scheme::AmazonS3),
        "file" => Ok(Scheme::Local),
        s => Err(SimKubeError::UnrecognizedTraceScheme(s.into())),
    }
}
