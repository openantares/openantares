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
use crate::error::CoreError;
use crate::event_time::EventTime;
use crate::evidence::EvidenceId;
use crate::ids::{ProjectId, TenantId, VertexId};

/// Stable identifier for an observation.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservationId(pub String);

/// An optimistic concurrency condition for one immutable revision chain.
///
/// `chain_id` is opaque to the engine. Its durable identity is scoped by
/// tenant, project and vault, so the same name in another scope is a different
/// chain. A normal successor names the exact previous revision. A first
/// guarded revision omits it. `initialize_from_existing` is the explicit
/// boundary for adopting an older, unguarded revision as the predecessor.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionalRevision {
    /// Opaque identity of the chain within its tenant/project/vault scope.
    pub chain_id: String,
    /// Exact head this revision expects, or `None` for first creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_previous_revision_id: Option<String>,
    /// Adopt the named older, unguarded revision as this chain's boundary.
    #[serde(default, skip_serializing_if = "is_false")]
    pub initialize_from_existing: bool,
}

/// Internal metadata slot that makes a condition part of the immutable
/// observation and therefore carries it through sync and `.ant` archives.
/// API conversions remove this implementation detail and expose the typed
/// `conditionalRevision` field instead.
pub const CONDITIONAL_REVISION_METADATA_KEY: &str = "__antaresConditionalRevision";
const ORIGINAL_METADATA_KEY: &str = "__antaresOriginalMetadata";
const METADATA_ENVELOPE_KEY: &str = "__antaresMetadataEnvelope";
const METADATA_ENVELOPE_VERSION: u64 = 1;

fn is_false(value: &bool) -> bool {
    !*value
}

impl ConditionalRevision {
    /// Maximum encoded size of an opaque chain id.
    pub const MAX_CHAIN_ID_BYTES: usize = 512;
    /// Maximum encoded size of current and previous revision ids.
    pub const MAX_REVISION_ID_BYTES: usize = 1024;

    /// Validate the condition against the id of the proposed revision.
    pub fn validate(&self, revision_id: &str) -> Result<(), CoreError> {
        fn valid_id(value: &str, max: usize) -> bool {
            !value.is_empty() && value.len() <= max && !value.contains('\0')
        }
        if !valid_id(&self.chain_id, Self::MAX_CHAIN_ID_BYTES) {
            return Err(CoreError::InvalidInput(format!(
                "conditional revision chainId must be 1..={} bytes and contain no NUL",
                Self::MAX_CHAIN_ID_BYTES
            )));
        }
        if !valid_id(revision_id, Self::MAX_REVISION_ID_BYTES) {
            return Err(CoreError::InvalidInput(format!(
                "conditional revision id must be 1..={} bytes and contain no NUL",
                Self::MAX_REVISION_ID_BYTES
            )));
        }
        if let Some(previous) = &self.expected_previous_revision_id {
            if !valid_id(previous, Self::MAX_REVISION_ID_BYTES) {
                return Err(CoreError::InvalidInput(format!(
                    "expectedPreviousRevisionId must be 1..={} bytes and contain no NUL",
                    Self::MAX_REVISION_ID_BYTES
                )));
            }
            if previous == revision_id {
                return Err(CoreError::InvalidInput(
                    "a conditional revision cannot name itself as its predecessor".into(),
                ));
            }
        }
        if self.initialize_from_existing && self.expected_previous_revision_id.is_none() {
            return Err(CoreError::InvalidInput(
                "initializeFromExisting requires expectedPreviousRevisionId".into(),
            ));
        }
        Ok(())
    }
}

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

impl Observation {
    /// Whether caller metadata is attempting to supply the engine-owned
    /// versioned envelope. A condition-shaped key on its own remains ordinary
    /// historical metadata; only the version marker claims engine semantics.
    pub fn metadata_uses_conditional_revision_envelope(metadata: &serde_json::Value) -> bool {
        metadata
            .as_object()
            .and_then(|object| object.get(METADATA_ENVELOPE_KEY))
            .and_then(serde_json::Value::as_u64)
            == Some(METADATA_ENVELOPE_VERSION)
    }

    /// Read the typed condition retained in this observation's internal
    /// metadata. A malformed reserved value is refused rather than ignored.
    pub fn conditional_revision(&self) -> Result<Option<ConditionalRevision>, CoreError> {
        let Some(object) = self.metadata.as_object() else {
            return Ok(None);
        };
        if !Self::metadata_uses_conditional_revision_envelope(&self.metadata) {
            return Ok(None);
        }
        let Some(value) = object.get(CONDITIONAL_REVISION_METADATA_KEY) else {
            return Err(CoreError::InvalidInput(
                "conditional revision metadata is not a valid engine envelope".into(),
            ));
        };
        if !object.contains_key(ORIGINAL_METADATA_KEY) || object.len() != 3 {
            return Err(CoreError::InvalidInput(
                "conditional revision metadata is not a valid engine envelope".into(),
            ));
        }
        serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|e| {
                CoreError::InvalidInput(format!("malformed conditional revision metadata: {e}"))
            })
    }

    /// Make `condition` part of the immutable stored record. Producer metadata
    /// is nested in a versioned engine envelope and restored exactly by
    /// `take_conditional_revision`, regardless of its JSON shape or keys.
    pub fn set_conditional_revision(
        &mut self,
        condition: ConditionalRevision,
    ) -> Result<(), CoreError> {
        condition.validate(&self.id.0)?;
        let value = serde_json::to_value(&condition)
            .map_err(|e| CoreError::InvalidInput(format!("conditional revision encode: {e}")))?;
        match self.conditional_revision()? {
            Some(existing) if existing == condition => return Ok(()),
            Some(_) => {
                return Err(CoreError::InvalidInput(
                    "metadata contains a different reserved conditional revision".into(),
                ))
            }
            None => {}
        }
        let original = std::mem::take(&mut self.metadata);
        let mut object = serde_json::Map::new();
        object.insert(
            METADATA_ENVELOPE_KEY.into(),
            serde_json::Value::from(METADATA_ENVELOPE_VERSION),
        );
        object.insert(ORIGINAL_METADATA_KEY.into(), original);
        object.insert(CONDITIONAL_REVISION_METADATA_KEY.into(), value);
        self.metadata = serde_json::Value::Object(object);
        Ok(())
    }

    /// Remove the internal representation for an API response and restore the
    /// caller's original metadata value.
    pub fn take_conditional_revision(&mut self) -> Result<Option<ConditionalRevision>, CoreError> {
        let condition = self.conditional_revision()?;
        if condition.is_none() {
            return Ok(None);
        }
        self.metadata = self
            .metadata
            .as_object()
            .and_then(|object| object.get(ORIGINAL_METADATA_KEY))
            .cloned()
            .expect("conditional_revision accepted only a complete envelope");
        Ok(condition)
    }
}
