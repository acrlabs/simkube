use std::ops::Deref;

use sk_core::errors::*;

// This is sortof a stupid hack, because anyhow::Error doesn't derive from
// std::error::Error, but the reconcile functions require you to return a
// result that derives from std::error::Error.  So we just wrap the anyhow,
// and then implement deref for it so we can get back to the underlying error
// wherever we actually care.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct AnyhowError(#[from] anyhow::Error);

impl Deref for AnyhowError {
    type Target = anyhow::Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

err_impl! {SkControllerError,
    #[error("configmap {0} not found")]
    ConfigmapNotFound(String),

    #[error("missing status field: {0}")]
    MissingStatusField(String),

    #[error("namespace {0} not found")]
    NamespaceNotFound(String),
}
