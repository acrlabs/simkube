use thiserror::Error;

pub type SimKubeResult<T, E = SimKubeError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum SimKubeError {
    #[error("error decoding trace data")]
    DeserializationError(#[from] rmp_serde::decode::Error),

    #[error("field not present in Kubernetes object")]
    FieldNotFound,

    #[error("could not read file")]
    FileIOError(#[from] std::io::Error),

    #[error("error communicating with the apiserver")]
    KubeApiError(#[from] kube::Error),

    #[error("parse error")]
    ParseError(#[from] url::ParseError),

    #[error("error serializing trace data")]
    SerializationError(#[from] rmp_serde::encode::Error),

    #[error("unrecognized trace scheme: {0}")]
    UnrecognizedTraceScheme(String),
}
