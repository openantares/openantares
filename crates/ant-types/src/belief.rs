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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BeliefId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Belief {
    /// Server-assigned. Unique per version.
    pub id: BeliefId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub subject_id: VertexId,
    /// What aspect of the subject this belief is about.
    /// Examples: `role_in_deal`, `compliance_gate_state`,
    /// `buying_committee_state`, `usage_trend`, `attention_leverage`,
    /// `renewal_risk`, `expansion_readiness`.
    pub predicate: String,
    /// The inferred value as JSON. Can be a string, number, boolean,
    /// or structured object. Direction-neutral by construction.
    pub value_json: Value,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateTime<Utc>>,
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
    pub value_json: String,
    pub confidence_bits: Option<u32>,
    pub derived_from: Vec<ObservationId>,
    pub evidence_ids: Vec<EvidenceId>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub observed_at: Option<DateTime<Utc>>,
    pub decay_policy: Option<String>,
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
