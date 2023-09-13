use anyhow::anyhow;
use reqwest::Url;

pub enum Scheme {
    AmazonS3,
    Local,
}

pub fn fetch_from_s3(_path: String) -> anyhow::Result<()> {
    todo!();
}

pub fn save_to_s3() {
    todo!();
}

pub fn get_scheme(path: &Url) -> anyhow::Result<Scheme> {
    match path.scheme() {
        "s3" => Ok(Scheme::AmazonS3),
        "file" => Ok(Scheme::Local),
        s => Err(anyhow!("unrecognized storage scheme: {}", s)),
    }
}
