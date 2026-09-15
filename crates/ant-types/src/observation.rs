//! Antares-native observations.
//!
//! An **Observation** is an immutable, source-bound, evidence-backed
//! atomic fact extracted from a real event/source. Observations live
//! in their own append-only plane and feed the Belief layer.
//!
//! Examples:
//!   - "Marcus from legal joined the review" → subject=person_marcus,
//!     predicate=joined_review, source_event_id=meeting_001
//!   - "SOC2 was mentioned in the call" → subject=meeting_001,
//!     predicate=mentioned_topic, object_value="SOC2"
//!   - "pricing page opened 5×" → subject=person_marcus,
//!     predicate=page_open_count, object_value=5
//!
//! This module ships ONLY the observation plane; beliefs are their
//! own record kind.

use serde::{Deserialize, Serialize};

use crate::author::AuthorStamp;
use crate::event_time::EventTime;
use crate::evidence::EvidenceId;
use crate::ids::{ProjectId, TenantId, VertexId};

/// Stable identifier for an observation.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservationId(pub String);

/// An atomic, source-bound fact about a subject.
///
/// Append-only: once an observation is stored, it cannot be modified.
/// Re-submitting identical content under the same id is a no-op
/// (idempotent). Re-submitting different content under the same id is
/// a conflict.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Observation id, unique within the scope.
    pub id: ObservationId,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Owning project.
    pub project_id: ProjectId,

    /// Identifier of the source event (e.g. "meeting_001",
    /// "email_002", "crm_webhook_2026-04-29T18:02"). Optional because
    /// some observations are aggregated (e.g. "champion silent for 14
    /// days") and don't tie to a single event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_event_id: Option<String>,

    /// URI of the source artifact (e.g. "antares://transcripts/m1#1240-1295").
    /// Optional and may duplicate `evidence_ids[0].source_uri`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,

    /// Subject of the observation — typically a deal, person, meeting,
    /// or email vertex. Optional for observations that aren't anchored
    /// to a specific entity (e.g. aggregate behavioral signals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<VertexId>,

    /// What was observed about the subject. Free-form string — common
    /// values include "joined_review", "mentioned_topic", "viewed",
    /// "opened_email", "forwarded_to", "went_silent", "usage_dropped".
    pub predicate: String,

    /// Object of the predicate when the observation is relational.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<VertexId>,

    /// Object as a literal value when the observation isn't relational
    /// (e.g. "SOC2 was mentioned" → object_value = "SOC2";
    /// "page opens count" → object_value = 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_value: Option<serde_json::Value>,

    /// When the underlying event happened — the EVENT time. Either a
    /// real instant or explicitly [`EventTime::Unknown`] with a reason;
    /// a dateless original carries the reason and nothing ever
    /// fabricates an instant for it (PRODUCT-231). Serializes as the
    /// bare v0.5 timestamp string when known without a basis.
    pub observed_at: EventTime,
    /// When the extractor produced this observation — the PROVENANCE
    /// time. Same shape; a producer may populate it from an extraction
    /// receipt with a [`crate::TimeBasis`]. Explicitly unknown when the
    /// producer recorded no such time.
    pub extracted_at: EventTime,

    /// Confidence in `[0,1]`; `None` is treated as 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// First-class evidence references. Each Observation should point
    /// at one or more Evidence records that back it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_ids: Vec<EvidenceId>,

    /// Version tag of the producing extractor, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor_version: Option<String>,

    /// Free-form metadata for extractor-specific extras.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,

    /// Which user authored this observation. `None` for older
    /// records and for anonymous calls. Set by the writer from the
    /// resolved authentication context at write time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,
}
