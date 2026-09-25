//! Antares-native first-class evidence.
//!
//! Evidence is a kernel concept, not a user-defined entity. Every fact
//! (Edge) may reference one or more Evidence records via `evidenced_by`,
//! and derived records may accumulate evidence from their causes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::author::AuthorStamp;
use crate::ids::{ProjectId, TenantId};

/// Evidence identifier (newtype over String). Distinct from VertexId
/// because evidence lives in its own storage plane.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", schema(value_type = String))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceId(pub String);

/// The exact original a primary Evidence was cut from, stored outside the
/// record (format v1.0).
///
/// The record carries this small reference; the bytes live in the engine's
/// original store and travel beside the record wherever it goes (the feed
/// carries only the reference and each node transfers the bytes before
/// applying it; a blob-bearing `.ant` file carries them as `original_chunk`
/// records right after the Evidence). `asset_id` names the stored original
/// on every node and is never an authorization capability: reading it is
/// authorized by the Evidence's own visibility. An Evidence's original is
/// immutable — a changed file is a new Evidence id.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlob {
    /// Opaque identity of the stored original, `[A-Za-z0-9_-]{16,128}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_-]{16,128}$"))
    )]
    pub asset_id: String,
    /// Exact length of the original, in bytes. Zero is an empty file.
    pub byte_length: u64,
    /// SHA-256 of the original bytes, 64 lowercase hex characters.
    #[cfg_attr(feature = "schemars", schemars(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub sha256: String,
    /// Media type declared for the original: informational, never used to
    /// serve it. 1..=255 printable ASCII bytes.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[\x20-\x7E]{1,255}$"))
    )]
    pub media_type: String,
    /// Display name of the original, 1..=1024 bytes with no control
    /// characters. Never used as a filesystem path. The schema bounds
    /// characters; readers bound the bytes.
    #[cfg_attr(
        feature = "schemars",
        schemars(
            length(min = 1, max = 1024),
            regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$")
        )
    )]
    pub file_name: String,
}

impl SourceBlob {
    /// Longest accepted `asset_id`.
    pub const ASSET_ID_MAX: usize = 128;
    /// Shortest accepted `asset_id`.
    pub const ASSET_ID_MIN: usize = 16;

    /// Whether `id` is a well-formed asset id.
    pub fn is_valid_asset_id(id: &str) -> bool {
        (Self::ASSET_ID_MIN..=Self::ASSET_ID_MAX).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }

    /// Structural validity: the shape every reader and writer of the
    /// reference agrees on. Returns the first problem, naming the field.
    pub fn validate(&self) -> Result<(), String> {
        if !Self::is_valid_asset_id(&self.asset_id) {
            return Err(format!(
                "sourceBlob.assetId must be {}..={} characters of [A-Za-z0-9_-]",
                Self::ASSET_ID_MIN,
                Self::ASSET_ID_MAX
            ));
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("sourceBlob.sha256 must be 64 lowercase hex characters".into());
        }
        if self.media_type.is_empty()
            || self.media_type.len() > 255
            || !self.media_type.bytes().all(|b| (0x20..0x7f).contains(&b))
        {
            return Err("sourceBlob.mediaType must be 1..=255 printable ASCII bytes".into());
        }
        if self.file_name.is_empty()
            || self.file_name.len() > 1024
            || self.file_name.chars().any(char::is_control)
        {
            return Err(
                "sourceBlob.fileName must be 1..=1024 bytes with no control characters".into(),
            );
        }
        Ok(())
    }
}

/// Where an Evidence's stored original came from — a native folder or
/// picker, a drive item, a mail message (format v1.0). Any number bind to
/// one original; they are append-only and immutable, and appending one
/// never changes the Evidence record itself. The binding
/// (`evidence_id`, `asset_id`, `sha256`, `byte_length`) must equal that
/// Evidence's [`SourceBlob`]: a reference names exact bytes.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReference {
    /// The primary Evidence whose original this describes: non-empty, with
    /// no control characters.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$"))
    )]
    pub evidence_id: String,
    /// That Evidence's `sourceBlob.assetId`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_-]{16,128}$"))
    )]
    pub asset_id: String,
    /// That Evidence's `sourceBlob.sha256`.
    #[cfg_attr(feature = "schemars", schemars(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub sha256: String,
    /// That Evidence's `sourceBlob.byteLength`.
    pub byte_length: u64,
    /// Client-chosen, deterministic for one source observation, so a retry
    /// converges: `[A-Za-z0-9_.:-]{1,128}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))
    )]
    pub reference_id: String,
    /// Where the bytes came from: a JSON object of at most
    /// [`SourceReference::SOURCE_MAX_BYTES`] serialized, nesting at most
    /// [`SourceReference::SOURCE_MAX_DEPTH`] levels, opaque to the engine.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    #[cfg_attr(feature = "schemars", schemars(extend("type" = "object")))]
    pub source: serde_json::Value,
    /// When the reference was first recorded.
    pub recorded_at: DateTime<Utc>,
    /// Who first recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorStamp>,
}

impl SourceReference {
    /// Largest serialized `source` object.
    pub const SOURCE_MAX_BYTES: usize = 16 * 1024;
    /// Deepest `source`: arrays and objects nested, `source` itself one.
    /// Well inside the 127 levels a JSON parser accepts, so the reference
    /// still reads in every envelope that carries it (a feed frame, a
    /// batch push).
    pub const SOURCE_MAX_DEPTH: usize = 64;

    /// Whether `id` is a well-formed reference id.
    pub fn is_valid_reference_id(id: &str) -> bool {
        (1..=128).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
    }

    /// Structural validity. Returns the first problem, naming the field.
    pub fn validate(&self) -> Result<(), String> {
        if !Self::is_valid_reference_id(&self.reference_id) {
            return Err(
                "reference.referenceId must be 1..=128 characters of [A-Za-z0-9_.:-]".into(),
            );
        }
        if !self.source.is_object() {
            return Err("reference.source must be a JSON object".into());
        }
        if serde_json::to_vec(&self.source).map_or(usize::MAX, |b| b.len()) > Self::SOURCE_MAX_BYTES
        {
            return Err(format!(
                "reference.source exceeds {} bytes serialized",
                Self::SOURCE_MAX_BYTES
            ));
        }
        if nests_deeper(&self.source, Self::SOURCE_MAX_DEPTH) {
            return Err(format!(
                "reference.source nests deeper than {} levels",
                Self::SOURCE_MAX_DEPTH
            ));
        }
        if self.evidence_id.is_empty() || self.evidence_id.chars().any(char::is_control) {
            return Err("reference.evidenceId must be non-empty with no control characters".into());
        }
        if !SourceBlob::is_valid_asset_id(&self.asset_id)
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("reference.assetId/sha256 are malformed".into());
        }
        Ok(())
    }

    /// Whether this reference binds to exactly `blob` of Evidence `evidence`.
    pub fn binds(&self, evidence: &str, blob: &SourceBlob) -> bool {
        self.evidence_id == evidence
            && self.asset_id == blob.asset_id
            && self.sha256 == blob.sha256
            && self.byte_length == blob.byte_length
    }

    /// Whether `other` records the same reference: the same id, primary,
    /// binding and `source`. `author` and `recordedAt` are the first
    /// append's and do not take part: a retry, a replay or an import of
    /// the same reference is the same reference.
    pub fn same_reference(&self, other: &SourceReference) -> bool {
        self.reference_id == other.reference_id
            && self.evidence_id == other.evidence_id
            && self.asset_id == other.asset_id
            && self.sha256 == other.sha256
            && self.byte_length == other.byte_length
            && self.source == other.source
    }
}

/// The contract string of a normalized-text derivation (format v1.0).
pub const NORMALIZED_TEXT_CONTRACT: &str = "antares.normalized-text/v1";

/// Which normalizer produced a derivative: stable parser identity, its
/// version, and the digest of its configuration. A new version or
/// configuration is a new job, never a rewrite.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Normalizer {
    /// Stable parser identity, `[A-Za-z0-9_.:-]{1,64}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,64}$"))
    )]
    pub name: String,
    /// Parser version, `[A-Za-z0-9_.:-]{1,64}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,64}$"))
    )]
    pub version: String,
    /// SHA-256 of the normalizer configuration, 64 lowercase hex characters.
    #[cfg_attr(feature = "schemars", schemars(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub configuration_sha256: String,
}

/// Where one derivative sits in its job: a stable 0-based slot (slots may
/// be sparse while work is pending, partial or refused), the source
/// locator it covers, and the producer's opaque per-segment coverage facts.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DerivationSegment {
    /// Stable 0-based slot within the job. Not assumed dense, and not
    /// assumed to equal a parser-unit ordinal.
    pub index: u64,
    /// Where in the original this text came from, at most
    /// [`Derivation::LOCATOR_MAX_BYTES`] bytes (UTF-8), with no control
    /// characters. The schema bounds characters; readers bound the bytes.
    #[cfg_attr(
        feature = "schemars",
        schemars(
            length(min = 1, max = 2048),
            regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$")
        )
    )]
    pub locator: String,
    /// The producer's coverage facts for this segment: a JSON object of at
    /// most [`Derivation::COVERAGE_MAX_BYTES`] serialized, nesting at most
    /// [`Derivation::COVERAGE_MAX_DEPTH`] levels, carried verbatim and never
    /// interpreted by the engine. It says nothing about the whole job's
    /// readiness.
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    #[cfg_attr(feature = "schemars", schemars(extend("type" = "object")))]
    pub coverage: serde_json::Value,
}

/// A cleaned-text Evidence's typed, immutable binding to the stored
/// original it was normalized from (format v1.0, contract
/// [`NORMALIZED_TEXT_CONTRACT`]).
///
/// Carried only by a derivative: ordinary blob-free Evidence whose
/// `content` is the cleaned text. The binding (`primary_evidence_id`,
/// `asset_id`, `sha256`, `byte_length`) must equal the primary's
/// [`SourceBlob`]; `text_sha256`/`text_byte_length` must equal the digest
/// and UTF-8 length of the record's `content`. The slot
/// `(primary_evidence_id, job_id, segment.index)` holds exactly one
/// derivative. Once written, a derivation is never changed or dropped.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Derivation {
    /// Always [`NORMALIZED_TEXT_CONTRACT`].
    #[cfg_attr(feature = "schemars", schemars(extend("const" = "antares.normalized-text/v1")))]
    pub contract: String,
    /// The primary Evidence carrying the original: non-empty, with no
    /// control characters.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$"))
    )]
    pub primary_evidence_id: String,
    /// That primary's `sourceBlob.assetId`, `[A-Za-z0-9_-]{16,128}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_-]{16,128}$"))
    )]
    pub asset_id: String,
    /// That primary's `sourceBlob.sha256`.
    #[cfg_attr(feature = "schemars", schemars(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub sha256: String,
    /// That primary's `sourceBlob.byteLength`.
    pub byte_length: u64,
    /// The normalizer that produced this text.
    pub normalizer: Normalizer,
    /// The producer's job, `[A-Za-z0-9_.:-]{1,128}`.
    #[cfg_attr(
        feature = "schemars",
        schemars(regex(pattern = r"^[A-Za-z0-9_.:-]{1,128}$"))
    )]
    pub job_id: String,
    /// This derivative's slot, locator and coverage.
    pub segment: DerivationSegment,
    /// SHA-256 of the record's `content` (its UTF-8 bytes).
    #[cfg_attr(feature = "schemars", schemars(regex(pattern = r"^[0-9a-f]{64}$")))]
    pub text_sha256: String,
    /// UTF-8 byte length of the record's `content`.
    pub text_byte_length: u64,
}

/// Whether `v` nests deeper than `levels` arrays and objects, `v` itself
/// counting as one level when it is one. Recursion stops at `levels`.
fn nests_deeper(v: &serde_json::Value, levels: usize) -> bool {
    match v {
        serde_json::Value::Array(items) => {
            levels == 0 || items.iter().any(|c| nests_deeper(c, levels - 1))
        }
        serde_json::Value::Object(members) => {
            levels == 0 || members.values().any(|c| nests_deeper(c, levels - 1))
        }
        _ => false,
    }
}

fn is_token(s: &str, max: usize) -> bool {
    (1..=max).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Derivation {
    /// Longest `segment.locator`, in bytes.
    pub const LOCATOR_MAX_BYTES: usize = 2 * 1024;
    /// Largest serialized `segment.coverage` object.
    pub const COVERAGE_MAX_BYTES: usize = 8 * 1024;
    /// Deepest `segment.coverage`: arrays and objects nested, the coverage
    /// itself one. Well inside the 127 levels a JSON parser accepts, so the
    /// derivative still reads in every envelope that carries it (a feed
    /// frame puts coverage seven levels down).
    pub const COVERAGE_MAX_DEPTH: usize = 64;

    /// Structural validity of the derivation alone. Returns the first
    /// problem, naming the field.
    pub fn validate(&self) -> Result<(), String> {
        if self.contract != NORMALIZED_TEXT_CONTRACT {
            return Err(format!(
                "derivation.contract must be `{NORMALIZED_TEXT_CONTRACT}`"
            ));
        }
        if self.primary_evidence_id.is_empty()
            || self.primary_evidence_id.chars().any(char::is_control)
        {
            return Err(
                "derivation.primaryEvidenceId must be non-empty with no control characters".into(),
            );
        }
        if !SourceBlob::is_valid_asset_id(&self.asset_id) || !is_sha256_hex(&self.sha256) {
            return Err("derivation.assetId/sha256 are malformed".into());
        }
        if !is_token(&self.normalizer.name, 64) || !is_token(&self.normalizer.version, 64) {
            return Err(
                "derivation.normalizer.name/version must be 1..=64 characters of [A-Za-z0-9_.:-]"
                    .into(),
            );
        }
        if !is_sha256_hex(&self.normalizer.configuration_sha256) {
            return Err(
                "derivation.normalizer.configurationSha256 must be 64 lowercase hex characters"
                    .into(),
            );
        }
        if !is_token(&self.job_id, 128) {
            return Err("derivation.jobId must be 1..=128 characters of [A-Za-z0-9_.:-]".into());
        }
        if self.segment.locator.is_empty()
            || self.segment.locator.len() > Self::LOCATOR_MAX_BYTES
            || self.segment.locator.chars().any(char::is_control)
        {
            return Err(format!(
                "derivation.segment.locator must be 1..={} bytes with no control characters",
                Self::LOCATOR_MAX_BYTES
            ));
        }
        if !self.segment.coverage.is_object() {
            return Err("derivation.segment.coverage must be a JSON object".into());
        }
        if serde_json::to_vec(&self.segment.coverage).map_or(usize::MAX, |b| b.len())
            > Self::COVERAGE_MAX_BYTES
        {
            return Err(format!(
                "derivation.segment.coverage exceeds {} bytes serialized",
                Self::COVERAGE_MAX_BYTES
            ));
        }
        if nests_deeper(&self.segment.coverage, Self::COVERAGE_MAX_DEPTH) {
            return Err(format!(
                "derivation.segment.coverage nests deeper than {} levels",
                Self::COVERAGE_MAX_DEPTH
            ));
        }
        if !is_sha256_hex(&self.text_sha256) {
            return Err("derivation.textSha256 must be 64 lowercase hex characters".into());
        }
        Ok(())
    }

    /// Whether this derivation binds to exactly `blob` of Evidence
    /// `primary`.
    pub fn binds(&self, primary: &str, blob: &SourceBlob) -> bool {
        self.primary_evidence_id == primary
            && self.asset_id == blob.asset_id
            && self.sha256 == blob.sha256
            && self.byte_length == blob.byte_length
    }

    /// Whether `content` is exactly the text this derivation names:
    /// `text_sha256` is the lowercase hex SHA-256 of its UTF-8 bytes, which
    /// the caller computes (`content_sha256`), and the length matches.
    pub fn names_text(&self, content: &str, content_sha256: &str) -> bool {
        self.text_byte_length == content.len() as u64 && self.text_sha256 == content_sha256
    }
}

/// A source-bound piece of supporting material: where it came from,
/// the literal content, and optional span offsets into the source.
/// The payload of an `evidence` record.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence id, unique within the scope.
    pub id: EvidenceId,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Owning project.
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
    /// The stored original this Evidence was cut from, when it is the
    /// primary record of a file (format v1.0). Absent on every other
    /// record, which keeps their bytes unchanged. The original itself is
    /// never embedded here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_blob: Option<SourceBlob>,
    /// The typed binding of a cleaned-text derivative to the stored
    /// original it was normalized from (format v1.0). Absent on every
    /// other record, which keeps their bytes unchanged. Never present
    /// together with `source_blob`.
    // Boxed: rare and large; inline it would double the size of every
    // Evidence and of every record enum holding one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Box<Derivation>>,
    /// Character offset of the span start in the source, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_start: Option<u32>,
    /// Character offset of the span end in the source, exclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_end: Option<u32>,
    /// Byte offset of the span start in the source, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_start: Option<u64>,
    /// Byte offset of the span end in the source, exclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_end: Option<u64>,
    /// Wall-clock time the source event happened or was seen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    /// Wall-clock time the extractor produced this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_at: Option<DateTime<Utc>>,
    /// Version tag of the producing extractor, verbatim.
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
            source_blob: None,
            derivation: None,
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
