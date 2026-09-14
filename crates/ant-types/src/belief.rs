//! Antares-native beliefs.
//!
//! A **Belief** is the system's *current inferred state* about a
//! subject (deal / account / person / meeting / etc.). Beliefs are:
//!
//!   - **Direction-neutral by construction**. A belief is a fact, not
//!     a judgement. Direction (positive / negative / neutral) belongs
//!     to whatever downstream rules interpret beliefs.
//!   - **Derived from observations**. Every belief carries
//!     `derived_from: Vec<ObservationId>` and `evidence_ids:
//!     Vec<EvidenceId>` so its reasoning chain is traceable.
//!   - **Versioned**. Each (`subject`, `predicate`) pair has a
//!     monotonically-increasing `belief_version`. New versions
//!     supersede old ones; old ones are kept for time-travel and
//!     trajectory analysis (deferred).
//!
//! Examples (drawn from the Antares method, all direction-neutral):
//!
//!   - subject = `person_marcus`,
//!     predicate = `role_in_deal`,
//!     value = `{ "role": "gatekeeper", "domain": "legal" }`
//!   - subject = `deal_hooli_001`,
//!     predicate = `compliance_gate_state`,
//!     value = `{ "state": "developing", "topics": [...] }`
//!   - subject = `deal_betaops`,
//!     predicate = `buying_committee_state`,
//!     value = `{ "state": "forming" }`
//!   - subject = `deal_helix`,
//!     predicate = `usage_trend`,
//!     value = `{ "trend": "falling", "delta": -0.22 }`
//!   - subject = `deal_lowfit`,
//!     predicate = `attention_leverage`,
//!     value = `{ "level": "low" }`
//!
//! This module ships ONLY the belief plane: the record shape as it
//! appears in a `.ant` file. How beliefs get materialized from
//! observations is a producer concern outside this crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::author::{AuthorStamp, UserId};
use crate::evidence::EvidenceId;
use crate::ids::{ProjectId, TenantId, VertexId};
use crate::observation::ObservationId;

/// Stable identifier for a belief — server-generated UUID. Every
/// version of a (subject, predicate) belief has its own distinct id.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BeliefId(pub String);

/// One version of an inferred fact about a subject. The payload of a
/// `belief` record.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Belief {
    /// Server-assigned. Unique per version.
    pub id: BeliefId,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Owning project.
    pub project_id: ProjectId,
    /// The vertex this belief is about.
    pub subject_id: VertexId,
    /// What aspect of the subject this belief is about.
    /// Examples: `role_in_deal`, `compliance_gate_state`,
    /// `buying_committee_state`, `usage_trend`, `attention_leverage`,
    /// `renewal_risk`, `expansion_readiness`.
    pub predicate: String,
    /// The inferred value as JSON. Can be a string, number, boolean,
    /// or structured object. Direction-neutral by construction.
    pub value_json: Value,
    /// Confidence in `[0,1]`; `None` when the producer does not score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Server-assigned. Monotonically increasing per
    /// (`subject_id`, `predicate`) pair. Latest version supersedes
    /// older ones; history is retained.
    pub belief_version: u64,
    /// Observations that this belief was derived from. Empty for
    /// beliefs that don't yet have a traced derivation chain
    /// (allowed but discouraged).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<ObservationId>,
    /// Evidence accumulated from `derived_from` observations (and
    /// optionally extra evidence the materializer attached directly).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_ids: Vec<EvidenceId>,
    /// Start of real-world validity; `None` = always was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateTime<Utc>>,
    /// End of real-world validity (exclusive); `None` = still holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateTime<Utc>>,
    /// Wall-clock time the underlying facts were observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    /// Server-assigned. Wall-clock time of the version's insertion.
    pub updated_at: DateTime<Utc>,
    /// Optional decay policy hint (e.g. `"halflife_days=14"`). Stored
    /// but not applied here; consumers may interpret it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decay_policy: Option<String>,
    /// Free-form metadata for materializer-specific extras.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,

    /// Which user authored this belief directly. For a human-written
    /// belief this is a human author's user id; for a belief produced
    /// by an automated materializer this is the service identity that
    /// ran the materialization step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,

    /// Union of `author.user_id` across every source observation
    /// (and chained source belief, when derivations are stacked)
    /// consumed to produce this belief. Lets downstream queries
    /// answer "this belief is built on whose contributions?".
    /// Deduplicated and sorted at write time. Empty for human-written
    /// beliefs that aren't derived from anything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributing_authors: Vec<UserId>,
}

/// Content fingerprint used to detect "same belief as the latest
/// version" — if the incoming draft matches the latest version's
/// content, the store returns the existing record idempotently and
/// does NOT create a new version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BeliefContent {
    /// Canonical JSON string of the value.
    pub value_json: String,
    /// Confidence as raw `f32` bits (`f32` itself has no `Hash`).
    pub confidence_bits: Option<u32>,
    /// Sorted source observation ids.
    pub derived_from: Vec<ObservationId>,
    /// Sorted evidence ids.
    pub evidence_ids: Vec<EvidenceId>,
    /// Start of real-world validity.
    pub valid_from: Option<DateTime<Utc>>,
    /// End of real-world validity (exclusive).
    pub valid_to: Option<DateTime<Utc>>,
    /// Wall-clock observation time.
    pub observed_at: Option<DateTime<Utc>>,
    /// Decay policy hint, verbatim.
    pub decay_policy: Option<String>,
    /// Canonical JSON string of the metadata.
    pub metadata: String,
}

impl Belief {
    /// Stable content fingerprint, ignoring server-assigned fields
    /// (id, belief_version, updated_at).
    pub fn content_fingerprint(&self) -> BeliefContent {
        BeliefContent {
            value_json: self.value_json.to_string(),
            // f32 doesn't impl Hash; map to bits.
            confidence_bits: self.confidence.map(f32::to_bits),
            derived_from: {
                let mut v = self.derived_from.clone();
                v.sort();
                v
            },
            evidence_ids: {
                let mut v = self.evidence_ids.clone();
                v.sort();
                v
            },
            valid_from: self.valid_from,
            valid_to: self.valid_to,
            observed_at: self.observed_at,
            decay_policy: self.decay_policy.clone(),
            metadata: self.metadata.to_string(),
        }
    }
}
