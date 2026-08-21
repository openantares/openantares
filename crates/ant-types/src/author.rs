//! Record authorship — the provenance stamp carried on insight-plane
//! records.
//!
//! Only the part of the engine's auth model that appears ON RECORDS
//! lives here. Roles, tokens, memberships, scope grants and everything
//! else about authenticating a caller stay in `antares-core`: a reader
//! of an `.ant` file has to understand who authored a belief, not how
//! the engine decided to let them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stable identifier for a user.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub String);

/// Stable identifier for a token record.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TokenId(pub String);

/// Token subject class. Lets a consumer tell "a person wrote this in
/// the client" from "a service connector wrote this".
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    /// A person, acting through an interactive client.
    User,
    /// A service connector or automation.
    Service,
    /// A desktop client instance.
    Desktop,
}

/// Per-record authorship stamp.
///
/// Carried as `Option<AuthorStamp>` on `Observation`, `Belief` and
/// `Evidence` so "whose call produced this insight" survives an export,
/// even though every team member writes into the same tenant-scoped
/// store. `None` for records written before authorship existed and for
/// anonymous compat-mode calls.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorStamp {
    /// The originating user id. For service tokens this is
    /// `"service:<subject_id>"` so service-written records are
    /// visually distinguishable from human-written ones.
    pub user_id: UserId,
    /// The token id that minted the context, when present. None for
    /// local-bootstrap contexts that don't transit a token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_id: Option<TokenId>,
    /// What class of subject authored the record.
    pub subject_type: SubjectType,
    /// Wall-clock time the authoring happened. Distinct from
    /// `observed_at` / `extracted_at`, which are content timestamps;
    /// this is the persistence timestamp.
    pub authored_at: DateTime<Utc>,
}
