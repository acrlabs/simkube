use thiserror::Error;

pub type SimKubeResult<T, E = SimKubeError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum SimKubeError {
    #[error("field-not-found")]
    FieldNotFound,

    #[error("kube-api-error")]
    KubeApiError(String),

    #[error("parse-error")]
    ParseError(String),

    #[error("unrecognized-trace-scheme")]
    UnrecognizedTraceScheme(String),
}

impl From<kube::Error> for SimKubeError {
    fn from(err: kube::Error) -> Self {
        SimKubeError::KubeApiError(err.to_string())
    }
}

impl From<url::ParseError> for SimKubeError {
    fn from(err: url::ParseError) -> Self {
        SimKubeError::KubeApiError(err.to_string())
    }
}
