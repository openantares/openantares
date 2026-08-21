//! Domain-layer error type shared by every record plane.

use thiserror::Error;

/// Errors raised by the core domain layer.
///
/// Consumers may map these to HTTP statuses.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A required configuration field is absent.
    #[error("invalid config: missing required field `{field}`")]
    MissingConfigField {
        /// Name of the missing field.
        field: String,
    },

    /// A configuration field has the wrong JSON type.
    #[error("invalid config: field `{field}` has wrong type")]
    WrongConfigType {
        /// Name of the mistyped field.
        field: String,
    },

    /// No project exists with the given id.
    #[error("project not found: id={0:?}")]
    ProjectNotFound(crate::ProjectId),

    /// A project already claims the given namespace.
    #[error("project namespace already exists: {0}")]
    NamespaceConflict(String),

    /// The input failed validation; the message says why.
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
