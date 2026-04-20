use sk_core::k8s::GVK;
use thiserror::Error;


#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Invalid path for {gvk}: {path}, expected: {expected}")]
    InvalidPath { gvk: GVK, path: String, expected: String },
    #[error("Missing pod spec path for {gvk}")]
    MissingPath { gvk: GVK },
}
