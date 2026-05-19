use thiserror::*;

#[derive(Debug, Error)]
pub enum SkelError {
    #[error("variable {0} is not present in selector {1} on left-hand side")]
    InvalidLHS(String, String),

    #[error("{0}: values = {1}")]
    MultipleMatchingValues(String, String),

    #[error("variable {0} already defined")]
    MultipleVariableDefinitions(String),

    #[error("variable {0} not defined")]
    UndefinedVariable(String),
}
