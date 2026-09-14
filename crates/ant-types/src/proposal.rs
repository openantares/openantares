//! Relationship proposals (`.ant` v0.5): first-class records of a
//! relationship the recon loop proposed, what it measured, and what
//! was decided about it.
//!
//! A proposal is KNOWLEDGE about a source, not a log line. The loop
//! that finds candidate joins measures each one, quarantines the ones
//! the data refuses, and hands the rest to a reviewer — and every part
//! of that lived in receipts local to one server, which is why a
//! grader reading an archive saw zero relationship proposals twice.
//! Carried as a native kind it exports, imports, syncs to followers
//! and verifies like a belief or a contradiction case.
//!
//! What it carries is REFERENCES — the findings it was drawn from, the
//! SQL probes that measured it, the reviewer's receipt when one
//! promoted it — never copies. That is what makes closure enforceable:
//! an archive holding a proposal must hold every record the proposal
//! cites, and [`RelationshipProposal::references`] is the one list a
//! checker, an exporter and an importer all read.
//!
//! **Recording a proposal never publishes it.** A proposal is a claim
//! about a source that somebody may act on; turning one into a mapping
//! is a separate, reviewed act with its own rules, and nothing here
//! changes them.
//!
//! **Promotion is a trusted human action, never a model boolean.**
//! [`ProposalStatus::PromotedByReviewer`] cannot be spelled without a
//! [`ReviewerReceipt`] naming who decided, when, why, and the evidence
//! record holding the receipt — and that record is closure-checked, so
//! a promotion whose receipt does not exist is not a promotion.
//!
//! Revisions are immutable. A change is a NEW record with a new `id`,
//! the same `proposal_id`, and `previous_revision_id` naming what it
//! supersedes: a quarantined hypothesis that a reviewer later promotes
//! is two revisions, not an edit.
//!
//! Wire casing is camelCase throughout, on the HTTP surface and inside
//! the `.ant` payload alike — one shape, so a captured mutation IS the
//! committed record.
//!
//! # The measurement (PRODUCT-192's contract)
//!
//! [`RelationSupport`], [`Sampling`] and [`Normalization`] live here
//! because the archive carries them: the mapper's proposal/mapping
//! contract and this record are the same contract, and a second
//! definition would be a second thing to get wrong. `antares-mapper`
//! re-exports these and keeps the mapping-side compatibility policy.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::author::AuthorStamp;
use crate::error::CoreError;
use crate::evidence::EvidenceId;
use crate::ids::{ProjectId, TenantId};

// ---------------------------------------------------------------------
// The measurement contract (PRODUCT-192), the archive's copy of record
// ---------------------------------------------------------------------

/// Version of the evidence contract itself. Bumped when the SHAPE
/// changes; a document declaring a version this build does not know is
/// refused rather than read with the wrong meaning.
pub const SUPPORT_CONTRACT_VERSION: u32 = 1;

/// How the support was measured. A closed set: an unknown method is a
/// parse error, because "some other method" is not evidence.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportMethod {
    /// Count source rows whose (normalized) key matches a target key,
    /// over source rows with a non-null key.
    JoinMatchScan,
}

/// How the rows the measurement covered were chosen.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingMethod {
    /// Every row was read. `percent`, `seed` and `cap` must be absent.
    FullScan,
    /// PostgreSQL `TABLESAMPLE SYSTEM (percent) REPEATABLE (seed)`.
    SystemRepeatable,
    /// `TABLESAMPLE BERNOULLI (percent) REPEATABLE (seed)`.
    BernoulliRepeatable,
    /// A bounded prefix: `LIMIT cap` under a deterministic order.
    CappedPrefix,
}

/// The sample a measurement was taken over, described well enough to
/// be taken again.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sampling {
    /// How rows were chosen.
    pub method: SamplingMethod,
    /// Sampled percentage, when the method takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    /// Seed, so the same sample can be taken again. A sampled
    /// measurement without one cannot be re-derived, and is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// Row cap, for `capped_prefix`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<u64>,
}

impl Sampling {
    /// Every row was read.
    pub fn full_scan() -> Self {
        Self {
            method: SamplingMethod::FullScan,
            percent: None,
            seed: None,
            cap: None,
        }
    }

    fn check(&self, where_: &str) -> Result<(), String> {
        match self.method {
            SamplingMethod::FullScan => {
                if self.percent.is_some() || self.seed.is_some() || self.cap.is_some() {
                    return Err(format!(
                        "{where_}: sampling.method is full_scan, so percent/seed/cap must be \
                         absent — a full scan has no sample to describe"
                    ));
                }
            }
            SamplingMethod::SystemRepeatable | SamplingMethod::BernoulliRepeatable => {
                let Some(p) = self.percent else {
                    return Err(format!(
                        "{where_}: sampling.percent is required for {:?}",
                        self.method
                    ));
                };
                if !(p > 0.0 && p <= 100.0) {
                    return Err(format!("{where_}: sampling.percent {p} is not in (0, 100]"));
                }
                if self.seed.is_none() {
                    return Err(format!(
                        "{where_}: sampling.seed is required for {:?} — a sampled measurement \
                         nobody can take again is not evidence",
                        self.method
                    ));
                }
            }
            SamplingMethod::CappedPrefix => {
                let Some(cap) = self.cap else {
                    return Err(format!(
                        "{where_}: sampling.cap is required for capped_prefix"
                    ));
                };
                if cap == 0 {
                    return Err(format!("{where_}: sampling.cap must be positive"));
                }
            }
        }
        Ok(())
    }
}

/// The measurement behind an inferred relationship.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationSupport {
    /// [`SUPPORT_CONTRACT_VERSION`] this record was written against.
    pub contract_version: u32,
    /// How it was measured.
    pub method: SupportMethod,
    /// That method's version.
    pub method_version: u32,
    /// Rows in the source relation the measurement covered.
    pub source_rows: u64,
    /// …of which this many have a non-null join key. THE DENOMINATOR:
    /// a null key is not a failed match, it is no reference at all.
    pub source_non_null: u64,
    /// …of which this many matched a target row. The numerator.
    pub matched_rows: u64,
    /// Rows in the target relation the measurement covered.
    pub target_rows: u64,
    /// …with a non-null key.
    pub target_non_null: u64,
    /// …DISTINCT non-null keys. Equal to `target_non_null` exactly when
    /// the key identifies at most one target row, which is what a
    /// relationship into an entity requires.
    pub target_distinct: u64,
    /// The sample the counts were taken over.
    pub sampling: Sampling,
    /// Fingerprint of the raw measurement (the server's own digest of
    /// the counts and the query that produced them), so a published
    /// record can be tied back to the run that measured it.
    pub fingerprint: String,
    /// The threshold this measurement was judged against, carried WITH
    /// the evidence. A ratio without the bar it cleared is not a claim.
    pub min_support: f64,
}

impl RelationSupport {
    /// Matched over non-null source rows. `None` when the denominator
    /// is empty — which is refused, never read as 0 or 1.
    pub fn ratio(&self) -> Option<f64> {
        if self.source_non_null == 0 {
            None
        } else {
            Some(self.matched_rows as f64 / self.source_non_null as f64)
        }
    }

    /// Is this evidence good enough to materialize edges from?
    /// `where_` names the relationship in the error.
    pub fn check(&self, where_: &str) -> Result<(), String> {
        self.check_coherent(where_)?;
        let Some(ratio) = self.ratio() else {
            return Err(format!(
                "{where_}: support has an EMPTY DENOMINATOR (no source row carries a non-null \
                 key), so nothing was measured. An unmeasured join is not a supported one"
            ));
        };
        if self.matched_rows == 0 {
            return Err(format!(
                "{where_}: support is ZERO — {} source rows carry a key and none of them match \
                 a target row. Materializing this would fabricate every edge",
                self.source_non_null
            ));
        }
        if ratio < self.min_support {
            return Err(format!(
                "{where_}: support {:.4} ({}/{}) is below the declared minimum {:.4}",
                ratio, self.matched_rows, self.source_non_null, self.min_support
            ));
        }
        if self.target_non_null == 0 {
            return Err(format!(
                "{where_}: the target side has no non-null keys, so no source row can resolve \
                 to a target identity"
            ));
        }
        if self.target_distinct != self.target_non_null {
            return Err(format!(
                "{where_}: the target key is NOT UNIQUE ({} non-null values, {} distinct) — a \
                 relationship into an entity must land on one row, and this one lands on \
                 several",
                self.target_non_null, self.target_distinct
            ));
        }
        Ok(())
    }

    /// The part of [`check`](Self::check) that is about the
    /// measurement being WELL-FORMED rather than about it clearing the
    /// bar: a known contract version, a fingerprint, a threshold in
    /// range, a describable sample, coherent counts.
    ///
    /// A quarantined hypothesis is exactly a well-formed measurement
    /// that does not clear the bar — zero matches out of 1,914 is a
    /// finding, not a malformed record — so the record demands this
    /// much of every proposal and the full check only of one that
    /// claims support.
    pub fn check_coherent(&self, where_: &str) -> Result<(), String> {
        if self.contract_version == 0 || self.contract_version > SUPPORT_CONTRACT_VERSION {
            return Err(format!(
                "{where_}: support.contractVersion {} is not one this build understands (1..={}); \
                 refusing to read evidence under the wrong contract",
                self.contract_version, SUPPORT_CONTRACT_VERSION
            ));
        }
        if self.fingerprint.trim().is_empty() {
            return Err(format!(
                "{where_}: support.fingerprint is empty — the measurement cannot be tied to \
                 the run that produced it"
            ));
        }
        if !(self.min_support > 0.0 && self.min_support <= 1.0) {
            return Err(format!(
                "{where_}: support.minSupport {} is not in (0, 1] — a threshold of zero admits \
                 every relationship, which is the same as having none",
                self.min_support
            ));
        }
        self.sampling.check(where_)?;
        if self.source_non_null > self.source_rows || self.matched_rows > self.source_non_null {
            return Err(format!(
                "{where_}: support counts are incoherent (rows {}, non-null {}, matched {})",
                self.source_rows, self.source_non_null, self.matched_rows
            ));
        }
        if self.target_non_null > self.target_rows || self.target_distinct > self.target_non_null {
            return Err(format!(
                "{where_}: target support counts are incoherent (rows {}, non-null {}, \
                 distinct {})",
                self.target_rows, self.target_non_null, self.target_distinct
            ));
        }
        Ok(())
    }
}

/// A normalization applied to BOTH sides of a join before matching.
///
/// The operator is executable, with exact PostgreSQL semantics, so the
/// export can reproduce the comparison the measurement made. It is not
/// a transformation of the source value into a target id: see
/// [`Normalization`] for why that distinction is the whole point.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationOp {
    /// `btrim(x)` — PostgreSQL's `trim(both from x)`.
    Trim,
    /// `lower(x)`.
    Lower,
    /// `lower(btrim(x))`, in that order.
    TrimLower,
    /// An explicit cast, e.g. a text column joined against a bigint key.
    Cast(CastTarget),
}

/// Casts the contract admits. Deliberately narrow: each one has
/// unambiguous PostgreSQL semantics and a total ordering that a chunked
/// read can rely on.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastTarget {
    /// `text`.
    Text,
    /// `bigint`.
    Bigint,
    /// `numeric`.
    Numeric,
    /// `uuid`.
    Uuid,
    /// `date`.
    Date,
    /// `timestamptz`.
    Timestamptz,
}

impl CastTarget {
    /// The type name, as PostgreSQL spells it.
    pub fn sql(self) -> &'static str {
        match self {
            CastTarget::Text => "text",
            CastTarget::Bigint => "bigint",
            CastTarget::Numeric => "numeric",
            CastTarget::Uuid => "uuid",
            CastTarget::Date => "date",
            CastTarget::Timestamptz => "timestamptz",
        }
    }
}

impl NormalizationOp {
    /// The operator applied to `expr`, as PostgreSQL text. `expr` must
    /// already be a safe expression (a quoted column reference).
    pub fn to_sql(self, expr: &str) -> String {
        match self {
            NormalizationOp::Trim => format!("btrim({expr})"),
            NormalizationOp::Lower => format!("lower({expr})"),
            NormalizationOp::TrimLower => format!("lower(btrim({expr}))"),
            NormalizationOp::Cast(t) => format!("({expr})::{}", t.sql()),
        }
    }

    /// The same operator applied in-process, for a value already read
    /// as text. Used to key the resolution map the export builds, so
    /// both sides of the comparison went through one implementation.
    pub fn apply(self, v: &str) -> String {
        match self {
            NormalizationOp::Trim => v.trim().to_string(),
            NormalizationOp::Lower => v.to_lowercase(),
            NormalizationOp::TrimLower => v.trim().to_lowercase(),
            // A cast's textual form is whatever the server rendered;
            // the SQL side does the casting, and both sides are read
            // back as text through the same cast.
            NormalizationOp::Cast(_) => v.to_string(),
        }
    }
}

/// Normalization declared on a relationship: what is applied to the
/// source key, and what is applied to the target key.
///
/// Two operators, not one, because "the source has trailing spaces" and
/// "the target is stored lowercased" are different facts. The export
/// applies each to its own side and then RESOLVES: it looks the
/// normalized source value up among the normalized target keys and uses
/// the identity of the row it found. It does not transform the source
/// string and assume the result names a target — that assumption
/// invents an id for every value that has no target row.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Normalization {
    /// Applied to the source key.
    pub source: NormalizationOp,
    /// Defaults to the same operator as the source side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<NormalizationOp>,
}

impl Normalization {
    /// The operator the target side is normalized with.
    pub fn target_op(&self) -> NormalizationOp {
        self.target.unwrap_or(self.source)
    }
}

// ---------------------------------------------------------------------
// The proposal record
// ---------------------------------------------------------------------

/// The stable identity every revision of one proposal shares.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelationshipProposalId(pub String);

/// The identity of ONE immutable revision of a proposal.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProposalRevisionId(pub String);

/// The source the run read, pinned by hash. A proposal is a claim about
/// one shape of one source at one moment; without the manifest it was
/// measured against, a later reader cannot tell whether the source has
/// moved underneath it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifestRef {
    /// The connection namespace the source is catalogued under.
    pub connection: String,
    /// The execution plan the run was pinned to (`planHash`). What the
    /// run was permitted to read, frozen.
    pub plan_hash: String,
    /// The catalog content hash the plan was built from: what the
    /// SOURCE looked like.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_hash: Option<String>,
    /// The security policy version the plan was made under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    /// That policy's hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
}

/// The run that produced a proposal, and the version of the loop that
/// ran. Two proposals from different loop versions are not the same
/// claim even when they name the same join.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalOrigin {
    /// The run id, as the loop stamps it.
    pub run_id: String,
    /// The reconnaissance loop's own version.
    pub recon_version: String,
    /// The source manifest the run read under.
    pub source_manifest: SourceManifestRef,
    /// The model consulted, when one was. A proposal a model suggested
    /// and a probe measured is still measured; the model is recorded so
    /// its suggestions can be graded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// That model's version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

/// The relationship being proposed, in the mapper's own terms
/// (PRODUCT-192's contract): which rows, which columns, and what is
/// applied to both sides before they are compared.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedRelation {
    /// The subject vertex type the edge would leave.
    pub subject_type: String,
    /// The predicate the edge would carry.
    pub predicate: String,
    /// The target vertex type the edge would reach.
    pub target_type: String,
    /// The source relation, spelled as the source spells it.
    pub source_relation: String,
    /// The source-side join columns, in order.
    pub source_key_columns: Vec<String>,
    /// The target relation.
    pub target_relation: String,
    /// The target-side join columns, in order.
    pub target_key_columns: Vec<String>,
    /// What is applied to each side before matching. Absent means the
    /// values are compared as they are stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization: Option<Normalization>,
}

/// One SQL probe the loop ran to measure a proposal, kept so the
/// measurement can be re-derived rather than believed.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeRef {
    /// What the probe measured (`matched_rows`, `target_distinct`).
    pub name: String,
    /// The statement AS EXECUTED, parameterized — never with customer
    /// values inlined. A probe is a question about a shape.
    pub statement: String,
    /// The SQL dialect the statement is written in.
    pub dialect: String,
    /// When it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ran_at: Option<DateTime<Utc>>,
    /// The evidence record holding what the probe returned, when the
    /// loop recorded one. Referenced, so it travels with the proposal
    /// and is closure-checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<EvidenceId>,
}

/// Who promoted a proposal, when, why, and the receipt that records it.
///
/// Every field is required. A promotion with no named reviewer is an
/// anonymous decision to materialize edges the measurement did not
/// support, and the receipt is an evidence record so that the decision
/// travels with the proposal and is closure-checked like any other
/// reference.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerReceipt {
    /// The reviewer's principal key.
    pub reviewer: String,
    /// When they decided.
    pub decided_at: DateTime<Utc>,
    /// Why, in their words.
    pub reason: String,
    /// The evidence record holding the receipt.
    pub receipt: EvidenceId,
}

/// Where a proposal stands.
///
/// Tagged, and promotion carries its receipt INSIDE the variant: there
/// is no way to spell "promoted" without naming a reviewer, a time, a
/// reason and a receipt record. That is the difference between a
/// trusted human action and a boolean a model can set.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Measured and held back: the measurement did not clear its own
    /// declared threshold. Not a failure to record — a finding.
    QuarantinedHypothesis {
        /// Why it is held back, in one line.
        reason: String,
    },
    /// The measurement clears its declared threshold on its own.
    Supported,
    /// A reviewer promoted it, on the record. The support need not
    /// clear the bar — promoting one that does is the whole point.
    PromotedByReviewer {
        /// Who decided, when, why, and the receipt.
        receipt: ReviewerReceipt,
    },
    /// The evidence killed it.
    Refuted {
        /// Why, in one line.
        reason: String,
    },
}

impl ProposalStatus {
    /// The wire tag, for messages and indexes.
    pub fn tag(&self) -> &'static str {
        match self {
            ProposalStatus::QuarantinedHypothesis { .. } => "quarantined_hypothesis",
            ProposalStatus::Supported => "supported",
            ProposalStatus::PromotedByReviewer { .. } => "promoted_by_reviewer",
            ProposalStatus::Refuted { .. } => "refuted",
        }
    }

    /// Would an exporter materialize edges from a proposal in this
    /// state? Supported by measurement, or promoted by a reviewer.
    /// Recording either NEVER publishes it into a mapping; this only
    /// says which states a publication may be built from at all.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            ProposalStatus::Supported | ProposalStatus::PromotedByReviewer { .. }
        )
    }
}

/// One immutable revision of a relationship proposal.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipProposal {
    /// This revision's id. Unique per record; never rewritten.
    pub id: ProposalRevisionId,
    /// The stable proposal id every revision shares.
    pub proposal_id: RelationshipProposalId,
    /// The revision this one supersedes; `None` for the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<ProposalRevisionId>,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Owning project.
    pub project_id: ProjectId,
    /// The run, the loop version and the source manifest it read.
    pub origin: ProposalOrigin,
    /// What is being proposed.
    pub relation: ProposedRelation,
    /// What was measured.
    pub support: RelationSupport,
    /// Where it stands.
    #[serde(flatten)]
    pub status: ProposalStatus,
    /// The findings the proposal was drawn from: evidence records
    /// holding what the loop saw. Referenced, so they travel with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<EvidenceId>,
    /// The SQL probes that measured it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<ProbeRef>,
    /// When this revision was authored.
    pub proposed_at: DateTime<Utc>,
    /// Who authored this revision, when known. Advisory — it is never
    /// what makes a promotion trusted; the receipt is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,
    /// Producer-specific extras. Free-form, never interpreted here.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
}

/// Everything a proposal points at that must exist wherever the
/// proposal does. The ONE list a closure checker, an exporter and an
/// importer read, so they cannot disagree about what closure means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProposalReferences {
    /// Evidence ids: findings, probe results, and the reviewer's
    /// receipt when one promoted it.
    pub evidence: Vec<String>,
    /// The revision this one supersedes.
    pub previous_revision: Option<String>,
}

impl RelationshipProposal {
    /// Structural validity, independent of any store.
    ///
    /// Beyond the empty-identity checks, two rules carry weight:
    ///
    /// * the measurement must be WELL-FORMED for every status — a
    ///   quarantined hypothesis is a real measurement that fell short,
    ///   not a malformed one;
    /// * `supported` must actually be supported. A proposal cannot
    ///   label itself supported when its own numbers do not clear its
    ///   own declared threshold; that is the label the grader reads.
    pub fn validate(&self) -> Result<(), CoreError> {
        let bad = |m: String| CoreError::InvalidInput(format!("relationship proposal: {m}"));
        if self.id.0.trim().is_empty() {
            return Err(bad("revision id is empty".into()));
        }
        let where_ = format!("`{}`", self.id.0);
        if self.proposal_id.0.trim().is_empty() {
            return Err(bad(format!("revision {where_} has an empty proposalId")));
        }
        if self.previous_revision_id.as_ref() == Some(&self.id) {
            return Err(bad(format!(
                "revision {where_} names itself as its previous revision"
            )));
        }
        let r = &self.relation;
        for (what, v) in [
            ("subjectType", &r.subject_type),
            ("predicate", &r.predicate),
            ("targetType", &r.target_type),
            ("sourceRelation", &r.source_relation),
            ("targetRelation", &r.target_relation),
        ] {
            if v.trim().is_empty() {
                return Err(bad(format!(
                    "revision {where_} has an empty relation.{what}"
                )));
            }
        }
        if r.source_key_columns.is_empty() || r.target_key_columns.is_empty() {
            return Err(bad(format!(
                "revision {where_} proposes a join with no key columns on one side \
                 (source {:?}, target {:?})",
                r.source_key_columns, r.target_key_columns
            )));
        }
        if r.source_key_columns.len() != r.target_key_columns.len() {
            return Err(bad(format!(
                "revision {where_} joins {} source column(s) to {} target column(s); a join \
                 compares one column to one column",
                r.source_key_columns.len(),
                r.target_key_columns.len()
            )));
        }
        if self.origin.run_id.trim().is_empty() || self.origin.recon_version.trim().is_empty() {
            return Err(bad(format!(
                "revision {where_} does not say which run or which loop version produced it"
            )));
        }
        if self.origin.source_manifest.plan_hash.trim().is_empty() {
            return Err(bad(format!(
                "revision {where_} does not name the source manifest it was measured against, \
                 so nobody can tell whether the source has moved since"
            )));
        }
        self.support
            .check_coherent(&where_)
            .map_err(|e| bad(format!("revision {e}")))?;
        for p in &self.probes {
            if p.statement.trim().is_empty() || p.name.trim().is_empty() {
                return Err(bad(format!(
                    "revision {where_} carries a probe with no name or no statement"
                )));
            }
        }
        match &self.status {
            ProposalStatus::Supported => {
                self.support.check(&where_).map_err(|e| {
                    bad(format!(
                        "revision {e}. A proposal is `supported` only when its own measurement \
                         clears its own declared threshold; one that does not is a \
                         `quarantined_hypothesis`, and saying otherwise is how an unsupported \
                         join reaches a mapping"
                    ))
                })?;
            }
            ProposalStatus::QuarantinedHypothesis { reason }
            | ProposalStatus::Refuted { reason } => {
                if reason.trim().is_empty() {
                    return Err(bad(format!(
                        "revision {where_} is {} with no stated reason",
                        self.status.tag()
                    )));
                }
            }
            ProposalStatus::PromotedByReviewer { receipt } => {
                if receipt.reviewer.trim().is_empty() {
                    return Err(bad(format!(
                        "revision {where_} is promoted with no named reviewer: promotion is a \
                         person's decision to act on a measurement, and an unattributed one is \
                         nobody's"
                    )));
                }
                if receipt.reason.trim().is_empty() {
                    return Err(bad(format!(
                        "revision {where_} is promoted with no stated reason"
                    )));
                }
                if receipt.receipt.0.trim().is_empty() {
                    return Err(bad(format!(
                        "revision {where_} is promoted with no receipt record"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Every id the proposal references, deduplicated.
    pub fn references(&self) -> ProposalReferences {
        let mut evidence: BTreeSet<String> = BTreeSet::new();
        for f in &self.findings {
            evidence.insert(f.0.clone());
        }
        for p in &self.probes {
            if let Some(e) = &p.evidence_id {
                evidence.insert(e.0.clone());
            }
        }
        if let ProposalStatus::PromotedByReviewer { receipt } = &self.status {
            evidence.insert(receipt.receipt.0.clone());
        }
        ProposalReferences {
            evidence: evidence.into_iter().collect(),
            previous_revision: self.previous_revision_id.as_ref().map(|r| r.0.clone()),
        }
    }

    /// Matched over non-null source rows, as a ratio. `None` when the
    /// denominator is empty.
    pub fn ratio(&self) -> Option<f64> {
        self.support.ratio()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn support() -> RelationSupport {
        RelationSupport {
            contract_version: 1,
            method: SupportMethod::JoinMatchScan,
            method_version: 1,
            source_rows: 1000,
            source_non_null: 900,
            matched_rows: 890,
            target_rows: 50,
            target_non_null: 50,
            target_distinct: 50,
            sampling: Sampling::full_scan(),
            fingerprint: "sha256:abc".into(),
            min_support: 0.95,
        }
    }

    fn proposal(status: ProposalStatus) -> RelationshipProposal {
        RelationshipProposal {
            id: ProposalRevisionId("prop_1@1".into()),
            proposal_id: RelationshipProposalId("prop_1".into()),
            previous_revision_id: None,
            tenant_id: TenantId(1),
            project_id: ProjectId(1),
            origin: ProposalOrigin {
                run_id: "run_7".into(),
                recon_version: "2.1".into(),
                source_manifest: SourceManifestRef {
                    connection: "acme".into(),
                    plan_hash: "f00d".into(),
                    catalog_hash: None,
                    policy_version: Some(3),
                    policy_hash: None,
                },
                model: None,
                model_version: None,
            },
            relation: ProposedRelation {
                subject_type: "Shop.Order".into(),
                predicate: "placedBy".into(),
                target_type: "Shop.Customer".into(),
                source_relation: "public.orders".into(),
                source_key_columns: vec!["customer_ref".into()],
                target_relation: "public.customers".into(),
                target_key_columns: vec!["code".into()],
                normalization: Some(Normalization {
                    source: NormalizationOp::TrimLower,
                    target: None,
                }),
            },
            support: support(),
            status,
            findings: vec![EvidenceId("ev_finding".into())],
            probes: vec![ProbeRef {
                name: "matched_rows".into(),
                statement: "SELECT count(*) FROM public.orders o JOIN public.customers c \
                            ON lower(btrim(o.customer_ref)) = lower(btrim(c.code))"
                    .into(),
                dialect: "postgres".into(),
                ran_at: None,
                evidence_id: Some(EvidenceId("ev_probe".into())),
            }],
            proposed_at: "2026-09-13T00:00:00Z".parse().unwrap(),
            author: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn a_proposal_cannot_call_itself_supported_when_its_own_numbers_fall_short() {
        proposal(ProposalStatus::Supported)
            .validate()
            .expect("a measurement that clears its bar is supported");

        // The live shape: 0 matches out of 1,914. Well-formed, and the
        // opposite of supported.
        let mut short = proposal(ProposalStatus::Supported);
        short.support.source_rows = 1914;
        short.support.source_non_null = 1914;
        short.support.matched_rows = 0;
        let err = short.validate().expect_err("zero matches is not support");
        assert!(err.to_string().contains("support is ZERO"), "{err}");
        assert!(
            err.to_string().contains("quarantined_hypothesis"),
            "the refusal names the state it should have carried: {err}"
        );

        // The same numbers, honestly labelled, are a valid record —
        // that is the point of the kind.
        let mut quarantined = short.clone();
        quarantined.status = ProposalStatus::QuarantinedHypothesis {
            reason: "0 of 1914 source rows match a target row".into(),
        };
        quarantined
            .validate()
            .expect("a quarantined hypothesis is a finding, not a malformed record");
        assert_eq!(quarantined.ratio(), Some(0.0));

        // …but not with no reason.
        let mut silent = quarantined.clone();
        silent.status = ProposalStatus::QuarantinedHypothesis {
            reason: "   ".into(),
        };
        assert!(silent.validate().is_err(), "a quarantine states its reason");
    }

    #[test]
    fn promotion_cannot_be_spelled_without_a_named_reviewer_and_a_receipt() {
        let promoted = ProposalStatus::PromotedByReviewer {
            receipt: ReviewerReceipt {
                reviewer: "user:dana".into(),
                decided_at: "2026-09-13T09:00:00Z".parse().unwrap(),
                reason: "the catalogue confirms the codes".into(),
                receipt: EvidenceId("ev_receipt".into()),
            },
        };
        // A promoted proposal whose measurement falls short is VALID:
        // promoting one that already clears the bar is not the point.
        let mut weak = proposal(promoted.clone());
        weak.support.matched_rows = 1;
        weak.validate()
            .expect("a reviewer may promote weak support");
        assert!(weak.status.is_actionable());
        // The receipt travels with it.
        assert!(weak
            .references()
            .evidence
            .contains(&"ev_receipt".to_string()));

        for (broken, want) in [
            (
                ReviewerReceipt {
                    reviewer: " ".into(),
                    decided_at: "2026-09-13T09:00:00Z".parse().unwrap(),
                    reason: "r".into(),
                    receipt: EvidenceId("ev".into()),
                },
                "no named reviewer",
            ),
            (
                ReviewerReceipt {
                    reviewer: "user:dana".into(),
                    decided_at: "2026-09-13T09:00:00Z".parse().unwrap(),
                    reason: "".into(),
                    receipt: EvidenceId("ev".into()),
                },
                "no stated reason",
            ),
            (
                ReviewerReceipt {
                    reviewer: "user:dana".into(),
                    decided_at: "2026-09-13T09:00:00Z".parse().unwrap(),
                    reason: "r".into(),
                    receipt: EvidenceId("".into()),
                },
                "no receipt record",
            ),
        ] {
            let p = proposal(ProposalStatus::PromotedByReviewer { receipt: broken });
            let err = p.validate().expect_err("incomplete promotion");
            assert!(err.to_string().contains(want), "{err}");
        }

        // And there is no boolean: the only way to say "promoted" is to
        // carry the receipt, so a document that omits it does not parse.
        let err = serde_json::from_value::<ProposalStatus>(
            serde_json::json!({"status": "promoted_by_reviewer"}),
        )
        .expect_err("a status with no receipt is not a promotion");
        assert!(err.to_string().contains("receipt"), "{err}");
    }

    #[test]
    fn references_cover_findings_probes_the_receipt_and_the_predecessor() {
        let mut p = proposal(ProposalStatus::PromotedByReviewer {
            receipt: ReviewerReceipt {
                reviewer: "user:dana".into(),
                decided_at: "2026-09-13T09:00:00Z".parse().unwrap(),
                reason: "confirmed".into(),
                receipt: EvidenceId("ev_receipt".into()),
            },
        });
        p.previous_revision_id = Some(ProposalRevisionId("prop_1@1".into()));
        p.id = ProposalRevisionId("prop_1@2".into());
        p.findings.push(EvidenceId("ev_second_finding".into()));
        let r = p.references();
        assert_eq!(
            r.evidence,
            ["ev_finding", "ev_probe", "ev_receipt", "ev_second_finding"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(r.previous_revision, Some("prop_1@1".to_string()));
    }

    #[test]
    fn the_wire_shape_is_camel_case_and_round_trips() {
        let p = proposal(ProposalStatus::QuarantinedHypothesis {
            reason: "0 of 1914".into(),
        });
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["proposalId"], "prop_1");
        assert_eq!(v["status"], "quarantined_hypothesis");
        assert_eq!(v["reason"], "0 of 1914");
        assert_eq!(v["origin"]["reconVersion"], "2.1");
        assert_eq!(v["origin"]["sourceManifest"]["planHash"], "f00d");
        assert_eq!(v["relation"]["sourceKeyColumns"][0], "customer_ref");
        assert_eq!(v["relation"]["normalization"]["source"], "trim_lower");
        assert_eq!(v["support"]["sourceNonNull"], 900);
        assert_eq!(v["support"]["minSupport"], 0.95);
        assert!(v.get("previousRevisionId").is_none(), "absent, not null");
        let back: RelationshipProposal = serde_json::from_value(v).unwrap();
        assert_eq!(back, p);
        // A cast operator keeps its target through the round trip.
        let n = Normalization {
            source: NormalizationOp::Cast(CastTarget::Bigint),
            target: Some(NormalizationOp::Lower),
        };
        let back: Normalization =
            serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        assert_eq!(back, n);
        assert_eq!(
            NormalizationOp::Cast(CastTarget::Bigint).to_sql("\"c\""),
            "(\"c\")::bigint"
        );
    }

    #[test]
    fn a_join_compares_one_column_to_one_column() {
        let mut p = proposal(ProposalStatus::Supported);
        p.relation.target_key_columns = vec!["a".into(), "b".into()];
        let err = p.validate().expect_err("uneven key");
        assert!(
            err.to_string().contains("one column to one column"),
            "{err}"
        );
        p.relation.source_key_columns.clear();
        p.relation.target_key_columns.clear();
        assert!(p.validate().is_err(), "a join needs key columns");
    }

    #[test]
    fn a_proposal_names_the_run_and_the_manifest_it_was_measured_against() {
        let mut p = proposal(ProposalStatus::Supported);
        p.origin.source_manifest.plan_hash = "  ".into();
        let err = p.validate().expect_err("no manifest");
        assert!(err.to_string().contains("source manifest"), "{err}");
        let mut p = proposal(ProposalStatus::Supported);
        p.origin.run_id = String::new();
        assert!(p.validate().is_err(), "a proposal names its run");
    }
}
