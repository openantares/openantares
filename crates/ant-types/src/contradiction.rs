//! Contradiction cases (`.ant` v0.4): first-class records of a
//! comparison between two or more exact claim revisions.
//!
//! A case is KNOWLEDGE, not a log line: it exports, imports, syncs to
//! followers and verifies like a belief. What it carries is REFERENCES
//! — to belief versions, observations and evidence positions, to the
//! comparator and snapshot that produced it, to receipts and vault
//! occurrences — and never a copy of their content. That is what makes
//! closure enforceable: an archive holding a case must hold every
//! revision the case compares, and [`ContradictionCase::references`]
//! is the one list a checker, an exporter and an importer all read.
//!
//! Three state families are kept apart because they move for different
//! reasons: what the evidence says ([`EpistemicState`]), what it would
//! cost if true ([`BusinessImpact`]), and where the work stands
//! ([`WorkflowState`]). Folding them into one status is how a case that
//! is "settled" quietly stops being read as incompatible.
//!
//! Revisions are immutable. A change is a NEW record with a new `id`,
//! the same `case_id`, and `previous_revision_id` naming what it
//! supersedes; nothing is edited in place. A second vault occurrence is
//! a new revision of the same case carrying one more reference, not a
//! second case.
//!
//! Wire casing is camelCase throughout (`caseId`, `previousRevisionId`),
//! on the HTTP surface and inside the `.ant` payload alike — one shape,
//! so a captured mutation IS the committed record.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::author::AuthorStamp;
use crate::error::CoreError;
use crate::evidence::EvidenceId;
use crate::ids::{ProjectId, TenantId};

/// The stable identity every revision of one case shares.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContradictionCaseId(pub String);

/// The identity of ONE immutable revision of a case. Unique per record.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaseRevisionId(pub String);

/// What the compared evidence says. Independent of impact and workflow.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicState {
    /// The claims cannot both hold.
    Incompatible,
    /// The claims hold together; the case is a non-case on the merits.
    Compatible,
    /// Not decidable on the material at hand.
    Uncertain,
    /// The claims are not about the same thing closely enough to
    /// compare — the shape a model-invented shared subject takes.
    InsufficientlyComparable,
}

/// What it would cost if the incompatibility is real. Independent of
/// the epistemic answer: a harmful case can be uncertain, a settled
/// incompatibility can be alignment-only.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusinessImpact {
    /// Acting on the wrong claim would cause harm.
    Harmful,
    /// Only alignment between sources is at stake.
    AlignmentOnly,
    /// Nobody has assessed it yet.
    Unassessed,
}

/// Where the work stands. Independent of the other two families.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    /// Raised, nobody has acted.
    Open,
    /// A question is out to a source or an author.
    AwaitingClarification,
    /// Ready for a reviewer.
    AwaitingReview,
    /// Reviewers disagree.
    Contested,
    /// Parked on purpose.
    Deferred,
    /// Closed with a resolution.
    Settled,
    /// Settled once, then reopened by a later revision.
    Reopened,
}

/// Which plane a compared claim lives on.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// A belief version (`belief` record; `version` pins it).
    Belief,
    /// An observation (`observation` record).
    Observation,
    /// A stretch of source material (`evidence` record plus pointer).
    Evidence,
}

/// A position inside an evidence record's content. Every field is
/// optional so a pointer can name a character span, a byte span, a
/// JSON path into structured content, or any combination.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePointer {
    /// First character (inclusive) of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_start: Option<u64>,
    /// End character (exclusive) of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_end: Option<u64>,
    /// First byte (inclusive) of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_start: Option<u64>,
    /// End byte (exclusive) of the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_end: Option<u64>,
    /// JSON pointer (RFC 6901) into structured content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// One exact claim revision the case compares.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimRef {
    /// The plane the claim lives on.
    pub kind: ClaimKind,
    /// The record id on that plane.
    pub id: String,
    /// The belief version, when `kind` is `belief`. A belief id is
    /// unique per version already; the version is carried so a reader
    /// can see WHICH version was compared without resolving the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    /// Where in the source the claim sits, when `kind` is `evidence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<SourcePointer>,
}

/// A source position the case relies on beyond the claims themselves.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    /// The evidence record.
    pub evidence_id: EvidenceId,
    /// Where inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<SourcePointer>,
}

/// A measured value the comparator used — the two numbers compared, a
/// distance, a tolerance — referenced back to where it was read.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementRef {
    /// What was measured (`amount_a`, `distance`, `tolerance`).
    pub name: String,
    /// The value, as JSON.
    pub value: Value,
    /// Unit, when the value has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The evidence the measurement was read from, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<EvidenceId>,
    /// Where inside that evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<SourcePointer>,
}

/// Who or what produced this comparison, pinned to versions and to the
/// snapshot it ran against, so the same question can be re-asked
/// against the same inputs.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparatorIdentity {
    /// The comparator (`numeric_tolerance`, `date_overlap`, …).
    pub comparator: String,
    /// Its version.
    pub comparator_version: String,
    /// The rule it applied, when a rule drove it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    /// That rule's version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_version: Option<String>,
    /// The model consulted, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// That model's version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    /// The snapshot of the world the comparison ran against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
}

/// How a piece of material relates to the sources already in play. A
/// forwarded copy of a source is NOT an independent witness to it;
/// counting it as one is how two sources become "confirmed by three".
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceDependency {
    /// Its own witness.
    Independent,
    /// A forwarded copy of another evidence record.
    ForwardedCopy {
        /// The evidence it copies.
        of: EvidenceId,
    },
    /// Derived from another evidence record (a summary, an extraction).
    Derived {
        /// The evidence it derives from.
        of: EvidenceId,
    },
}

/// Material that supports or refutes the incompatibility.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// The evidence record.
    pub evidence_id: EvidenceId,
    /// Where inside it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<SourcePointer>,
    /// Whether it stands on its own.
    pub dependency: SourceDependency,
}

/// Where the compared claims occur in a vault, and under what
/// conditions the occurrence applies. A reference: the vault item is
/// not copied here.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultOccurrence {
    /// The vault.
    pub vault_id: String,
    /// The item inside it.
    pub item_id: String,
    /// The item revision, when the vault versions items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Where inside the item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<SourcePointer>,
    /// Conditions under which the occurrence applies (a jurisdiction,
    /// a date range, a product version), as the vault states them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
}

/// One immutable revision of a contradiction case.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContradictionCase {
    /// This revision's id. Unique per record; never rewritten.
    pub id: CaseRevisionId,
    /// The stable case id every revision shares.
    pub case_id: ContradictionCaseId,
    /// The revision this one supersedes; `None` for the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<CaseRevisionId>,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Owning project.
    pub project_id: ProjectId,
    /// The comparison family the case belongs to: the kind of question
    /// being asked (`same_subject_numeric`, `date_overlap`, …). Cases
    /// are compared within a family, never across.
    pub family: String,
    /// Two or more exact claim revisions compared.
    pub claims: Vec<ClaimRef>,
    /// Source positions relied on beyond the claims.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRef>,
    /// Measured values the comparator used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurements: Vec<MeasurementRef>,
    /// What produced the comparison, and against which snapshot.
    pub comparator: ComparatorIdentity,
    /// Material supporting the incompatibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting: Vec<Material>,
    /// Material refuting it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refuting: Vec<Material>,
    /// Where the claims occur in a vault.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_occurrences: Vec<VaultOccurrence>,
    /// What the evidence says.
    pub epistemic: EpistemicState,
    /// What it would cost.
    pub impact: BusinessImpact,
    /// Where the work stands.
    pub workflow: WorkflowState,
    /// The proposal this case is linked to, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    /// Review receipts: evidence records recording who reviewed what
    /// and decided how. Referenced, so they travel with the case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_receipts: Vec<EvidenceId>,
    /// When this revision was authored.
    pub revised_at: DateTime<Utc>,
    /// Who authored this revision, when known. Advisory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,
    /// Producer-specific extras. Free-form, never interpreted here.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
}

/// Everything a case points at that must exist wherever the case does.
/// The ONE list a closure checker, an exporter and an importer read,
/// so they cannot disagree about what closure means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaseReferences {
    /// Belief revisions as `(id, version)`; `None` pins the id only.
    pub beliefs: Vec<(String, Option<u64>)>,
    /// Observation ids.
    pub observations: Vec<String>,
    /// Evidence ids: claims, positions, measurements, material, the
    /// sources material depends on, and review receipts.
    pub evidence: Vec<String>,
    /// The revision this one supersedes.
    pub previous_revision: Option<String>,
}

impl ContradictionCase {
    /// Structural validity, independent of any store: at least two
    /// claims, non-empty identities, and a revision that does not name
    /// itself as its own predecessor.
    pub fn validate(&self) -> Result<(), CoreError> {
        let bad = |m: String| CoreError::InvalidInput(format!("contradiction case: {m}"));
        if self.id.0.trim().is_empty() {
            return Err(bad("revision id is empty".into()));
        }
        if self.case_id.0.trim().is_empty() {
            return Err(bad(format!("revision `{}` has an empty caseId", self.id.0)));
        }
        if self.claims.len() < 2 {
            return Err(bad(format!(
                "revision `{}` compares {} claim(s); a case compares two or more",
                self.id.0,
                self.claims.len()
            )));
        }
        if let Some(c) = self.claims.iter().find(|c| c.id.trim().is_empty()) {
            return Err(bad(format!(
                "revision `{}` has a {:?} claim with an empty id",
                self.id.0, c.kind
            )));
        }
        if self.previous_revision_id.as_ref() == Some(&self.id) {
            return Err(bad(format!(
                "revision `{}` names itself as its previous revision",
                self.id.0
            )));
        }
        if self.family.trim().is_empty() {
            return Err(bad(format!("revision `{}` has an empty family", self.id.0)));
        }
        Ok(())
    }

    /// Every id the case references, deduplicated, by plane.
    pub fn references(&self) -> CaseReferences {
        let mut beliefs: BTreeSet<(String, Option<u64>)> = BTreeSet::new();
        let mut observations: BTreeSet<String> = BTreeSet::new();
        let mut evidence: BTreeSet<String> = BTreeSet::new();
        for c in &self.claims {
            match c.kind {
                ClaimKind::Belief => {
                    beliefs.insert((c.id.clone(), c.version));
                }
                ClaimKind::Observation => {
                    observations.insert(c.id.clone());
                }
                ClaimKind::Evidence => {
                    evidence.insert(c.id.clone());
                }
            }
        }
        for e in &self.evidence {
            evidence.insert(e.evidence_id.0.clone());
        }
        for m in &self.measurements {
            if let Some(e) = &m.evidence_id {
                evidence.insert(e.0.clone());
            }
        }
        for m in self.supporting.iter().chain(&self.refuting) {
            evidence.insert(m.evidence_id.0.clone());
            match &m.dependency {
                SourceDependency::Independent => {}
                SourceDependency::ForwardedCopy { of } | SourceDependency::Derived { of } => {
                    evidence.insert(of.0.clone());
                }
            }
        }
        for r in &self.review_receipts {
            evidence.insert(r.0.clone());
        }
        CaseReferences {
            beliefs: beliefs.into_iter().collect(),
            observations: observations.into_iter().collect(),
            evidence: evidence.into_iter().collect(),
            previous_revision: self.previous_revision_id.as_ref().map(|r| r.0.clone()),
        }
    }

    /// Supporting material that stands on its own. A forwarded copy or a
    /// derivation of a source already counted is not another witness.
    pub fn independent_witnesses(&self) -> usize {
        self.supporting
            .iter()
            .filter(|m| matches!(m.dependency, SourceDependency::Independent))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(kind: ClaimKind, id: &str) -> ClaimRef {
        ClaimRef {
            kind,
            id: id.into(),
            version: None,
            pointer: None,
        }
    }

    fn case(claims: Vec<ClaimRef>) -> ContradictionCase {
        ContradictionCase {
            id: CaseRevisionId("rev_1".into()),
            case_id: ContradictionCaseId("case_1".into()),
            previous_revision_id: None,
            tenant_id: TenantId(1),
            project_id: ProjectId(1),
            family: "same_subject_numeric".into(),
            claims,
            evidence: vec![],
            measurements: vec![],
            comparator: ComparatorIdentity {
                comparator: "numeric_tolerance".into(),
                comparator_version: "1.0".into(),
                rule_id: None,
                rule_version: None,
                model: None,
                model_version: None,
                snapshot_id: None,
            },
            supporting: vec![],
            refuting: vec![],
            vault_occurrences: vec![],
            epistemic: EpistemicState::Incompatible,
            impact: BusinessImpact::Unassessed,
            workflow: WorkflowState::Open,
            proposal_id: None,
            review_receipts: vec![],
            revised_at: "2026-09-10T00:00:00Z".parse().unwrap(),
            author: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn a_case_compares_two_or_more_claims() {
        let one = case(vec![claim(ClaimKind::Belief, "b1")]);
        let err = one.validate().expect_err("one claim is not a comparison");
        assert!(err.to_string().contains("two or more"), "{err}");
        let two = case(vec![
            claim(ClaimKind::Belief, "b1"),
            claim(ClaimKind::Observation, "o1"),
        ]);
        two.validate().expect("two claims compare");
        let mut selfref = two.clone();
        selfref.previous_revision_id = Some(selfref.id.clone());
        assert!(
            selfref.validate().is_err(),
            "a revision cannot supersede itself"
        );
    }

    #[test]
    fn references_cover_every_plane_the_case_points_at() {
        let mut c = case(vec![
            ClaimRef {
                kind: ClaimKind::Belief,
                id: "b1".into(),
                version: Some(3),
                pointer: None,
            },
            claim(ClaimKind::Observation, "o1"),
            claim(ClaimKind::Evidence, "e_claim"),
        ]);
        c.previous_revision_id = Some(CaseRevisionId("rev_0".into()));
        c.evidence.push(EvidenceRef {
            evidence_id: EvidenceId("e_pos".into()),
            pointer: None,
        });
        c.measurements.push(MeasurementRef {
            name: "amount".into(),
            value: serde_json::json!(42),
            unit: None,
            evidence_id: Some(EvidenceId("e_meas".into())),
            pointer: None,
        });
        c.supporting.push(Material {
            evidence_id: EvidenceId("e_sup".into()),
            pointer: None,
            dependency: SourceDependency::ForwardedCopy {
                of: EvidenceId("e_orig".into()),
            },
        });
        c.refuting.push(Material {
            evidence_id: EvidenceId("e_ref".into()),
            pointer: None,
            dependency: SourceDependency::Independent,
        });
        c.review_receipts.push(EvidenceId("e_receipt".into()));
        let r = c.references();
        assert_eq!(r.beliefs, vec![("b1".to_string(), Some(3))]);
        assert_eq!(r.observations, vec!["o1".to_string()]);
        assert_eq!(
            r.evidence,
            [
                "e_claim",
                "e_meas",
                "e_orig",
                "e_pos",
                "e_receipt",
                "e_ref",
                "e_sup"
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
            "claims, positions, measurements, material, its sources and receipts"
        );
        assert_eq!(r.previous_revision, Some("rev_0".to_string()));
        assert_eq!(
            c.independent_witnesses(),
            0,
            "a forwarded copy is not an independent witness"
        );
    }

    #[test]
    fn the_wire_shape_is_camel_case_and_round_trips() {
        let c = case(vec![
            claim(ClaimKind::Belief, "b1"),
            claim(ClaimKind::Belief, "b2"),
        ]);
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["caseId"], "case_1");
        assert_eq!(v["epistemic"], "incompatible");
        assert_eq!(v["impact"], "unassessed");
        assert_eq!(v["workflow"], "open");
        assert_eq!(v["comparator"]["comparatorVersion"], "1.0");
        assert!(v.get("previousRevisionId").is_none(), "absent, not null");
        let back: ContradictionCase = serde_json::from_value(v).unwrap();
        assert_eq!(back, c);
        let dep = serde_json::to_value(SourceDependency::ForwardedCopy {
            of: EvidenceId("e".into()),
        })
        .unwrap();
        assert_eq!(dep, serde_json::json!({"kind": "forwardedCopy", "of": "e"}));
    }
}
