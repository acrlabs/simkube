use sk_core::errors::*;

err_impl! {SkControllerError,
    #[error("configmap {0} not found")]
    ConfigmapNotFound(String),

    #[error("missing status field: {0}")]
    MissingStatusField(String),

    #[error("namespace {0} not found")]
    NamespaceNotFound(String),
}
