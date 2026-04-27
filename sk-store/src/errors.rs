use sk_core::k8s::GVK;
use thiserror::Error;


#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid path for {0}")]
    InvalidPath(GVK),
    #[error("Missing pod spec path for {0}")]
    MissingPath(GVK),
}
