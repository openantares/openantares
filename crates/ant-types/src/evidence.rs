//! Antares-native first-class evidence.
//!
//! Evidence is a kernel concept, not a user-defined entity. Every fact
//! (Edge) may reference one or more Evidence records via `evidenced_by`.
//! Tripwires recursively accumulate evidence from their causes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::author::AuthorStamp;
use crate::ids::{ProjectId, TenantId};

/// Evidence identifier (newtype over String). Distinct from VertexId
/// because evidence lives in its own storage plane.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    /// URI/path of the source artifact (e.g. "s3://antares/calls/2026-04-29.vtt"
    /// or "antares://transcripts/meeting_001"). Required.
    pub source_uri: String,
    /// What kind of source: "transcript" | "email_event" | "crm_field" | ...
    pub source_type: String,
    /// Identifier of the source event/artifact (e.g. "meeting_001",
    /// "email_001"). Required.
    pub source_id: String,
    /// The literal text/data the evidence points at. Required.
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_end: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_start: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_end: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor_version: Option<String>,
    /// Confidence in `[0,1]`. `None` = treated as 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Free-form metadata. Use sparingly — first-class fields above
    /// are preferred.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,

    /// Which user persisted this evidence. `None` for older
    /// records and for anonymous calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,
}

impl Evidence {
    /// Shorthand constructor for tests/fixtures.
    pub fn quick(
        id: &str,
        tenant: TenantId,
        project: ProjectId,
        source_type: &str,
        source_id: &str,
        content: &str,
    ) -> Self {
        Self {
            id: EvidenceId(id.to_string()),
            tenant_id: tenant,
            project_id: project,
            source_uri: format!("antares://{source_type}/{source_id}"),
            source_type: source_type.to_string(),
            source_id: source_id.to_string(),
            content: content.to_string(),
            char_start: None,
            char_end: None,
            byte_start: None,
            byte_end: None,
            observed_at: None,
            extracted_at: None,
            extractor_version: None,
            confidence: None,
            metadata: serde_json::Value::Null,
            author: None,
        }
    }
}
