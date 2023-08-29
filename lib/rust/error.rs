use thiserror::Error;

pub type SimKubeResult<T, E = SimKubeError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum SimKubeError {
    #[error("kube-api-error")]
    KubeApiError(String),

    #[error("field-not-found")]
    FieldNotFound,
}

impl From<kube::Error> for SimKubeError {
    fn from(err: kube::Error) -> Self {
        SimKubeError::KubeApiError(err.to_string())
    }
}
