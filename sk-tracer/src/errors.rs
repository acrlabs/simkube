use std::sync::{
    MutexGuard,
    PoisonError,
};

use rocket::Responder;
use sk_store::TraceStore;

#[derive(Responder)]
pub enum ExportResponseError {
    #[response(status = 404)]
    StorageNotFound(String),

    #[response(status = 500)]
    TracerError(String),

    #[response(status = 502)]
    StorageError(String),
}

impl From<anyhow::Error> for ExportResponseError {
    fn from(e: anyhow::Error) -> Self {
        Self::TracerError(format!("SimKube error: {e}"))
    }
}

impl From<PoisonError<MutexGuard<'_, TraceStore>>> for ExportResponseError {
    fn from(e: PoisonError<MutexGuard<'_, TraceStore>>) -> Self {
        Self::TracerError(format!("Mutex was poisoned: {e}"))
    }
}

impl From<url::ParseError> for ExportResponseError {
    fn from(e: url::ParseError) -> Self {
        Self::TracerError(format!("Could not parse URL: {e}"))
    }
}

impl From<object_store::Error> for ExportResponseError {
    fn from(e: object_store::Error) -> Self {
        match e {
            object_store::Error::NotFound { .. } => {
                Self::StorageNotFound(format!("Could not find external storage location: {e}"))
            },
            _ => Self::StorageError(format!("Could not write to object store: {e}")),
        }
    }
}
