use thiserror::*;

#[derive(Debug, Error)]
pub enum SkelError {
    #[error("variable {0} must be present in selector {1} on left-hand side")]
    InvalidLHS(String, String),

    #[error("variable {0} not defined")]
    UndefinedVariable(String),
}
