use thiserror::Error;

/// Errors raised by the core domain layer.
///
/// Consumers may map these to HTTP statuses.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid config: missing required field `{field}`")]
    MissingConfigField { field: String },

    #[error("invalid config: field `{field}` has wrong type")]
    WrongConfigType { field: String },

    #[error("project not found: id={0:?}")]
    ProjectNotFound(crate::ProjectId),

    #[error("project namespace already exists: {0}")]
    NamespaceConflict(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Append-only resource (typically Observation) was re-submitted with
    /// the same id but different content. Mapped to HTTP 409.
    #[error("conflict: {0}")]
    Conflict(String),

    /// No (or invalid) authentication credentials were presented.
    /// Mapped to HTTP 401 Unauthorized.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Caller is authenticated but lacks the required scope or
    /// tenant binding for the action. Mapped to HTTP 403 Forbidden.
    #[error("forbidden: {0}")]
    Forbidden(String),
}
