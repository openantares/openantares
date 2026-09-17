//! Immutable elected ontology revisions (`.ant` v0.7).
//!
//! An ontology revision records one reviewed semantic manifest and the exact
//! authority attestation that elected it. The manifest digest is the semantic
//! identity. The surrounding [`OntologyRevision`] envelope is written once;
//! replay may reproduce the same bytes but may never replace its publisher,
//! idempotency key, commit time, conditional-chain position, or vault closure.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CoreError, ProjectId, SchemaType, TenantId};

/// Digest encoding used by ontology manifests, requests, and exact content.
pub const ONTOLOGY_CANONICAL_ENCODING: &str = "antares-canonical-json-v1";

/// Internal conditional-revision namespace reserved for ontology heads.
pub const ONTOLOGY_REVISION_DOMAIN: &str = "ontology/v1";

/// Conditional chain name used inside the ontology revision domain.
pub const ONTOLOGY_CHAIN_ID: &str = "ontology";

/// Stable identity of one immutable elected ontology revision.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OntologyRevisionId(pub String);

/// Native record kinds an elected manifest may cite or publish.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyRecordKind {
    /// Graph vertex.
    Vertex,
    /// Graph edge.
    Edge,
    /// Source-bound observation.
    Observation,
    /// Source evidence.
    Evidence,
    /// Inferred belief version.
    Belief,
    /// Contradiction-case revision.
    ContradictionCase,
    /// Relationship-proposal revision.
    RelationshipProposal,
}

/// One exact native record and its canonical content digest.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRecordRef {
    /// Native plane containing the record.
    pub kind: OntologyRecordKind,
    /// Record identity within the project.
    pub id: String,
    /// SHA-256 in [`ONTOLOGY_CANONICAL_ENCODING`].
    pub content_sha256: String,
}

/// One immutable ontology revision reference.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRevisionRef {
    /// Revision identity (`orv1:<manifest digest>`).
    pub id: OntologyRevisionId,
    /// Canonical digest of the referenced manifest.
    pub manifest_sha256: String,
}

/// A vault revision and its elected ontology head at review time.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyVaultPin {
    /// Vault whose feed position is pinned.
    pub vault_id: String,
    /// Exact vault feed revision reviewed.
    pub vault_revision: u64,
    /// Elected head at that position. Absent only for a genesis chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ontology_revision: Option<OntologyRevisionRef>,
}

/// Typed definition of a predicate exposed by the elected ontology.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPredicateDefinition {
    /// Qualified subject type.
    pub subject_type: String,
    /// Predicate or relationship name.
    pub predicate: String,
    /// Qualified object type or canonical scalar type.
    pub object_type: String,
    /// Source field that supplies the value, when the definition is mapped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_field: Option<String>,
}

/// Typed mapping adopted by an ontology revision.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyMappingDefinition {
    /// Qualified source type.
    pub source_type: String,
    /// Field read from the source.
    pub source_field: String,
    /// Predicate the mapping produces.
    pub predicate: String,
    /// Qualified target type or canonical scalar type.
    pub target_type: String,
    /// Optional named, versioned transform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
}

/// Typed derived rule adopted by an ontology revision.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRuleDefinition {
    /// Stable rule name.
    pub rule_id: String,
    /// Qualified subject type produced or constrained by the rule.
    pub subject_type: String,
    /// Predicate produced or constrained by the rule.
    pub predicate: String,
    /// Versioned rule expression. Unsupported languages are rejected by the
    /// reasoning consumer rather than treated as opaque success.
    pub expression: String,
    /// Rule language and version, for example `kgdsl-expression/v1`.
    pub language: String,
}

/// One typed semantic item in an elected manifest.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OntologySemanticItem {
    /// A complete schema type declaration.
    SchemaType {
        /// Stable semantic key.
        key: String,
        /// Source/review revision naming these exact bytes.
        revision: String,
        /// Typed schema declaration.
        #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
        content: SchemaType,
        /// Canonical digest of `content`.
        #[serde(rename = "contentSha256")]
        content_sha256: String,
        /// Native records supporting this item.
        #[serde(default)]
        support: Vec<OntologyRecordRef>,
    },
    /// A predicate definition.
    PredicateDefinition {
        /// Stable semantic key.
        key: String,
        /// Source/review revision naming these exact bytes.
        revision: String,
        /// Typed predicate declaration.
        content: OntologyPredicateDefinition,
        /// Canonical digest of `content`.
        #[serde(rename = "contentSha256")]
        content_sha256: String,
        /// Native records supporting this item.
        #[serde(default)]
        support: Vec<OntologyRecordRef>,
    },
    /// A field-to-predicate mapping.
    MappingDefinition {
        /// Stable semantic key.
        key: String,
        /// Source/review revision naming these exact bytes.
        revision: String,
        /// Typed mapping declaration.
        content: OntologyMappingDefinition,
        /// Canonical digest of `content`.
        #[serde(rename = "contentSha256")]
        content_sha256: String,
        /// Native records supporting this item.
        #[serde(default)]
        support: Vec<OntologyRecordRef>,
    },
    /// A derived semantic rule.
    RuleDefinition {
        /// Stable semantic key.
        key: String,
        /// Source/review revision naming these exact bytes.
        revision: String,
        /// Typed rule declaration.
        content: OntologyRuleDefinition,
        /// Canonical digest of `content`.
        #[serde(rename = "contentSha256")]
        content_sha256: String,
        /// Native records supporting this item.
        #[serde(default)]
        support: Vec<OntologyRecordRef>,
    },
}

impl OntologySemanticItem {
    /// Stable snake-case semantic kind used on the wire.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::SchemaType { .. } => "schema_type",
            Self::PredicateDefinition { .. } => "predicate_definition",
            Self::MappingDefinition { .. } => "mapping_definition",
            Self::RuleDefinition { .. } => "rule_definition",
        }
    }

    /// Stable key shared by all semantic variants.
    pub fn key(&self) -> &str {
        match self {
            Self::SchemaType { key, .. }
            | Self::PredicateDefinition { key, .. }
            | Self::MappingDefinition { key, .. }
            | Self::RuleDefinition { key, .. } => key,
        }
    }

    /// Declared canonical digest of the typed content.
    pub fn content_sha256(&self) -> &str {
        match self {
            Self::SchemaType { content_sha256, .. }
            | Self::PredicateDefinition { content_sha256, .. }
            | Self::MappingDefinition { content_sha256, .. }
            | Self::RuleDefinition { content_sha256, .. } => content_sha256,
        }
    }

    /// Source/review revision naming these exact semantic bytes.
    pub fn revision(&self) -> &str {
        match self {
            Self::SchemaType { revision, .. }
            | Self::PredicateDefinition { revision, .. }
            | Self::MappingDefinition { revision, .. }
            | Self::RuleDefinition { revision, .. } => revision,
        }
    }

    /// Supporting native records.
    pub fn support(&self) -> &[OntologyRecordRef] {
        match self {
            Self::SchemaType { support, .. }
            | Self::PredicateDefinition { support, .. }
            | Self::MappingDefinition { support, .. }
            | Self::RuleDefinition { support, .. } => support,
        }
    }

    /// Typed semantic content, borrowed without converting through an
    /// untyped JSON number model.
    pub fn content(&self) -> OntologySemanticContent<'_> {
        match self {
            Self::SchemaType { content, .. } => OntologySemanticContent::SchemaType(content),
            Self::PredicateDefinition { content, .. } => {
                OntologySemanticContent::PredicateDefinition(content)
            }
            Self::MappingDefinition { content, .. } => {
                OntologySemanticContent::MappingDefinition(content)
            }
            Self::RuleDefinition { content, .. } => {
                OntologySemanticContent::RuleDefinition(content)
            }
        }
    }
}

/// Borrowed typed semantic content for byte-exact hashing.
#[derive(Serialize)]
#[serde(untagged)]
pub enum OntologySemanticContent<'a> {
    /// Schema type content.
    SchemaType(&'a SchemaType),
    /// Predicate content.
    PredicateDefinition(&'a OntologyPredicateDefinition),
    /// Mapping content.
    MappingDefinition(&'a OntologyMappingDefinition),
    /// Rule content.
    RuleDefinition(&'a OntologyRuleDefinition),
}

/// Disposition of one retained reviewed position.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyPositionDisposition {
    /// Adopted position.
    Accepted,
    /// Still viable but not elected.
    Competing,
    /// Reviewed and rejected without deleting its evidence.
    Rejected,
}

/// Reviewed positions retained by an election.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRetainedPosition {
    /// How the election treated these records.
    pub disposition: OntologyPositionDisposition,
    /// Exact native records retaining the position.
    pub records: Vec<OntologyRecordRef>,
}

/// Contributor attribution retained through upward publication.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyAttribution {
    /// Engine-authenticated contributor principal.
    pub principal: String,
    /// Native evidence written by that contributor.
    pub evidence: OntologyRecordRef,
}

/// Source-vault owner's explicit consent to publish one exact reviewed set.
///
/// The trusted executor obtains this consent outside the engine and retains
/// the complete source-authority evidence here. The engine rechecks every
/// duplicated binding against the material election subject; service read
/// membership alone is never treated as publication authority.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologySourceOwnerConsent {
    /// Consent shape version. Version 1 is defined here.
    pub consent_version: u32,
    /// Durable external consent identity.
    pub consent_id: String,
    /// Source owner named by the trusted authority evaluation.
    pub owner_principal: String,
    /// Exact material election subject the owner released.
    pub election_subject_sha256: String,
    /// Reviewed source position.
    pub source: OntologyVaultPin,
    /// Reviewed target position.
    pub target: OntologyVaultPin,
    /// Exact native records the owner released to the target.
    pub published_records: Vec<OntologyRecordRef>,
    /// Exact ontology revisions the owner released to the target.
    pub published_revision_refs: Vec<OntologyRevisionRef>,
    /// Complete externally evaluated source-owner consent bytes.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub consent: Value,
}

/// Complete external approval asserted by the authenticated executor.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyApprovalAttestation {
    /// Attestation shape version.
    pub attestation_version: u32,
    /// Machine principal that must match the authenticated caller.
    pub attester_principal: String,
    /// PRODUCT-208 proposal identity.
    pub proposal_id: String,
    /// Canonical digest of the material election subject.
    pub election_subject_sha256: String,
    /// Reviewed source vault.
    pub source_vault: String,
    /// Reviewed source vault revision.
    pub source_vault_revision: u64,
    /// Reviewed target vault.
    pub target_vault: String,
    /// Reviewed target vault revision.
    pub target_vault_revision: u64,
    /// PRODUCT-43 authority-policy version revalidated before the effect.
    pub authority_policy_revision: String,
    /// Canonical SHA-256 of that exact authority-policy document.
    pub authority_policy_sha256: String,
    /// Why the human approver was designated for this proposal.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub designation: Value,
    /// Exact grant/principal match selected by PRODUCT-43.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub matched_by: Value,
    /// Separately bound authorization from the source-vault owner to publish
    /// this exact source/target/ref set.
    pub source_owner_consent: OntologySourceOwnerConsent,
    /// Complete PRODUCT-208 Approval bytes. The engine validates required
    /// binding fields but does not replace the external authority service.
    pub approval: Value,
}

/// Digest plus complete authority attestation carried in the manifest.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyApprovalBinding {
    /// Canonical digest of [`Self::attestation`].
    pub content_sha256: String,
    /// Complete attestation.
    pub attestation: OntologyApprovalAttestation,
}

/// Exact reviewed semantic manifest.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRevisionManifest {
    /// Manifest contract version. Version 1 is defined here.
    pub contract_version: u32,
    /// Canonical digest of [`Self::election_subject`].
    pub election_subject_sha256: String,
    /// Reviewed source vault position.
    pub source: OntologyVaultPin,
    /// Reviewed target vault position.
    pub target: OntologyVaultPin,
    /// Immutable common ancestor. Absent only for genesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_base: Option<OntologyRevisionRef>,
    /// Lower-level ontology dependencies and their exact vault positions.
    #[serde(default)]
    pub dependencies: Vec<OntologyVaultPin>,
    /// Typed semantic definitions elected by this revision.
    pub semantic_items: Vec<OntologySemanticItem>,
    /// Native records explicitly disclosed to the target vault.
    #[serde(default)]
    pub published_records: Vec<OntologyRecordRef>,
    /// Ontology revisions explicitly disclosed to the target vault.
    #[serde(default)]
    pub published_revision_refs: Vec<OntologyRevisionRef>,
    /// Claims adopted by the election.
    #[serde(default)]
    pub accepted_claims: Vec<OntologyRecordRef>,
    /// Accepted, competing, and rejected positions retained after election.
    #[serde(default)]
    pub retained_positions: Vec<OntologyRetainedPosition>,
    /// Contributor attribution retained through publication.
    #[serde(default)]
    pub attribution: Vec<OntologyAttribution>,
    /// Current target head intentionally reversed by this revision.
    // Do not skip `None`: the approved canonical contract carries
    // `"reverses": null`, and null is a material typed JSON value.
    #[serde(default)]
    pub reverses: Option<OntologyRevisionId>,
    /// Complete reviewed authority attestation.
    pub approval: OntologyApprovalBinding,
}

impl OntologyRevisionManifest {
    /// The fields whose digest PRODUCT-208 reviews before approval is added.
    pub fn election_subject(&self) -> OntologyElectionSubject<'_> {
        OntologyElectionSubject {
            source: &self.source,
            target: &self.target,
            common_base: self.common_base.as_ref(),
            dependencies: &self.dependencies,
            semantic_items: &self.semantic_items,
            published_records: &self.published_records,
            published_revision_refs: &self.published_revision_refs,
            accepted_claims: &self.accepted_claims,
            retained_positions: &self.retained_positions,
            attribution: &self.attribution,
            reverses: self.reverses.as_ref(),
        }
    }

    /// Validate structural and closure rules that do not require storage or
    /// hashing. Exact digests and visibility are checked by the engine.
    pub fn validate_shape(&self) -> Result<(), CoreError> {
        if self.contract_version != 1 {
            return Err(invalid("contractVersion must be 1"));
        }
        require_sha256("electionSubjectSha256", &self.election_subject_sha256)?;
        require_nonempty("source.vaultId", &self.source.vault_id)?;
        require_nonempty("target.vaultId", &self.target.vault_id)?;
        if self.semantic_items.is_empty() {
            return Err(invalid("semanticItems must not be empty"));
        }
        if self.semantic_items.len() > 1_024 {
            return Err(invalid("semanticItems exceeds 1024"));
        }
        if self.dependencies.len() > 64 {
            return Err(invalid("dependencies exceeds 64"));
        }
        if self.published_records.len() + self.published_revision_refs.len() > 4_096 {
            return Err(invalid("published record/revision refs exceeds 4096"));
        }
        if self.retained_positions.len() + self.attribution.len() > 4_096 {
            return Err(invalid("retained positions/attributions exceeds 4096"));
        }

        let mut dependency_vaults = BTreeSet::new();
        for dependency in &self.dependencies {
            require_nonempty("dependencies.vaultId", &dependency.vault_id)?;
            if !dependency_vaults.insert(dependency.vault_id.as_str()) {
                return Err(invalid(format!(
                    "dependency vault `{}` appears more than once",
                    dependency.vault_id
                )));
            }
        }

        let mut semantic_keys = BTreeSet::new();
        for item in &self.semantic_items {
            require_nonempty("semanticItems.key", item.key())?;
            require_nonempty("semanticItems.revision", item.revision())?;
            require_sha256("semanticItems.contentSha256", item.content_sha256())?;
            validate_semantic_item_shape(item)?;
            if item.support().is_empty() {
                return Err(invalid(format!(
                    "semantic item `{}` must retain at least one native support record",
                    item.key()
                )));
            }
            validate_record_refs("semanticItems.support", item.support())?;
            if !semantic_keys.insert(item.key()) {
                return Err(invalid(format!(
                    "semantic item key `{}` appears more than once",
                    item.key()
                )));
            }
        }

        validate_record_refs("publishedRecords", &self.published_records)?;
        validate_revision_refs("publishedRevisionRefs", &self.published_revision_refs)?;
        validate_record_refs("acceptedClaims", &self.accepted_claims)?;
        let published: BTreeSet<_> = self.published_records.iter().cloned().collect();
        let published_revisions: BTreeSet<_> =
            self.published_revision_refs.iter().cloned().collect();

        for item in &self.semantic_items {
            for support in item.support() {
                require_published_record(&published, support, "semantic support")?;
            }
        }
        for claim in &self.accepted_claims {
            require_published_record(&published, claim, "accepted claim")?;
        }
        for position in &self.retained_positions {
            if position.records.is_empty() {
                return Err(invalid("retained position records must not be empty"));
            }
            validate_record_refs("retainedPositions.records", &position.records)?;
            for record in &position.records {
                require_published_record(&published, record, "retained position")?;
            }
        }
        for attribution in &self.attribution {
            require_nonempty("attribution.principal", &attribution.principal)?;
            if attribution.evidence.kind != OntologyRecordKind::Evidence {
                return Err(invalid("attribution.evidence must be an evidence record"));
            }
            require_published_record(&published, &attribution.evidence, "attribution")?;
        }

        self.approval
            .attestation
            .source_owner_consent
            .validate_shape()?;
        let owner_consent = &self.approval.attestation.source_owner_consent;
        if owner_consent.election_subject_sha256 != self.election_subject_sha256
            || owner_consent.source != self.source
            || owner_consent.target != self.target
            || owner_consent.published_records != self.published_records
            || owner_consent.published_revision_refs != self.published_revision_refs
        {
            return Err(invalid(
                "source-owner consent does not bind the exact election subject, vault pins, and published refs",
            ));
        }

        for required in self
            .source
            .ontology_revision
            .iter()
            .chain(self.common_base.iter())
            .chain(
                self.dependencies
                    .iter()
                    .filter_map(|pin| pin.ontology_revision.as_ref()),
            )
        {
            validate_revision_ref(required)?;
            if !published_revisions.contains(required) {
                return Err(invalid(format!(
                    "required ontology revision `{}` is not in publishedRevisionRefs",
                    required.id.0
                )));
            }
        }
        if self.common_base.is_some() != self.target.ontology_revision.is_some() {
            return Err(invalid(
                "commonBase must be present exactly when the reviewed target has an ontology head",
            ));
        }
        if let Some(target_head) = &self.target.ontology_revision {
            validate_revision_ref(target_head)?;
            if self
                .reverses
                .as_ref()
                .is_some_and(|id| id != &target_head.id)
            {
                return Err(invalid(
                    "reverses must equal the reviewed target ontology head",
                ));
            }
        } else if self.reverses.is_some() {
            return Err(invalid("a genesis target cannot reverse a prior head"));
        }

        require_sha256("approval.contentSha256", &self.approval.content_sha256)?;
        self.approval.attestation.validate_shape()?;
        if self.approval.attestation.election_subject_sha256 != self.election_subject_sha256 {
            return Err(invalid(
                "approval electionSubjectSha256 does not match the manifest",
            ));
        }
        if self.approval.attestation.source_vault != self.source.vault_id
            || self.approval.attestation.source_vault_revision != self.source.vault_revision
            || self.approval.attestation.target_vault != self.target.vault_id
            || self.approval.attestation.target_vault_revision != self.target.vault_revision
        {
            return Err(invalid(
                "approval source/target pins do not match the manifest",
            ));
        }
        Ok(())
    }
}

impl OntologyApprovalAttestation {
    /// Validate required external approval fields without claiming to be the
    /// PRODUCT-208/43 authority service.
    pub fn validate_shape(&self) -> Result<(), CoreError> {
        if self.attestation_version != 1 {
            return Err(invalid("attestationVersion must be 1"));
        }
        require_nonempty("attesterPrincipal", &self.attester_principal)?;
        require_nonempty("proposalId", &self.proposal_id)?;
        require_sha256("electionSubjectSha256", &self.election_subject_sha256)?;
        require_nonempty("authorityPolicyRevision", &self.authority_policy_revision)?;
        require_sha256("authorityPolicySha256", &self.authority_policy_sha256)?;
        if !self.designation.is_object() {
            return Err(invalid("designation must be an object"));
        }
        if !self.matched_by.is_object() {
            return Err(invalid("matchedBy must be an object"));
        }
        for pointer in [
            "/reviewId",
            "/revision",
            "/materialFingerprint",
            "/reviewer",
            "/grantId",
            "/authorized",
            "/binding",
            "/identity",
            "/policyRevision/version",
            "/policyRevision/sha256",
            "/designation",
            "/at",
            "/valid",
            "/binding/executor",
        ] {
            if self.approval.pointer(pointer).is_none() {
                return Err(invalid(format!("approval is missing `{pointer}`")));
            }
        }
        if self.approval.pointer("/valid").and_then(Value::as_bool) != Some(true) {
            return Err(invalid("approval.valid must be true"));
        }
        if self
            .approval
            .pointer("/policyRevision/version")
            .and_then(Value::as_str)
            != Some(self.authority_policy_revision.as_str())
            || self
                .approval
                .pointer("/policyRevision/sha256")
                .and_then(Value::as_str)
                != Some(self.authority_policy_sha256.as_str())
            || self.approval.pointer("/designation") != Some(&self.designation)
        {
            return Err(invalid(
                "approval authority policy or designation does not match the durable attestation",
            ));
        }
        Ok(())
    }

    /// Executor principal bound by the external approval.
    pub fn executor(&self) -> Option<&str> {
        self.approval
            .pointer("/binding/executor")
            .and_then(Value::as_str)
    }
}

impl OntologySourceOwnerConsent {
    /// Validate the complete external consent and its duplicated stable
    /// bindings. Manifest equality is checked by
    /// [`OntologyRevisionManifest::validate_shape`].
    pub fn validate_shape(&self) -> Result<(), CoreError> {
        if self.consent_version != 1 {
            return Err(invalid("sourceOwnerConsent.consentVersion must be 1"));
        }
        require_nonempty("sourceOwnerConsent.consentId", &self.consent_id)?;
        require_nonempty("sourceOwnerConsent.ownerPrincipal", &self.owner_principal)?;
        require_sha256(
            "sourceOwnerConsent.electionSubjectSha256",
            &self.election_subject_sha256,
        )?;
        require_nonempty("sourceOwnerConsent.source.vaultId", &self.source.vault_id)?;
        require_nonempty("sourceOwnerConsent.target.vaultId", &self.target.vault_id)?;
        validate_record_refs(
            "sourceOwnerConsent.publishedRecords",
            &self.published_records,
        )?;
        validate_revision_refs(
            "sourceOwnerConsent.publishedRevisionRefs",
            &self.published_revision_refs,
        )?;
        for pointer in [
            "/consentId",
            "/sourceOwner",
            "/electionSubjectSha256",
            "/source/vaultId",
            "/source/vaultRevision",
            "/target/vaultId",
            "/target/vaultRevision",
            "/publishedRecords",
            "/publishedRevisionRefs",
            "/at",
            "/valid",
        ] {
            if self.consent.pointer(pointer).is_none() {
                return Err(invalid(format!(
                    "source-owner consent is missing `{pointer}`"
                )));
            }
        }
        let published_records = serde_json::to_value(&self.published_records)
            .map_err(|error| invalid(format!("invalid source-owner record binding: {error}")))?;
        let published_revision_refs = serde_json::to_value(&self.published_revision_refs)
            .map_err(|error| invalid(format!("invalid source-owner revision binding: {error}")))?;
        if self.consent.pointer("/valid").and_then(Value::as_bool) != Some(true)
            || self.consent.pointer("/consentId").and_then(Value::as_str)
                != Some(self.consent_id.as_str())
            || self.consent.pointer("/sourceOwner").and_then(Value::as_str)
                != Some(self.owner_principal.as_str())
            || self
                .consent
                .pointer("/electionSubjectSha256")
                .and_then(Value::as_str)
                != Some(self.election_subject_sha256.as_str())
            || self
                .consent
                .pointer("/source/vaultId")
                .and_then(Value::as_str)
                != Some(self.source.vault_id.as_str())
            || self
                .consent
                .pointer("/source/vaultRevision")
                .and_then(Value::as_u64)
                != Some(self.source.vault_revision)
            || self
                .consent
                .pointer("/target/vaultId")
                .and_then(Value::as_str)
                != Some(self.target.vault_id.as_str())
            || self
                .consent
                .pointer("/target/vaultRevision")
                .and_then(Value::as_u64)
                != Some(self.target.vault_revision)
            || self.consent.pointer("/publishedRecords") != Some(&published_records)
            || self.consent.pointer("/publishedRevisionRefs") != Some(&published_revision_refs)
        {
            return Err(invalid(
                "source-owner consent evidence does not match its durable binding",
            ));
        }
        Ok(())
    }
}

/// Borrowed material election subject, excluding the later approval.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyElectionSubject<'a> {
    /// Source pin.
    pub source: &'a OntologyVaultPin,
    /// Target pin.
    pub target: &'a OntologyVaultPin,
    /// Common ancestor.
    pub common_base: Option<&'a OntologyRevisionRef>,
    /// Dependency pins.
    pub dependencies: &'a [OntologyVaultPin],
    /// Typed semantics.
    pub semantic_items: &'a [OntologySemanticItem],
    /// Explicit record publication.
    pub published_records: &'a [OntologyRecordRef],
    /// Explicit revision publication.
    pub published_revision_refs: &'a [OntologyRevisionRef],
    /// Adopted claims.
    pub accepted_claims: &'a [OntologyRecordRef],
    /// Retained positions.
    pub retained_positions: &'a [OntologyRetainedPosition],
    /// Contributor attribution.
    pub attribution: &'a [OntologyAttribution],
    /// Reversed head.
    pub reverses: Option<&'a OntologyRevisionId>,
}

/// Authenticated publisher stamped by the engine on first commit.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyPublisherStamp {
    /// Resolved machine principal.
    pub principal: String,
    /// Engine-native token used for the first commit.
    pub token_id: String,
    /// Stable credential subject type (`service`).
    pub subject_type: String,
}

/// Conditional-chain position committed atomically with the revision.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyConditionalPosition {
    /// Engine-assigned domain. Must equal [`ONTOLOGY_REVISION_DOMAIN`].
    pub revision_domain: String,
    /// Chain name. Must equal [`ONTOLOGY_CHAIN_ID`].
    pub chain_id: String,
    /// This committed revision.
    pub revision_id: OntologyRevisionId,
    /// Previous target head, absent for genesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<OntologyRevisionId>,
    /// Whether a guarded chain explicitly adopted pre-existing unguarded data.
    pub initialized_from_existing: bool,
}

/// One immutable elected ontology revision envelope.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyRevision {
    /// Semantic identity (`orv1:<manifest digest>`).
    pub id: OntologyRevisionId,
    /// Tenant containing the record.
    pub tenant_id: TenantId,
    /// Project containing the record.
    pub project_id: ProjectId,
    /// Canonical manifest digest.
    pub manifest_sha256: String,
    /// Exact reviewed manifest.
    pub manifest: OntologyRevisionManifest,
    /// Resolved first publisher credential.
    pub publisher: OntologyPublisherStamp,
    /// First committed request key.
    pub request_id: String,
    /// Canonical digest of the first request.
    pub request_sha256: String,
    /// Engine commit time for the first envelope.
    pub committed_at: DateTime<Utc>,
    /// Atomic conditional-head position.
    pub conditional: OntologyConditionalPosition,
}

impl OntologyRevision {
    /// Validate envelope identity and non-hash structural rules.
    pub fn validate_shape(&self) -> Result<(), CoreError> {
        self.manifest.validate_shape()?;
        require_sha256("manifestSha256", &self.manifest_sha256)?;
        require_sha256("requestSha256", &self.request_sha256)?;
        require_nonempty("publisher.principal", &self.publisher.principal)?;
        require_nonempty("publisher.tokenId", &self.publisher.token_id)?;
        require_nonempty("requestId", &self.request_id)?;
        if self.id.0 != format!("orv1:{}", self.manifest_sha256) {
            return Err(invalid(
                "ontology revision id does not match its declared manifest digest",
            ));
        }
        if self.publisher.subject_type != "service" {
            return Err(invalid("publisher.subjectType must be `service`"));
        }
        if self.conditional.revision_domain != ONTOLOGY_REVISION_DOMAIN
            || self.conditional.chain_id != ONTOLOGY_CHAIN_ID
            || self.conditional.revision_id != self.id
        {
            return Err(invalid(
                "conditional ontology domain/chain/revision mismatch",
            ));
        }
        if self.conditional.previous_revision_id
            != self
                .manifest
                .target
                .ontology_revision
                .as_ref()
                .map(|revision| revision.id.clone())
        {
            return Err(invalid(
                "conditional previous revision does not match the reviewed target head",
            ));
        }
        if self.conditional.initialized_from_existing {
            return Err(invalid(
                "ontology revisions cannot initialize from an unguarded predecessor",
            ));
        }
        Ok(())
    }
}

fn validate_semantic_item_shape(item: &OntologySemanticItem) -> Result<(), CoreError> {
    match item {
        OntologySemanticItem::SchemaType { key, content, .. } => {
            require_nonempty("schemaType.name", &content.name.0)?;
            if key != &content.name.0 {
                return Err(invalid(
                    "a schema_type semantic key must equal the declared schema type name",
                ));
            }
            let mut properties = BTreeSet::new();
            for property in &content.properties {
                require_nonempty("schemaType.properties.name", &property.name)?;
                if !properties.insert(property.name.as_str()) {
                    return Err(invalid(format!(
                        "schema type `{}` declares property `{}` more than once",
                        content.name.0, property.name
                    )));
                }
            }
            let mut relations = BTreeSet::new();
            for relation in &content.relations {
                require_nonempty("schemaType.relations.name", &relation.name)?;
                require_nonempty("schemaType.relations.target", &relation.target.0)?;
                if !relations.insert(relation.name.as_str()) {
                    return Err(invalid(format!(
                        "schema type `{}` declares relationship `{}` more than once",
                        content.name.0, relation.name
                    )));
                }
                let mut edge_properties = BTreeSet::new();
                for property in &relation.properties {
                    require_nonempty("schemaType.relations.properties.name", &property.name)?;
                    if !edge_properties.insert(property.name.as_str()) {
                        return Err(invalid(format!(
                            "relationship `{}.{}` declares edge property `{}` more than once",
                            content.name.0, relation.name, property.name
                        )));
                    }
                }
            }
        }
        OntologySemanticItem::PredicateDefinition { content, .. } => {
            require_nonempty("predicateDefinition.subjectType", &content.subject_type)?;
            require_nonempty("predicateDefinition.predicate", &content.predicate)?;
            require_nonempty("predicateDefinition.objectType", &content.object_type)?;
            if let Some(source_field) = &content.source_field {
                require_nonempty("predicateDefinition.sourceField", source_field)?;
            }
        }
        OntologySemanticItem::MappingDefinition { content, .. } => {
            require_nonempty("mappingDefinition.sourceType", &content.source_type)?;
            require_nonempty("mappingDefinition.sourceField", &content.source_field)?;
            require_nonempty("mappingDefinition.predicate", &content.predicate)?;
            require_nonempty("mappingDefinition.targetType", &content.target_type)?;
            if let Some(transform) = &content.transform {
                require_nonempty("mappingDefinition.transform", transform)?;
            }
        }
        OntologySemanticItem::RuleDefinition { content, .. } => {
            require_nonempty("ruleDefinition.ruleId", &content.rule_id)?;
            require_nonempty("ruleDefinition.subjectType", &content.subject_type)?;
            require_nonempty("ruleDefinition.predicate", &content.predicate)?;
            require_nonempty("ruleDefinition.expression", &content.expression)?;
            require_nonempty("ruleDefinition.language", &content.language)?;
        }
    }
    Ok(())
}

fn validate_record_refs(name: &str, records: &[OntologyRecordRef]) -> Result<(), CoreError> {
    let mut seen = BTreeSet::new();
    for record in records {
        require_nonempty(&format!("{name}.id"), &record.id)?;
        require_sha256(&format!("{name}.contentSha256"), &record.content_sha256)?;
        if !seen.insert((record.kind, record.id.as_str())) {
            return Err(invalid(format!(
                "{name} contains duplicate identity `{:?}:{}`",
                record.kind, record.id
            )));
        }
    }
    Ok(())
}

fn validate_revision_refs(name: &str, revisions: &[OntologyRevisionRef]) -> Result<(), CoreError> {
    let mut seen = BTreeSet::new();
    for revision in revisions {
        validate_revision_ref(revision)?;
        if !seen.insert(revision) {
            return Err(invalid(format!("{name} contains a duplicate revision")));
        }
    }
    Ok(())
}

fn validate_revision_ref(revision: &OntologyRevisionRef) -> Result<(), CoreError> {
    require_sha256(
        "ontology revision manifestSha256",
        &revision.manifest_sha256,
    )?;
    if revision.id.0 != format!("orv1:{}", revision.manifest_sha256) {
        return Err(invalid(format!(
            "ontology revision id `{}` does not match its manifest digest",
            revision.id.0
        )));
    }
    Ok(())
}

fn require_published_record(
    published: &BTreeSet<OntologyRecordRef>,
    record: &OntologyRecordRef,
    context: &str,
) -> Result<(), CoreError> {
    if published.contains(record) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{context} `{:?}:{}` is not in publishedRecords",
            record.kind, record.id
        )))
    }
}

fn require_nonempty(name: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(invalid(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}

fn require_sha256(name: &str, value: &str) -> Result<(), CoreError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{name} must be 64 lowercase hex characters"
        )))
    }
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::InvalidInput(message.into())
}
