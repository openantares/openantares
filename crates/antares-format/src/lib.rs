//! The Open Antares (`.ant`) container format.
//!
//! A self-contained, compressed, streamable container for exchanging a
//! selection of an Antares world model: schema types, vertices, edges,
//! observations, evidence (structured AND unstructured together),
//! beliefs, and vector docs (vectors ride in the file so an import can
//! restore them without re-embedding).
//!
//! The entry points are [`AntWriter`] (records in, `.ant` bytes out)
//! and [`AntReader`] (streaming reads with integrity verification);
//! both have runnable examples. [`SUPPORTED_FORMAT_VERSION`] names the
//! `MAJOR.MINOR` format version this build reads and writes — crate
//! version and format version are formally independent. The
//! compatibility rule is same-major: any minor at the same major is
//! readable (see [`FormatVersion`]).
//!
//! ## Container
//!
//! One zstd-compressed stream of NDJSON records:
//!
//! ```text
//! {"kind":"manifest", ...}      exactly one, first line
//! {"kind":"schema_type", "data":{...}}
//! {"kind":"vertex",      "data":{...}}
//! {"kind":"edge",        "data":{...}}
//! {"kind":"observation", "data":{...}}
//! {"kind":"evidence",    "data":{...}}
//! {"kind":"belief",      "data":{...}}
//! {"kind":"vector",      "data":{...}}
//! {"kind":"vertex_tombstone", "data":{...}}          v0.2
//! {"kind":"edge_tombstone",   "data":{...}}          v0.2
//! {"kind":"contradiction_case", "data":{...}}        v0.4
//! {"kind":"relationship_proposal", "data":{...}}     v0.5
//! {"kind":"ontology_revision", "data":{...}}         v0.7
//! {"kind":"original_chunk",  "data":{...}}         v1.0
//! {"kind":"original_source", "data":{...}}         v1.0
//! {"kind":"trailer", "counts":{...}, "sha256":"..."}   exactly one, last line
//! ```
//!
//! - `data` payloads are the serde JSON of the corresponding
//!   `ant-types` records (the same property encoding the wire and store
//!   use).
//!
//! ## Contradiction cases (v0.4)
//!
//! A `contradiction_case` record is one immutable revision of a case
//! comparing two or more exact claim revisions
//! ([`ant_types::ContradictionCase`]). It carries REFERENCES — belief
//! versions, observations, evidence positions, receipts — never copies,
//! which is what makes closure checkable: an archive holding a case
//! must hold every record the case references, and a closure
//! verifier refuses one that does not, exactly as it refuses a
//! dangling evidence id today. A case built from ids inside JSON
//! metadata would have let a "valid" archive omit the very revisions it
//! compares; a native kind cannot. Counted in the trailer as
//! `contradictionCases`, which older readers default to zero and
//! ignore when it is present (they skip the kind and hash it).
//!
//! ## Relationship proposals (v0.5)
//!
//! A `relationship_proposal` record is one immutable revision of a
//! relationship the reconnaissance loop proposed
//! ([`ant_types::RelationshipProposal`]): the join it proposes, the
//! run and source manifest it was measured under, the measurement
//! itself, where it stands — quarantined, supported, promoted by a
//! reviewer, refuted — and REFERENCES to the findings it was drawn
//! from, the SQL probes that measured it, and the reviewer's receipt
//! when one promoted it. Like a case, it carries references and never
//! copies, so an archive holding a proposal must hold every record it
//! cites. Counted in the trailer as `relationshipProposals`, which
//! older readers default to zero and ignore when present (they skip
//! the kind and hash it).
//!
//! Recording a proposal never publishes it into a mapping: what an
//! exporter may build edges from is a separate, reviewed decision.
//!
//! ## Stored originals (v1.0)
//!
//! A selection that carries stored originals is written as format
//! [`ORIGINALS_FORMAT_VERSION`] (1.0); every other selection is still
//! written as [`FORMAT_VERSION`] (0.7), byte for byte as before. A
//! primary Evidence names its original in `source_blob`
//! ([`ant_types::SourceBlob`]); the original's bytes follow it as
//! [`OriginalChunk`] records (base64, in order, each digest-checked, the
//! whole verified against the blob), then its provenance as
//! `original_source` records ([`ant_types::SourceReference`]). Cleaned
//! text can be bound to its primary's exact original through
//! `evidence.derivation` ([`ant_types::Derivation`]). The trailer counts
//! `originalChunks` and `originalSources`, omitted when zero.
//!
//! This is a MAJOR on purpose: a 0.x reader skips unknown kinds, so a
//! minor would let it import every Evidence while silently dropping the
//! originals. A 0.x reader refuses a 1.0 file at its manifest instead.
//! This build reads both majors it writes (0.x and 1.x).
//!
//! ## Elected ontology revisions (v0.7)
//!
//! An `ontology_revision` record carries one complete immutable elected
//! semantic envelope ([`ant_types::OntologyRevision`]): typed definitions,
//! reviewed vault/head pins, explicit publication closure, retained competing
//! and rejected positions, contributor attribution, the complete approval
//! attestation, first publisher credential, committed idempotency tuple, and
//! conditional-chain position. The current per-vault head is derived from the
//! unique validated chain; it is not a second canonical archive record.
//!
//! Import must reject a divergent same-id envelope, fork, cycle, domain
//! mismatch, or missing visible closure. Counted as `ontologyRevisions`; older
//! readers default that trailer key to zero and skip the unknown record kind
//! while still hashing its exact line.
//!
//! ## Explicitly-unknown observation time (v0.6)
//!
//! An observation's `observed_at` and `extracted_at` are no longer a
//! bare instant. They are an [`ant_types::EventTime`]: either
//! `Known { at, basis }` — an instant, optionally with the
//! [`ant_types::TimeBasis`] the instant was drawn from — or
//! `Unknown { reason }`, a first-class "this time is not known" that
//! carries an [`ant_types::UnknownTime`] reason rather than a null or a
//! fabricated epoch. A genuinely undated original is now representable
//! end to end instead of being dropped, and a time-aware reader treats
//! it AS unknown (it is on no timeline) rather than clamping it to the
//! epoch or to now.
//!
//! The wire form is additive, so the common case is byte-identical to
//! v0.5:
//!
//! ```text
//! "2026-08-09T10:00:00Z"                       Known, no basis (v0.5-identical)
//! {"known":{"at":"2026-08-09T10:00:00Z","basis":"source_record_time"}}
//! {"unknown":{"reason":"no_source_time"}}
//! ```
//!
//! A `Known` with no basis is the bare RFC3339 string every v0.5 file
//! already wrote — no historical record is re-encoded and no basis is
//! ever fabricated onto one. Only an observation that actually carries a
//! basis or an unknown time takes one of the two object forms. This is a
//! MINOR bump: a v0.6 reader reads every v0.5 file unchanged, and a v0.5
//! reader reads the bare-string subset of a v0.6 file and errors only on
//! a record that uses the new states — which is exactly what "the file
//! is ahead of this reader" is there to signal. The vocabulary
//! (`Known`/`Unknown`, the basis and reason names) is shared with the
//! mining layer's `EventTime`, not a second dialect.
//!
//! ## Property values (v0.3)
//!
//! v0.2 carried property values as bare untagged JSON scalars:
//! `null | bool | number | string | object`. That set cannot express the
//! SQL types — DECIMAL, DATE, TIME, TIMESTAMP, UUID and BLOB are all
//! JSON strings, so a reader could not tell a date from a string that
//! looks like one, and the type was lost on the first round-trip.
//!
//! v0.3 keeps those five shapes EXACTLY as they were and adds a tagged
//! envelope for the typed values:
//!
//! ```text
//! {"$ant":"decimal",   "v":"12345678901234567.89"}  exact, never f64
//! {"$ant":"date",      "v":"2024-03-01"}
//! {"$ant":"time",      "v":"12:30:45.123456"}
//! {"$ant":"timestamp", "v":"2024-03-01T12:00:00+02:00"}   offset kept
//! {"$ant":"uuid",      "v":"6ba7b810-..."}
//! {"$ant":"bytes",     "v":"<base64>"}
//! {"$ant":"int32",     "v":-2147483648}
//! {"$ant":"int16",     "v":-32768}
//! {"$ant":"array",     "v":[ <property values> ]}
//! ```
//!
//! An object is an envelope ONLY when it has exactly the two keys
//! `$ant` and `v` and `$ant` names a known type; anything else is an
//! ordinary JSON document value. So a producer's own document with a
//! `$ant` field still round-trips as that document.
//!
//! This is a MINOR bump because it is additive under the compatibility
//! policy below: a v0.2 reader reads a v0.3 file without error, and any
//! value it already understood is byte-identical. What it loses is the
//! type — an envelope decodes as a plain JSON object rather than as a
//! decimal — which is precisely what "the file is ahead of this reader"
//! is there to signal.
//!
//! ## Integrity and identification
//!
//! - The trailer's `sha256` is over every preceding UNCOMPRESSED line
//!   including newlines (manifest through the last record), so
//!   truncation and tampering are detectable without a second pass.
//! - File identification: the zstd magic plus a first record with
//!   `kind == "manifest"` and a supported `format` version.
//! - Records of unknown `kind` are skipped by readers (forward
//!   compatibility); additive fields inside `data` follow serde
//!   defaults.
//!
//! ## Selection semantics (writer-side contract)
//!
//! A `.ant` file carries whatever selection the exporter chose (whole
//! scope, a seed set + traversal, a 50-row digest). The manifest
//! records the selection descriptor verbatim so the consumer knows what
//! the file claims to contain; **evidence closure** is the exporter's
//! obligation: every `evidence_id` referenced by an exported
//! observation/edge should have its evidence record included.

use std::io::{BufRead, BufReader, Read, Write};

use ant_types::exact_json;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ant_types::{
    Belief, ContradictionCase, Evidence, Observation, OntologyRevision, RelationshipProposal,
    SchemaType, SourceReference, Vertex,
};

/// The `MAJOR.MINOR` format version written into new manifests.
pub const FORMAT_VERSION: &str = "0.7";
/// Conventional file extension for the container.
pub const EXTENSION: &str = "ant";

/// The `.ant` format version this crate reads and writes, as a
/// `MAJOR.MINOR` string. Crate version and format version are
/// formally independent: a crate at any semver may support any format
/// `MAJOR.MINOR`, and this constant — not the crate version — says which.
/// Alias of [`FORMAT_VERSION`], named for README/consumer use.
pub const SUPPORTED_FORMAT_VERSION: &str = FORMAT_VERSION;

/// Major version this reader implements. See [`FormatVersion`].
pub const FORMAT_MAJOR: u32 = 0;
/// Minor version this reader implements.
pub const FORMAT_MINOR: u32 = 7;

/// The version written for a selection that carries stored originals:
/// v0.7 plus `Evidence.sourceBlob` and `original_chunk` records (SPEC §5.6).
///
/// A MAJOR on purpose. A 0.x reader skips unknown record kinds and ignores
/// unknown fields, so a minor bump would let it import every Evidence while
/// silently dropping the originals; a different major is refused before
/// the first record. Selections with no original are still written as
/// [`FORMAT_VERSION`], byte for byte as before.
pub const ORIGINALS_FORMAT_VERSION: &str = "1.0";
/// Major version of [`ORIGINALS_FORMAT_VERSION`].
pub const ORIGINALS_FORMAT_MAJOR: u32 = 1;
/// Minor version of [`ORIGINALS_FORMAT_VERSION`].
pub const ORIGINALS_FORMAT_MINOR: u32 = 0;

/// Largest `original_chunk` a reader accepts, decoded. Bounds reader
/// memory; writers use smaller chunks (the engine writes 1 MiB).
pub const ORIGINAL_CHUNK_MAX_BYTES: usize = 8 * 1024 * 1024;

/// A parsed `MAJOR.MINOR` format version.
///
/// # Compatibility policy
///
/// The version gate used to be string equality, which meant a v0.1
/// reader refused a v0.2 file even when the only change was an added
/// record kind it already knew how to skip. That makes every additive
/// change a breaking one, which defeats the point of having a minor
/// version at all. The rule is now:
///
/// * **Same major, any minor → readable.** Minor bumps are
///   additive-only by contract: new record kinds, new optional fields.
///   An older reader skips what it does not recognise (see
///   [`AntReader::next_record`]) and still verifies the trailer, so it
///   gets a truthful subset rather than an error.
/// * **Unknown record kinds are skipped, not fatal** — within the same
///   major. They are still hashed, so integrity still holds.
/// * **A different major → rejected, explicitly.** A major bump is
///   reserved for changes an old reader would silently MISREAD:
///   changed field meanings, a different container framing, a
///   removed or repurposed kind. Refusing is the only safe answer.
/// * **This build reads two majors.** 0.x, and 1.x — the files that carry
///   stored originals ([`ORIGINALS_FORMAT_VERSION`]). A 0.x reader refuses
///   a 1.x file at its manifest, before any record, which is exactly why
///   originals are a major: nothing an old reader imports can be an
///   Evidence whose original it silently dropped.
/// * **A newer minor is not an error, but it is a known unknown.**
///   [`AntReader::minor_ahead`] reports it so a caller can warn that
///   the file may carry records this build did not surface.
///
/// Writers must therefore never change the meaning of an existing
/// field within a major. If a change cannot be expressed additively,
/// it needs a major bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FormatVersion {
    /// Major: different majors are mutually unreadable.
    pub major: u32,
    /// Minor: additive-only within a major.
    pub minor: u32,
}

impl FormatVersion {
    /// The version this build reads and writes.
    pub const CURRENT: FormatVersion = FormatVersion {
        major: FORMAT_MAJOR,
        minor: FORMAT_MINOR,
    };

    /// Parse `"MAJOR.MINOR"`. A bare `"1"` is treated as `1.0`.
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.trim().splitn(2, '.');
        let major = it.next()?.parse().ok()?;
        let minor = match it.next() {
            None => 0,
            Some(m) => m.parse().ok()?,
        };
        Some(FormatVersion { major, minor })
    }

    /// Can this build read a file written at `self`? This build reads
    /// both majors it writes: 0.x, and 1.x (files with stored originals).
    pub fn readable_by_current(&self) -> bool {
        self.major == FORMAT_MAJOR || self.major == ORIGINALS_FORMAT_MAJOR
    }

    /// Whether a file at `self` may carry stored originals.
    pub fn carries_originals(&self) -> bool {
        self.major >= ORIGINALS_FORMAT_MAJOR
    }

    /// Newest minor this build knows within `self`'s major.
    fn known_minor(&self) -> u32 {
        if self.major == ORIGINALS_FORMAT_MAJOR {
            ORIGINALS_FORMAT_MINOR
        } else {
            FORMAT_MINOR
        }
    }
}

impl std::fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Everything that can go wrong reading or writing a `.ant` stream.
#[derive(Debug, thiserror::Error)]
pub enum AntError {
    /// Underlying I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A line failed to parse as JSON.
    #[error("json on line {line}: {err}")]
    Json {
        /// 1-based line number in the uncompressed stream.
        line: u64,
        /// The parser's message.
        err: String,
    },
    /// The input is not a `.ant` container (bad framing, missing or
    /// malformed manifest, misplaced trailer).
    #[error("not an .ant stream: {0}")]
    NotAnt(String),
    /// Refused by the compatibility policy. The message says WHICH rule
    /// refused it — "version mismatch" alone leaves the reader guessing
    /// whether to upgrade, re-export, or file a bug.
    #[error("{0}")]
    Version(String),
    /// Trailer verification failed: hash or counts do not match the
    /// records actually read (truncation or tampering).
    #[error("integrity: {0}")]
    Integrity(String),
}

/// Edge payload: `ant_types::Edge` is graph-plane; serialize as-is.
pub use ant_types::Edge;

/// A vector document as exported (mirrors the vector store's doc).
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorRecord {
    /// Which plane the embedded record belongs to (e.g. "vertex").
    pub record_type: String,
    /// Id of the embedded record within that plane.
    pub record_id: String,
    /// Type label of the embedded record.
    pub label: String,
    /// Which field of the record the embedding covers.
    pub field: String,
    /// The embedding itself.
    pub vector: Vec<f32>,
    /// Short preview of the embedded text, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_preview: Option<String>,
    /// Evidence ids backing the embedded content, when available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_ids: Vec<String>,
}

/// The first record of every stream: what this file is, which scope
/// it came from, and what it claims to contain.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Always "antares" — belt for the zstd-magic braces.
    #[cfg_attr(feature = "schemars", schemars(schema_with = "format_tag_schema"))]
    pub format: String,
    /// `MAJOR.MINOR` version of this container layout.
    pub version: String,
    /// Originating tenant id.
    pub tenant_id: u64,
    /// Originating project id.
    pub project_id: u64,
    /// Free-form description of what was selected (whole scope, seed
    /// query, digest params...). Recorded verbatim, not interpreted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<serde_json::Value>,
    /// When the export was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Producer identifier (server version, tool).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
}

/// Per-kind record tallies, carried in the trailer and checked by the
/// reader against what it actually saw.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// `schema_type` records.
    pub schema_types: u64,
    /// `vertex` records.
    pub vertices: u64,
    /// `edge` records.
    pub edges: u64,
    /// `observation` records.
    pub observations: u64,
    /// `evidence` records.
    pub evidence: u64,
    /// `belief` records.
    pub beliefs: u64,
    /// `vector` records.
    pub vectors: u64,
    /// `vertex_tombstone` records. Added in v0.2. Absent in a v0.1
    /// trailer, where it means zero — readers MUST default it rather than
    /// reject the older file.
    #[serde(default)]
    pub vertex_tombstones: u64,
    /// `edge_tombstone` records. Added in v0.2. Absent in a v0.1 trailer,
    /// where it means zero — readers MUST default it rather than reject
    /// the older file.
    #[serde(default)]
    pub edge_tombstones: u64,
    /// `contradiction_case` records. Added in v0.4. Absent in a trailer
    /// written before v0.4, where it means zero — readers MUST default it
    /// rather than reject the older file; an older reader ignores the key.
    #[serde(default)]
    pub contradiction_cases: u64,
    /// `relationship_proposal` records. Added in v0.5. Absent in a trailer
    /// written before v0.5, where it means zero — readers MUST default it
    /// rather than reject the older file; an older reader ignores the key.
    #[serde(default)]
    pub relationship_proposals: u64,
    /// `ontology_revision` records. Added in v0.7. Absent in an older
    /// trailer, where it means zero; an older reader ignores the key and
    /// skips the record kind while preserving stream integrity verification.
    #[serde(default)]
    pub ontology_revisions: u64,
    /// `original_chunk` records. v1.0 only; omitted when zero, so a file
    /// without originals has exactly the v0.7 trailer.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub original_chunks: u64,
    /// `original_source` records. v1.0 only; omitted when zero.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub original_sources: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// A deletion, carried so a re-import can propagate it.
///
/// Import is otherwise additive: without this, deleting a vertex at the
/// source and re-exporting leaves the deleted record alive at the
/// destination forever, and the two stores silently diverge.
///
/// # Which planes can be tombstoned
///
/// Vertices and edges only. Those are the mutable graph planes — a
/// vertex is a current-state record and deleting one is a normal
/// operation.
///
/// Observations are append-only by design: an observation is a claim
/// that something was seen at a time, and un-saying it would break the
/// audit trail the format exists to carry. Evidence and beliefs are
/// likewise not tombstoned here — evidence is the justification other
/// records cite (deleting it would strand them, and the closure checker
/// would rightly call the file broken), and beliefs are derived state
/// that a re-materialisation regenerates. If retraction is ever needed
/// on those planes it should be a RETRACTION record carrying a reason,
/// not a delete — a different feature with different semantics.
///
/// # Conflict rules
///
/// * Tombstone for an id that does not exist locally → **no-op**, not
///   an error. Imports are meant to converge from any starting point,
///   and a file may legitimately carry a deletion the destination never
///   saw the creation of.
/// * A live record NEWER than the tombstone → **the record wins, the
///   delete is ignored**. `deleted_at` is compared against the live
///   record's last-write time; a stale tombstone must not resurrect a
///   deletion that a later write already undid. This is last-write-wins
///   on the same clock the rest of the store already uses.
/// * Ties (equal timestamps) → the **tombstone wins**, so a delete is
///   not lost to clock granularity.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    /// Id of the deleted record, in its own plane's namespace.
    pub id: String,
    /// When the deletion happened at the source. Drives the
    /// last-write-wins comparison above.
    pub deleted_at: chrono::DateTime<chrono::Utc>,
    /// Who deleted it, when the source knows. Advisory — carried for
    /// the audit trail, never used to decide the conflict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ant_types::AuthorStamp>,
}

/// The manifest's `format` tag. The reader refuses anything else, and
/// the published schema states it as a constant rather than `string`.
pub const FORMAT_NAME: &str = "antares";

#[cfg(feature = "schemars")]
fn format_tag_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "const": FORMAT_NAME })
}

/// A trailer hash is 64 lowercase hex digits — the reader compares it to
/// its own digest, and the schema states the shape.
#[cfg(feature = "schemars")]
fn sha256_hex_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "string", "pattern": "^[0-9a-f]{64}$" })
}

/// The trailer key each data kind is tallied under — `vertex` records
/// under `vertices`, and so on. The reader's increments and the reference
/// bindings' count maps are both instances of this table; the schema
/// generator publishes it, and `data_kind_count_keys_cover_the_types`
/// fails when a record kind or a `Counts` field is missing from it.
pub const DATA_KIND_COUNT_KEYS: &[(&str, &str)] = &[
    ("schema_type", "schemaTypes"),
    ("vertex", "vertices"),
    ("edge", "edges"),
    ("observation", "observations"),
    ("evidence", "evidence"),
    ("belief", "beliefs"),
    ("vector", "vectors"),
    ("vertex_tombstone", "vertexTombstones"),
    ("edge_tombstone", "edgeTombstones"),
    ("contradiction_case", "contradictionCases"),
    ("relationship_proposal", "relationshipProposals"),
    ("ontology_revision", "ontologyRevisions"),
    ("original_chunk", "originalChunks"),
    ("original_source", "originalSources"),
];

/// A line's `kind`, read without decoding any number on the line.
fn line_kind(line: &str) -> Option<String> {
    exact_json::field(line.as_bytes(), &["kind"])
        .ok()
        .flatten()
        .and_then(|k| k.as_str().map(str::to_string))
}

/// Decode one data line. A derivative's `segment.coverage` and a source
/// reference's `source` (format 1.0) are taken out exactly BEFORE the
/// shared parser reads the line (`exact_json`), so a finite double it would
/// round or refuse reaches the record exactly; every other kind, and every
/// other field, decodes exactly as before.
fn decode_line(line: &str) -> Result<AntRecord, String> {
    let path = match line_kind(line).as_deref() {
        Some("evidence") => exact_json::keys(&["data", "derivation", "segment", "coverage"]),
        Some("original_source") => exact_json::keys(&["data", "source"]),
        _ => return serde_json::from_str(line).map_err(|e| e.to_string()),
    };
    let (mut rec, mut values): (AntRecord, _) =
        exact_json::from_slice_exact(line.as_bytes(), &[path])?;
    if let Some(exact) = values.remove(0) {
        match &mut rec {
            AntRecord::Evidence { data } => {
                if let Some(d) = data.derivation.as_mut() {
                    d.segment.coverage = exact;
                }
            }
            AntRecord::OriginalSource { data } => data.source = exact,
            _ => {}
        }
    }
    Ok(rec)
}

/// Whether `kind` names a record this build decodes: every data kind, the
/// manifest and the trailer. Only other kinds are skipped as unknown.
fn is_known_kind(kind: &str) -> bool {
    kind == "manifest" || kind == "trailer" || DATA_KIND_COUNT_KEYS.iter().any(|(k, _)| *k == kind)
}

/// Trailer count keys a writer OMITS when zero, so files that never use
/// the kind keep their earlier trailer bytes exactly (v0.7 files carry no
/// `originalChunks`). Bindings that write trailers must do the same; the
/// schema generator publishes this list to them.
pub const OMITTED_WHEN_ZERO_COUNT_KEYS: &[&str] = &["originalChunks", "originalSources"];

/// One chunk of a stored original (v1.0). Every chunk of an Evidence's
/// original follows that Evidence record immediately, in order, before any
/// other record; together they are exactly `sourceBlob.byteLength` bytes
/// with digest `sourceBlob.sha256`. All chunks but the last have the length
/// of the first; the last is no longer. [`AntReader`] enforces every one
/// of those rules as it reads, so a reader that returns without error has
/// verified each original it passed through.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalChunk {
    /// The Evidence record this chunk belongs to — the one just before it.
    pub evidence_id: String,
    /// That Evidence's `sourceBlob.assetId`.
    pub asset_id: String,
    /// Position in the original, from 0.
    pub index: u64,
    /// Offset of the chunk's first byte in the original.
    pub byte_offset: u64,
    /// SHA-256 of the decoded chunk, 64 lowercase hex.
    #[cfg_attr(feature = "schemars", schemars(schema_with = "sha256_hex_schema"))]
    pub sha256: String,
    /// The chunk's bytes, standard base64 with padding.
    pub bytes: String,
}

impl OriginalChunk {
    /// Build a chunk record from raw bytes.
    pub fn new(
        evidence_id: &str,
        asset_id: &str,
        index: u64,
        byte_offset: u64,
        raw: &[u8],
    ) -> Self {
        use base64::Engine as _;
        Self {
            evidence_id: evidence_id.to_string(),
            asset_id: asset_id.to_string(),
            index,
            byte_offset,
            sha256: format!("{:x}", Sha256::digest(raw)),
            bytes: base64::engine::general_purpose::STANDARD.encode(raw),
        }
    }

    /// The decoded bytes. Refuses malformed base64 and anything larger
    /// than [`ORIGINAL_CHUNK_MAX_BYTES`] before allocating for it.
    pub fn decode(&self) -> Result<Vec<u8>, String> {
        use base64::Engine as _;
        if self.bytes.len() / 4 * 3 > ORIGINAL_CHUNK_MAX_BYTES + 3 {
            return Err(format!(
                "original_chunk {} exceeds the {ORIGINAL_CHUNK_MAX_BYTES}-byte limit",
                self.index
            ));
        }
        base64::engine::general_purpose::STANDARD
            .decode(&self.bytes)
            .map_err(|e| format!("original_chunk {}: bytes are not base64: {e}", self.index))
    }
}

/// One record in the stream.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AntRecord {
    /// The stream header. Exactly one, first line.
    Manifest(Manifest),
    /// A schema type declaration.
    SchemaType {
        /// The declaration.
        data: SchemaType,
    },
    /// A graph vertex.
    Vertex {
        /// The vertex.
        data: Vertex,
    },
    /// A graph edge.
    Edge {
        /// The edge.
        data: Edge,
    },
    /// An observation.
    Observation {
        /// The observation.
        data: Observation,
    },
    /// An evidence record.
    Evidence {
        /// The evidence.
        data: Evidence,
    },
    /// A belief version.
    Belief {
        /// The belief.
        data: Belief,
    },
    /// An embedding document.
    Vector {
        /// The embedding document.
        data: VectorRecord,
    },
    /// Deletion of a vertex (v0.2), carried so a re-import propagates the
    /// deletion instead of leaving the record alive at the destination
    /// forever. Cascades to its edges on import, exactly as a live delete
    /// does. Only the vertex and edge planes may be tombstoned:
    /// observations are append-only, evidence is cited by other records,
    /// and beliefs are derived state.
    VertexTombstone {
        /// The deletion.
        data: Tombstone,
    },
    /// Deletion of an edge (v0.2), carried so a re-import propagates the
    /// deletion instead of leaving the record alive at the destination
    /// forever. Only the vertex and edge planes may be tombstoned:
    /// observations are append-only, evidence is cited by other records,
    /// and beliefs are derived state.
    EdgeTombstone {
        /// The deletion.
        data: Tombstone,
    },
    /// One immutable revision of a contradiction case (v0.4). Every id it
    /// references MUST resolve inside the same file (SPEC.md §5.2).
    // Boxed: a case carries several reference lists and would otherwise
    // make every record slot the size of the largest case.
    ContradictionCase {
        /// The revision.
        data: Box<ContradictionCase>,
    },
    /// One immutable revision of a relationship proposal (v0.5). Every id
    /// it references MUST resolve inside the same file (SPEC.md §5.3).
    // Boxed for the same reason as a case.
    RelationshipProposal {
        /// The revision.
        data: Box<RelationshipProposal>,
    },
    /// One immutable elected ontology revision (v0.7). The envelope contains
    /// every byte required to rebuild its idempotency indexes and validated
    /// conditional head without inventing authority during hydration.
    // Boxed because manifests retain the complete typed semantic and evidence
    // closure and would otherwise determine the size of every enum slot.
    OntologyRevision {
        /// The immutable elected revision.
        data: Box<OntologyRevision>,
    },
    /// One chunk of an Evidence's stored original (v1.0), right after that
    /// Evidence. See [`OriginalChunk`].
    OriginalChunk {
        /// The chunk.
        data: OriginalChunk,
    },
    /// One source reference of an Evidence's stored original (v1.0): where
    /// the bytes came from. All of them follow that original's last chunk,
    /// in strictly increasing `referenceId` order, bound to exactly that
    /// Evidence's `sourceBlob`.
    OriginalSource {
        /// The reference.
        data: SourceReference,
    },
    /// The stream footer: per-kind counts and the integrity hash.
    /// Exactly one, last line.
    Trailer {
        /// Per-kind record tallies.
        counts: Counts,
        /// Hex sha256 over every preceding uncompressed line.
        #[cfg_attr(feature = "schemars", schemars(schema_with = "sha256_hex_schema"))]
        sha256: String,
    },
}

/// Streaming `.ant` writer: records in, zstd-framed NDJSON out.
/// Call [`AntWriter::finish`] to emit the trailer and flush.
///
/// # Example
///
/// Write a small file — manifest first (via [`AntWriter::new`]), then
/// records, then [`AntWriter::finish`] to append the trailer with the
/// per-kind counts and the integrity hash:
///
/// ```
/// use ant_types::{Evidence, ProjectId, TenantId, TypeName, Vertex, VertexId};
/// use antares_format::{AntRecord, AntWriter, Manifest, FORMAT_VERSION};
/// use std::collections::BTreeMap;
///
/// # fn main() -> Result<(), antares_format::AntError> {
/// let manifest = Manifest {
///     format: "antares".into(),
///     version: FORMAT_VERSION.into(),
///     tenant_id: 1,
///     project_id: 1,
///     selection: None,
///     created_at: None,
///     producer: Some("doctest/0.1".into()),
/// };
///
/// // Any `std::io::Write` works; a Vec keeps the example in memory.
/// let mut writer = AntWriter::new(Vec::new(), manifest, 0)?;
/// writer.write(AntRecord::Vertex {
///     data: Vertex {
///         id: VertexId("deal_1".into()),
///         name: "Example Deal".into(),
///         label: TypeName("Demo.Deal".into()),
///         properties: BTreeMap::new(),
///     },
/// })?;
/// writer.write(AntRecord::Evidence {
///     data: Evidence::quick(
///         "ev1", TenantId(1), ProjectId(1),
///         "note", "n1", "example content",
///     ),
/// })?;
///
/// let bytes = writer.finish()?; // appends the trailer, flushes zstd
/// assert!(!bytes.is_empty());
/// # Ok(())
/// # }
/// ```
pub struct AntWriter<W: Write> {
    enc: std::io::BufWriter<zstd::stream::write::Encoder<'static, W>>,
    hasher: Sha256,
    counts: Counts,
    finished: bool,
}

struct HashingWrite<'a, W> {
    out: &'a mut W,
    hasher: &'a mut Sha256,
}

impl<W: Write> Write for HashingWrite<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.out.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

impl<W: Write> AntWriter<W> {
    /// Compression level 0 = zstd default (currently 3). Levels up to
    /// 19 trade speed for size; text-heavy evidence compresses 5-10x
    /// at the default already.
    pub fn new(out: W, manifest: Manifest, level: i32) -> Result<Self, AntError> {
        let enc = zstd::stream::write::Encoder::new(out, level)?;
        let mut w = Self {
            enc: std::io::BufWriter::with_capacity(64 * 1024, enc),
            hasher: Sha256::new(),
            counts: Counts::default(),
            finished: false,
        };
        w.write_record(&AntRecord::Manifest(manifest))?;
        Ok(w)
    }

    /// Write a manifest whose selection metadata is serialized incrementally.
    /// This is the same wire format as `new`; large vault-reference maps may
    /// come from a disk cursor instead of a `serde_json::Value` in memory.
    /// `selection` replaces `manifest.selection`.
    pub fn new_with_selection<S: Serialize>(
        out: W,
        manifest: &Manifest,
        selection: &S,
        level: i32,
    ) -> Result<Self, AntError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Header<'a, S> {
            kind: &'static str,
            format: &'a str,
            version: &'a str,
            tenant_id: u64,
            project_id: u64,
            selection: &'a S,
            #[serde(skip_serializing_if = "Option::is_none")]
            created_at: &'a Option<chrono::DateTime<chrono::Utc>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            producer: &'a Option<String>,
        }
        let mut w = Self {
            enc: std::io::BufWriter::with_capacity(
                64 * 1024,
                zstd::stream::write::Encoder::new(out, level)?,
            ),
            hasher: Sha256::new(),
            counts: Counts::default(),
            finished: false,
        };
        w.write_serialized(&Header {
            kind: "manifest",
            format: &manifest.format,
            version: &manifest.version,
            tenant_id: manifest.tenant_id,
            project_id: manifest.project_id,
            selection,
            created_at: &manifest.created_at,
            producer: &manifest.producer,
        })?;
        Ok(w)
    }

    fn write_record(&mut self, rec: &AntRecord) -> Result<(), AntError> {
        self.write_serialized(rec)
    }

    fn write_serialized(&mut self, value: &impl Serialize) -> Result<(), AntError> {
        let mut out = HashingWrite {
            out: &mut self.enc,
            hasher: &mut self.hasher,
        };
        serde_json::to_writer(&mut out, value).map_err(|e| AntError::Json {
            line: 0,
            err: e.to_string(),
        })?;
        out.write_all(b"\n")?;
        Ok(())
    }

    /// Append one record. The manifest is written by [`AntWriter::new`]
    /// and the trailer by [`AntWriter::finish`]; passing either here is
    /// an error.
    pub fn write(&mut self, rec: AntRecord) -> Result<(), AntError> {
        match &rec {
            AntRecord::Manifest(_) => {
                return Err(AntError::NotAnt("manifest may only appear first".into()))
            }
            AntRecord::Trailer { .. } => {
                return Err(AntError::NotAnt("trailer is written by finish()".into()))
            }
            AntRecord::SchemaType { .. } => self.counts.schema_types += 1,
            AntRecord::Vertex { .. } => self.counts.vertices += 1,
            AntRecord::Edge { .. } => self.counts.edges += 1,
            AntRecord::Observation { .. } => self.counts.observations += 1,
            AntRecord::Evidence { .. } => self.counts.evidence += 1,
            AntRecord::Belief { .. } => self.counts.beliefs += 1,
            AntRecord::Vector { .. } => self.counts.vectors += 1,
            AntRecord::VertexTombstone { .. } => self.counts.vertex_tombstones += 1,
            AntRecord::EdgeTombstone { .. } => self.counts.edge_tombstones += 1,
            AntRecord::ContradictionCase { .. } => self.counts.contradiction_cases += 1,
            AntRecord::RelationshipProposal { .. } => self.counts.relationship_proposals += 1,
            AntRecord::OntologyRevision { .. } => self.counts.ontology_revisions += 1,
            AntRecord::OriginalChunk { .. } => self.counts.original_chunks += 1,
            AntRecord::OriginalSource { .. } => self.counts.original_sources += 1,
        }
        self.write_record(&rec)
    }

    /// Records written so far (excluding manifest/trailer).
    pub fn counts(&self) -> &Counts {
        &self.counts
    }

    /// Emit the trailer (counts + sha256 of everything before it) and
    /// finish the zstd frame. Returns the inner writer.
    pub fn finish(mut self) -> Result<W, AntError> {
        let digest = format!("{:x}", self.hasher.clone().finalize());
        let trailer = AntRecord::Trailer {
            counts: self.counts.clone(),
            sha256: digest,
        };
        let mut line = serde_json::to_string(&trailer).map_err(|e| AntError::Json {
            line: 0,
            err: e.to_string(),
        })?;
        line.push('\n');
        self.enc.write_all(line.as_bytes())?;
        self.finished = true;
        Ok(self
            .enc
            .into_inner()
            .map_err(|e| e.into_error())?
            .finish()?)
    }
}

/// Streaming `.ant` reader. Yields records after validating the
/// manifest; reading through to the trailer verifies counts + hash and
/// sets [`AntReader::verified`].
///
/// # Example
///
/// Read back a stream, one record at a time. The reader hashes every
/// line as it goes; when [`AntReader::next_record`] returns `Ok(None)`
/// the trailer's sha256 and per-kind counts have been checked against
/// what was actually read:
///
/// ```
/// use antares_format::{AntReader, AntRecord, AntWriter, Manifest, FORMAT_VERSION};
///
/// # fn main() -> Result<(), antares_format::AntError> {
/// # let manifest = Manifest {
/// #     format: "antares".into(),
/// #     version: FORMAT_VERSION.into(),
/// #     tenant_id: 1,
/// #     project_id: 1,
/// #     selection: None,
/// #     created_at: None,
/// #     producer: None,
/// # };
/// # let mut writer = AntWriter::new(Vec::new(), manifest, 0)?;
/// # writer.write(AntRecord::Evidence {
/// #     data: ant_types::Evidence::quick(
/// #         "ev1", ant_types::TenantId(1), ant_types::ProjectId(1),
/// #         "note", "n1", "example content",
/// #     ),
/// # })?;
/// # let bytes = writer.finish()?;
/// let mut reader = AntReader::new(bytes.as_slice())?;
/// assert_eq!(reader.manifest.format, "antares");
///
/// let mut records = 0;
/// while let Some(record) = reader.next_record()? {
///     match record {
///         AntRecord::Evidence { data } => assert_eq!(data.content, "example content"),
///         other => panic!("unexpected record: {other:?}"),
///     }
///     records += 1;
/// }
///
/// // `Ok(None)` means the trailer was reached AND verified: its hash
/// // and counts matched the records streamed above.
/// assert!(reader.verified);
/// assert_eq!(records, 1);
/// # Ok(())
/// # }
/// ```
pub struct AntReader<R: Read> {
    lines: Lines<R>,
    /// The manifest, validated during [`AntReader::new`].
    pub manifest: Manifest,
    hasher: Sha256,
    counts: Counts,
    line_no: u64,
    /// Set once the trailer was seen and verified.
    pub verified: bool,
    /// Version parsed from the manifest. Same major as this build, or
    /// `new` would have refused the file.
    pub version: FormatVersion,
    /// The file was written by a NEWER minor than this build. Readable
    /// by policy (minors are additive-only), but it may carry record
    /// kinds this build skipped — a caller that reports completeness to
    /// a user should say so.
    pub minor_ahead: bool,
    /// The original being read: its chunks must come next.
    original: Option<PendingOriginal>,
    /// The original just completed, whose source references may follow it,
    /// with the last `referenceId` seen.
    completed: Option<(String, ant_types::SourceBlob, Option<String>)>,
    /// The primary whose cleaned-text derivatives are being read, with the
    /// last `(jobId, index)` seen: derivatives follow their primary's
    /// original and source references, in strictly increasing order.
    derivatives_of: Option<(String, ant_types::SourceBlob, (String, u64))>,
}

/// State of an original whose chunks are still arriving.
struct PendingOriginal {
    blob: ant_types::SourceBlob,
    evidence_id: String,
    asset_id: String,
    byte_length: u64,
    sha256: String,
    next_index: u64,
    next_offset: u64,
    chunk_len: Option<u64>,
    hasher: Sha256,
}

/// Longest data line a v1.x file may carry (after its manifest). A 0.x
/// file keeps the reader's historical behaviour (no line bound); a 1.x
/// file carries base64 originals, so one oversized line is refused having
/// allocated at most this bound. Admits any legitimate record:
/// captured records are at most 16 MiB and a chunk at most
/// [`ORIGINAL_CHUNK_MAX_BYTES`] before base64.
pub const V1_DATA_LINE_MAX_BYTES: usize = 64 * 1024 * 1024;

/// Default bound on the memory a reader spends on the manifest — the line
/// itself plus its decoded form — whatever the file's version. The manifest
/// is read before the version is known, so this cannot depend on it; it
/// applies to 0.x files too (a deliberate compatibility decision, SPEC §2).
///
/// It is a bound on archive METADATA, never on an original: originals are
/// separate, individually bounded chunk lines and stream at any size. The
/// manifest grows with a scope's vault map (one id per attributed record),
/// so a very large vault-attributed scope can exceed the default; a reader
/// that can afford more passes a larger budget
/// ([`AntReader::new_with_manifest_budget`]; the engine's knob is
/// `ANTARES_IMPORT_MANIFEST_MEMORY_BYTES`). 256 MiB is the engine's default
/// export working-memory budget: reading an archive back costs no more
/// metadata memory than writing it was allowed to.
pub const MANIFEST_MEMORY_BUDGET_BYTES: usize = 256 * 1024 * 1024;

/// Upper bound, in bytes, on what decoding one JSON line into a
/// `serde_json::Value` tree allocates beyond the line itself, computed in one
/// pass over the bytes without decoding them.
///
/// Every value the line can produce is counted: each array element, object
/// member and the top-level value is introduced by `,` `[` `{` `:` or the
/// start of the line, so `values <= separators + 1`. Each costs at most
/// [`DECODED_VALUE_BYTES`] (the 32-byte `Value`, a doubling container's
/// slack for it, a map node's key/entry overhead and a string's minimum
/// heap block), and string contents cost at most their encoded length.
/// Deliberately pessimistic: a line of one-byte values (`0,0,0,…`) really
/// does expand ~16× before container slack, and this must refuse such a
/// line BEFORE the parser allocates, not after.
pub fn decoded_json_bytes_bound(line: &[u8]) -> u64 {
    let mut state = JsonScan::default();
    state.feed(line);
    state.bound()
}

/// Per-value allocation bound used by [`decoded_json_bytes_bound`].
pub const DECODED_VALUE_BYTES: u64 = 128;

/// Incremental form of [`decoded_json_bytes_bound`], for a line hashed in
/// chunks.
#[derive(Debug, Default, Clone)]
pub struct JsonScan {
    in_string: bool,
    escaped: bool,
    separators: u64,
    string_bytes: u64,
}

impl JsonScan {
    /// Account for the next bytes of the line.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.in_string {
                self.string_bytes += 1;
                if self.escaped {
                    self.escaped = false;
                } else if b == b'\\' {
                    self.escaped = true;
                } else if b == b'"' {
                    self.in_string = false;
                }
            } else {
                match b {
                    b'"' => self.in_string = true,
                    b',' | b'[' | b'{' | b':' => self.separators += 1,
                    _ => {}
                }
            }
        }
    }

    /// The decoded-allocation bound for everything fed so far.
    pub fn bound(&self) -> u64 {
        (self.separators + 1)
            .saturating_mul(DECODED_VALUE_BYTES)
            .saturating_add(self.string_bytes)
    }
}

/// Read one `\n`-terminated line into `buf` (cleared first), never
/// REQUESTING buffer capacity past `max + 1` bytes: an oversized line is
/// refused before the reader asks for room beyond the bound, not merely
/// before it keeps the bytes. (`read_until` doubles its buffer, so bounding
/// only the bytes read still asks for up to twice the bound.) The allocator
/// may round a request up; this bounds what is requested, and is not a
/// measured process-memory guarantee. Returns `Ok(None)` at
/// end of input, `Ok(Some(false))` for a line over `max` (its remainder is
/// not consumed — the reader stops there).
fn read_line_capped<B: BufRead>(
    input: &mut B,
    max: usize,
    buf: &mut Vec<u8>,
) -> std::io::Result<Option<bool>> {
    buf.clear();
    let limit = max.saturating_add(1);
    loop {
        let available = match input.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(if buf.is_empty() { None } else { Some(true) });
        }
        let newline = available.iter().position(|b| *b == b'\n');
        let take = newline.map_or(available.len(), |n| n + 1);
        let room = limit - buf.len();
        if take > room {
            // Over the bound: ask for no room past it.
            return Ok(Some(false));
        }
        let needed = buf.len() + take;
        if needed > buf.capacity() {
            let grown = buf.capacity().saturating_mul(2).max(needed).min(limit);
            buf.reserve_exact(grown - buf.len());
        }
        buf.extend_from_slice(&available[..take]);
        input.consume(take);
        if newline.is_some() {
            return Ok(Some(true));
        }
    }
}

/// Decoded NDJSON lines, optionally bounded. Strips `\n` / `\r\n` and
/// refuses invalid UTF-8 exactly as `BufRead::lines` does, so an unbounded
/// read is byte-for-byte the historical behaviour.
struct Lines<R: Read> {
    inner: BufReader<zstd::stream::read::Decoder<'static, BufReader<R>>>,
    max: Option<usize>,
}

/// A line longer than the bound.
struct LineTooLong(usize);

impl<R: Read> Lines<R> {
    fn next_line(&mut self) -> Option<Result<Result<String, LineTooLong>, std::io::Error>> {
        let mut buf = Vec::new();
        match self.max {
            None => match self.inner.read_until(b'\n', &mut buf) {
                Err(e) => return Some(Err(e)),
                Ok(0) => return None,
                Ok(_) => {}
            },
            Some(max) => match read_line_capped(&mut self.inner, max, &mut buf) {
                Err(e) => return Some(Err(e)),
                Ok(None) => return None,
                Ok(Some(false)) => return Some(Ok(Err(LineTooLong(max)))),
                Ok(Some(true)) => {}
            },
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        Some(String::from_utf8(buf).map(Ok).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "stream did not contain valid UTF-8",
            )
        }))
    }
}

impl<R: Read> AntReader<R> {
    /// Open a `.ant` stream: decode the framing, read and validate the
    /// manifest, and apply the version compatibility policy. The manifest
    /// is held to [`MANIFEST_MEMORY_BUDGET_BYTES`].
    pub fn new(input: R) -> Result<Self, AntError> {
        Self::new_with_manifest_budget(input, MANIFEST_MEMORY_BUDGET_BYTES)
    }

    /// [`AntReader::new`] with a caller-chosen manifest memory budget: the
    /// manifest line may be at most `budget` bytes, and the line plus the
    /// [`decoded_json_bytes_bound`] of its decoded form must fit `budget`
    /// too. Both refusals happen before the parser allocates.
    pub fn new_with_manifest_budget(input: R, budget: usize) -> Result<Self, AntError> {
        let dec = zstd::stream::read::Decoder::new(input)
            .map_err(|e| AntError::NotAnt(format!("zstd: {e}")))?;
        let mut lines = Lines {
            inner: BufReader::new(dec),
            max: Some(budget),
        };
        let first = match lines
            .next_line()
            .ok_or_else(|| AntError::NotAnt("empty stream".into()))??
        {
            Ok(line) => line,
            Err(LineTooLong(max)) => {
                return Err(AntError::Integrity(format!(
                    "ARCHIVE_MANIFEST_TOO_LONG: the manifest line exceeds the {max}-byte \
                     manifest memory budget; refused after reading at most {max} bytes of it \
                     (raise the reader's manifest budget to open this archive)"
                )))
            }
        };
        let need = (first.len() as u64).saturating_add(decoded_json_bytes_bound(first.as_bytes()));
        if need > budget as u64 {
            return Err(AntError::Integrity(format!(
                "ARCHIVE_MANIFEST_TOO_LARGE: decoding this {}-byte manifest could need up to \
                 {need} bytes, over the {budget}-byte manifest memory budget; refused before \
                 decoding (raise the reader's manifest budget to at least {need} to open it)",
                first.len()
            )));
        }
        // 0.x data lines keep the historical unbounded read; 1.x lines are
        // bounded below, once the version is known.
        lines.max = None;
        let rec: AntRecord = serde_json::from_str(&first).map_err(|e| AntError::Json {
            line: 1,
            err: e.to_string(),
        })?;
        let AntRecord::Manifest(manifest) = rec else {
            return Err(AntError::NotAnt("first record is not a manifest".into()));
        };
        if manifest.format != FORMAT_NAME {
            return Err(AntError::NotAnt(format!("format `{}`", manifest.format)));
        }
        // Compatibility policy (see `FormatVersion`): same major reads,
        // different major is refused with the reason spelled out.
        let file_version = FormatVersion::parse(&manifest.version).ok_or_else(|| {
            AntError::Version(format!(
                "manifest version `{}` is not MAJOR.MINOR; this reader implements {}",
                manifest.version,
                FormatVersion::CURRENT
            ))
        })?;
        if !file_version.readable_by_current() {
            return Err(AntError::Version(format!(
                "file is format v{file_version}, this reader implements v{} and \
                 v{ORIGINALS_FORMAT_VERSION}. Major versions are not compatible: a major \
                 bump means field meanings or the container framing changed, so \
                 reading it here would silently misinterpret records. Upgrade the \
                 reader to a v{}.x build, or re-export the file at v{}.",
                FormatVersion::CURRENT,
                file_version.major,
                FORMAT_MAJOR,
            )));
        }
        let minor_ahead = file_version.minor > file_version.known_minor();
        if file_version.carries_originals() {
            lines.max = Some(V1_DATA_LINE_MAX_BYTES);
        }
        let mut hasher = Sha256::new();
        hasher.update(first.as_bytes());
        hasher.update(b"\n");
        Ok(Self {
            lines,
            manifest,
            hasher,
            counts: Counts::default(),
            line_no: 1,
            verified: false,
            version: file_version,
            minor_ahead,
            original: None,
            completed: None,
            derivatives_of: None,
        })
    }

    /// Enforce the v1.0 original rules on the record just read (see
    /// [`OriginalChunk`]): an Evidence with a `sourceBlob` is followed by
    /// exactly its chunks, contiguous, correctly sized and digested, and
    /// nothing else may interrupt them. A 0.x file may carry neither.
    fn check_originals(&mut self, rec: &AntRecord) -> Result<(), AntError> {
        let bad = |m: String| Err(AntError::Integrity(m));
        match rec {
            AntRecord::OriginalChunk { data } => {
                if !self.version.carries_originals() {
                    return bad(format!(
                        "original_chunk in a v{} file; originals need v{ORIGINALS_FORMAT_VERSION}",
                        self.version
                    ));
                }
                let Some(p) = self.original.as_mut() else {
                    return bad(format!(
                        "original_chunk {} of evidence {} does not follow its evidence record",
                        data.index, data.evidence_id
                    ));
                };
                if data.evidence_id != p.evidence_id || data.asset_id != p.asset_id {
                    return bad(format!(
                        "original_chunk for evidence {} / asset {} where evidence {} / asset {} \
                         is being read",
                        data.evidence_id, data.asset_id, p.evidence_id, p.asset_id
                    ));
                }
                if data.index != p.next_index || data.byte_offset != p.next_offset {
                    return bad(format!(
                        "original of evidence {}: chunk {} at offset {} where chunk {} at offset \
                         {} is next (missing or reordered chunk)",
                        p.evidence_id, data.index, data.byte_offset, p.next_index, p.next_offset
                    ));
                }
                let raw = data.decode().map_err(AntError::Integrity)?;
                let len = raw.len() as u64;
                if len == 0 || format!("{:x}", Sha256::digest(&raw)) != data.sha256 {
                    return bad(format!(
                        "original of evidence {}: chunk {} is empty or does not match its sha256",
                        p.evidence_id, data.index
                    ));
                }
                let end = p.next_offset + len;
                let first_len = *p.chunk_len.get_or_insert(len);
                if len > first_len
                    || end > p.byte_length
                    || (len < first_len && end != p.byte_length)
                {
                    return bad(format!(
                        "original of evidence {}: chunk {} has length {len}; chunks are {first_len} \
                         bytes except a shorter last one, {} in all",
                        p.evidence_id, data.index, p.byte_length
                    ));
                }
                p.hasher.update(&raw);
                p.next_index += 1;
                p.next_offset = end;
                if end == p.byte_length {
                    let p = self.original.take().expect("pending original");
                    if format!("{:x}", p.hasher.finalize()) != p.sha256 {
                        return bad(format!(
                            "original of evidence {} does not match its sourceBlob.sha256",
                            p.evidence_id
                        ));
                    }
                    self.completed = Some((p.evidence_id, p.blob, None));
                }
                Ok(())
            }
            AntRecord::OriginalSource { data } => {
                if !self.version.carries_originals() {
                    return bad(format!(
                        "original_source in a v{} file; originals need v{ORIGINALS_FORMAT_VERSION}",
                        self.version
                    ));
                }
                if let Some(p) = &self.original {
                    return bad(format!(
                        "the original of evidence {} ends at byte {} of {}: chunks missing",
                        p.evidence_id, p.next_offset, p.byte_length
                    ));
                }
                let Some((evidence, blob, last)) = self.completed.as_mut() else {
                    return bad(format!(
                        "original_source {} of evidence {} does not follow its original",
                        data.reference_id, data.evidence_id
                    ));
                };
                data.validate().map_err(AntError::Integrity)?;
                if !data.binds(evidence, blob) {
                    return bad(format!(
                        "original_source {} does not bind to the original of evidence {evidence}",
                        data.reference_id
                    ));
                }
                if last
                    .as_deref()
                    .is_some_and(|l| l >= data.reference_id.as_str())
                {
                    return bad(format!(
                        "original_source {} of evidence {evidence} is out of order or repeated",
                        data.reference_id
                    ));
                }
                *last = Some(data.reference_id.clone());
                Ok(())
            }
            other => {
                if let Some(p) = &self.original {
                    return bad(format!(
                        "the original of evidence {} ends at byte {} of {}: chunks missing",
                        p.evidence_id, p.next_offset, p.byte_length
                    ));
                }
                // A cleaned-text derivative continues its primary's group.
                if let AntRecord::Evidence { data } = other {
                    if let Some(d) = &data.derivation {
                        return self.check_derivative(data, d);
                    }
                }
                // Anything else ends the run of source references and
                // derivatives.
                self.completed = None;
                self.derivatives_of = None;
                if let AntRecord::Evidence { data } = other {
                    if let Some(blob) = &data.source_blob {
                        if !self.version.carries_originals() {
                            return bad(format!(
                                "evidence {} has a sourceBlob in a v{} file; originals need \
                                 v{ORIGINALS_FORMAT_VERSION}",
                                data.id.0, self.version
                            ));
                        }
                        blob.validate().map_err(AntError::Integrity)?;
                        if blob.byte_length == 0 {
                            if format!("{:x}", Sha256::digest(b"")) != blob.sha256 {
                                return bad(format!(
                                    "evidence {} declares an empty original with the wrong sha256",
                                    data.id.0
                                ));
                            }
                            self.completed = Some((data.id.0.clone(), blob.clone(), None));
                        } else {
                            self.original = Some(PendingOriginal {
                                blob: blob.clone(),
                                evidence_id: data.id.0.clone(),
                                asset_id: blob.asset_id.clone(),
                                byte_length: blob.byte_length,
                                sha256: blob.sha256.clone(),
                                next_index: 0,
                                next_offset: 0,
                                chunk_len: None,
                                hasher: Sha256::new(),
                            });
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// A derivative Evidence (v1.0): it follows its primary's completed
    /// original and source references, or another derivative of the same
    /// primary, in strictly increasing `(jobId, index)`; it binds exactly
    /// that primary's original; its content is exactly the text it names;
    /// it carries no original of its own.
    fn check_derivative(
        &mut self,
        data: &ant_types::Evidence,
        d: &ant_types::Derivation,
    ) -> Result<(), AntError> {
        let bad = |m: String| Err(AntError::Integrity(m));
        if !self.version.carries_originals() {
            return bad(format!(
                "evidence {} is a derivative in a v{} file; derivatives need \
                 v{ORIGINALS_FORMAT_VERSION}",
                data.id.0, self.version
            ));
        }
        if data.source_blob.is_some() {
            return bad(format!(
                "evidence {} carries both a derivation and an original",
                data.id.0
            ));
        }
        d.validate().map_err(AntError::Integrity)?;
        if format!("{:x}", Sha256::digest(data.content.as_bytes())) != d.text_sha256
            || data.content.len() as u64 != d.text_byte_length
        {
            return bad(format!(
                "derivative {}: content is not the text its derivation names",
                data.id.0
            ));
        }
        let (primary, blob, last) = match (self.completed.take(), self.derivatives_of.take()) {
            (Some((primary, blob, _)), _) => (primary, blob, None),
            (None, Some((primary, blob, last))) => (primary, blob, Some(last)),
            (None, None) => {
                return bad(format!(
                    "derivative {} does not follow its primary {}'s original",
                    data.id.0, d.primary_evidence_id
                ))
            }
        };
        if !d.binds(&primary, &blob) {
            return bad(format!(
                "derivative {} does not bind to the original of evidence {primary} it follows",
                data.id.0
            ));
        }
        let key = (d.job_id.clone(), d.segment.index);
        if last.as_ref().is_some_and(|l| *l >= key) {
            return bad(format!(
                "derivative {} of evidence {primary} is out of (jobId, index) order or repeated",
                data.id.0
            ));
        }
        self.derivatives_of = Some((primary, blob, key));
        Ok(())
    }

    /// Next data record; `None` after a VERIFIED trailer. Unknown-kind
    /// lines are skipped (forward compatibility) but still hashed.
    pub fn next_record(&mut self) -> Result<Option<AntRecord>, AntError> {
        loop {
            let Some(line) = self.lines.next_line() else {
                return Err(AntError::Integrity(
                    "stream ended without a trailer (truncated?)".into(),
                ));
            };
            self.line_no += 1;
            let line = match line? {
                Ok(line) => line,
                Err(LineTooLong(max)) => {
                    return Err(AntError::Integrity(format!(
                        "ARCHIVE_LINE_TOO_LONG: line {} exceeds the {max}-byte bound for a \
                         v{} file; refused after reading at most {max} bytes of it",
                        self.line_no, self.version
                    )))
                }
            };
            // Trailer hash covers everything BEFORE the trailer line.
            let pre_trailer_digest = format!("{:x}", self.hasher.clone().finalize());
            self.hasher.update(line.as_bytes());
            self.hasher.update(b"\n");
            match decode_line(&line) {
                Ok(AntRecord::Manifest(_)) => {
                    return Err(AntError::NotAnt("duplicate manifest".into()))
                }
                Ok(AntRecord::Trailer { counts, sha256 }) => {
                    if let Some(p) = &self.original {
                        return Err(AntError::Integrity(format!(
                            "the original of evidence {} ends at byte {} of {}: chunks missing",
                            p.evidence_id, p.next_offset, p.byte_length
                        )));
                    }
                    if sha256 != pre_trailer_digest {
                        return Err(AntError::Integrity(format!(
                            "sha256 mismatch: trailer {sha256}, computed {pre_trailer_digest}"
                        )));
                    }
                    if counts != self.counts {
                        return Err(AntError::Integrity(format!(
                            "counts mismatch: trailer {counts:?}, read {:?}",
                            self.counts
                        )));
                    }
                    self.verified = true;
                    return Ok(None);
                }
                Ok(rec) => {
                    match &rec {
                        AntRecord::SchemaType { .. } => self.counts.schema_types += 1,
                        AntRecord::Vertex { .. } => self.counts.vertices += 1,
                        AntRecord::Edge { .. } => self.counts.edges += 1,
                        AntRecord::Observation { .. } => self.counts.observations += 1,
                        AntRecord::Evidence { .. } => self.counts.evidence += 1,
                        AntRecord::Belief { .. } => self.counts.beliefs += 1,
                        AntRecord::Vector { .. } => self.counts.vectors += 1,
                        AntRecord::VertexTombstone { .. } => self.counts.vertex_tombstones += 1,
                        AntRecord::EdgeTombstone { .. } => self.counts.edge_tombstones += 1,
                        AntRecord::ContradictionCase { .. } => self.counts.contradiction_cases += 1,
                        AntRecord::RelationshipProposal { .. } => {
                            self.counts.relationship_proposals += 1
                        }
                        AntRecord::OntologyRevision { .. } => self.counts.ontology_revisions += 1,
                        AntRecord::OriginalChunk { .. } => self.counts.original_chunks += 1,
                        AntRecord::OriginalSource { .. } => self.counts.original_sources += 1,
                        AntRecord::Manifest(_) | AntRecord::Trailer { .. } => unreachable!(),
                    }
                    self.check_originals(&rec)?;
                    return Ok(Some(rec));
                }
                Err(e) => {
                    // A kind this build does not know => forward-compat
                    // skip. A KNOWN kind that does not decode is malformed,
                    // never skipped: skipping it (with a trailer adjusted to
                    // match) would drop a record this reader must check.
                    // Only a line that is complete, valid JSON to the
                    // shared parser may be skipped as an unknown kind:
                    // malformed JSON (`1.2.3`) or a number it refuses
                    // (`1e999`) in an unknown record is an error, as it was
                    // before the 1.0 opaque preparse existed.
                    let probe: Option<serde_json::Value> = serde_json::from_str(&line).ok();
                    let probed = probe
                        .as_ref()
                        .and_then(|v| v.get("kind")?.as_str().map(str::to_string));
                    match probed {
                        Some(kind) if !is_known_kind(&kind) => continue,
                        Some(kind) => {
                            return Err(AntError::Json {
                                line: self.line_no,
                                err: format!("malformed `{kind}` record: {e}"),
                            })
                        }
                        None => {
                            // Not valid JSON to the shared parser: never
                            // skipped. A known kind is still named.
                            let err = match line_kind(&line) {
                                Some(kind) if is_known_kind(&kind) => {
                                    format!("malformed `{kind}` record: {e}")
                                }
                                _ => e,
                            };
                            return Err(AntError::Json {
                                line: self.line_no,
                                err,
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ant_types::{ObservationId, ProjectId, TenantId, TypeName, VertexId};
    use std::collections::BTreeMap;

    /// Bounding only the bytes read is not bounding the allocation:
    /// `read_until` doubles its buffer. The capped read never lets the
    /// buffer's capacity pass the bound, even for a line far over it that
    /// arrives in many small pieces.
    #[test]
    fn a_capped_line_read_never_allocates_past_its_bound() {
        struct Trickle<'a>(&'a [u8]);
        impl std::io::Read for Trickle<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                let n = self.0.len().min(out.len()).min(7);
                out[..n].copy_from_slice(&self.0[..n]);
                self.0 = &self.0[n..];
                Ok(n)
            }
        }
        let max = 1000;
        let line = vec![b'x'; 3 * max];
        let mut input = std::io::BufReader::with_capacity(16, Trickle(&line));
        let mut buf = Vec::new();
        let got = super::read_line_capped(&mut input, max, &mut buf).unwrap();
        assert_eq!(got, Some(false), "a line over the bound is refused");
        assert!(
            buf.capacity() <= max + 1,
            "the buffer grew to {} bytes for a {max}-byte bound",
            buf.capacity()
        );
        // A line within the bound is read whole, terminator included.
        let ok = b"abc\ndef";
        let mut input = std::io::BufReader::with_capacity(2, Trickle(ok));
        assert_eq!(
            super::read_line_capped(&mut input, max, &mut buf).unwrap(),
            Some(true)
        );
        assert_eq!(buf, b"abc\n");
        assert_eq!(
            super::read_line_capped(&mut input, max, &mut buf).unwrap(),
            Some(true)
        );
        assert_eq!(buf, b"def");
        assert_eq!(
            super::read_line_capped(&mut input, max, &mut buf).unwrap(),
            None
        );
    }

    fn manifest() -> Manifest {
        Manifest {
            format: "antares".into(),
            version: FORMAT_VERSION.into(),
            tenant_id: 1,
            project_id: 1,
            selection: Some(serde_json::json!({"kind": "whole_scope"})),
            created_at: None,
            producer: Some("antares-format tests".into()),
        }
    }

    fn sample_vertex() -> Vertex {
        let mut props = BTreeMap::new();
        props.insert("amount".into(), ant_types::PropertyValue::Long(42));
        props.insert(
            "doc".into(),
            ant_types::PropertyValue::Json(serde_json::json!({"nested": [1, 2]})),
        );
        Vertex {
            id: VertexId("d1".into()),
            name: "Deal".into(),
            label: TypeName("Antares.Deal".into()),
            properties: props,
        }
    }

    fn sample_obs() -> Observation {
        Observation {
            id: ObservationId("o1".into()),
            tenant_id: TenantId(1),
            project_id: ProjectId(1),
            source_event_id: None,
            source_uri: None,
            subject_id: Some(VertexId("d1".into())),
            predicate: "stage_change".into(),
            object_id: None,
            object_value: Some(serde_json::json!("proposal")),
            observed_at: ant_types::EventTime::known("2026-08-09T00:00:00Z".parse().unwrap()),
            extracted_at: ant_types::EventTime::known("2026-08-09T00:00:01Z".parse().unwrap()),
            confidence: Some(0.9),
            evidence_ids: vec![],
            extractor_version: Some("test/1".into()),
            metadata: serde_json::Value::Null,
            author: None,
        }
    }

    fn sample_case() -> ContradictionCase {
        use ant_types::{
            BusinessImpact, CaseRevisionId, ClaimKind, ClaimRef, ComparatorIdentity,
            ContradictionCaseId, EpistemicState, WorkflowState,
        };
        ContradictionCase {
            id: CaseRevisionId("case_1@1".into()),
            case_id: ContradictionCaseId("case_1".into()),
            previous_revision_id: None,
            tenant_id: TenantId(1),
            project_id: ProjectId(1),
            family: "same_subject_numeric".into(),
            claims: vec![
                ClaimRef {
                    kind: ClaimKind::Observation,
                    id: "o1".into(),
                    version: None,
                    pointer: None,
                },
                ClaimRef {
                    kind: ClaimKind::Belief,
                    id: "b1".into(),
                    version: Some(1),
                    pointer: None,
                },
            ],
            evidence: vec![],
            measurements: vec![],
            comparator: ComparatorIdentity {
                comparator: "numeric_tolerance".into(),
                comparator_version: "1.0".into(),
                rule_id: None,
                rule_version: None,
                model: None,
                model_version: None,
                snapshot_id: Some("snap_1".into()),
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
            metadata: serde_json::Value::Null,
        }
    }

    fn write_sample() -> Vec<u8> {
        let mut w = AntWriter::new(Vec::new(), manifest(), 0).unwrap();
        w.write(AntRecord::Vertex {
            data: sample_vertex(),
        })
        .unwrap();
        w.write(AntRecord::Observation { data: sample_obs() })
            .unwrap();
        w.write(AntRecord::Vector {
            data: VectorRecord {
                record_type: "evidence".into(),
                record_id: "e1".into(),
                label: "Antares.Chunk".into(),
                field: "content".into(),
                vector: vec![0.1, 0.2, 0.3],
                text_preview: None,
                evidence_ids: vec![],
            },
        })
        .unwrap();
        w.write(AntRecord::ContradictionCase {
            data: Box::new(sample_case()),
        })
        .unwrap();
        assert_eq!(
            w.counts().contradiction_cases,
            1,
            "the writer tallies the kind"
        );
        w.finish().unwrap()
    }

    #[test]
    fn round_trip_verifies_and_preserves_records() {
        let bytes = write_sample();
        let mut r = AntReader::new(&bytes[..]).unwrap();
        assert_eq!(r.manifest.project_id, 1);
        let mut got = Vec::new();
        while let Some(rec) = r.next_record().unwrap() {
            got.push(rec);
        }
        assert!(r.verified, "trailer hash + counts verified");
        assert_eq!(got.len(), 4);
        assert_eq!(
            got[3],
            AntRecord::ContradictionCase {
                data: Box::new(sample_case())
            },
            "a case revision survives the round trip intact"
        );
        assert_eq!(
            got[0],
            AntRecord::Vertex {
                data: sample_vertex()
            },
            "typed properties (incl. Json variant) survive the round trip"
        );
        assert_eq!(got[1], AntRecord::Observation { data: sample_obs() });
    }

    #[test]
    fn tampering_and_truncation_are_detected() {
        let bytes = write_sample();
        // Tamper: flip a byte inside the compressed payload -> zstd or
        // hash layer must reject it.
        let mut bad = bytes.clone();
        let mid = bad.len() / 2;
        bad[mid] ^= 0xff;
        let corrupted = (|| -> Result<(), AntError> {
            let mut r = AntReader::new(&bad[..])?;
            while r.next_record()?.is_some() {}
            Ok(())
        })()
        .is_err();
        assert!(corrupted, "bit-flip must not verify");

        // Truncate: drop the tail -> must error, never silently succeed.
        let cut = &bytes[..bytes.len() - 8];
        let truncated = (|| -> Result<(), AntError> {
            let mut r = AntReader::new(cut)?;
            while r.next_record()?.is_some() {}
            Ok(())
        })()
        .is_err();
        assert!(truncated, "truncation must surface");
    }

    #[test]
    fn unknown_kinds_are_skipped_for_forward_compat() {
        // Hand-build a stream with an unknown record kind between valid
        // ones, with a correct trailer hash.
        use sha2::{Digest, Sha256};
        let m = serde_json::to_string(&AntRecord::Manifest(manifest())).unwrap();
        let v = serde_json::to_string(&AntRecord::Vertex {
            data: sample_vertex(),
        })
        .unwrap();
        let unknown = r#"{"kind":"hologram","data":{"future":true}}"#;
        let mut hasher = Sha256::new();
        for line in [&m, &v, &unknown.to_string()] {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        let trailer = AntRecord::Trailer {
            counts: Counts {
                vertices: 1,
                ..Default::default()
            },
            sha256: format!("{:x}", hasher.finalize()),
        };
        let t = serde_json::to_string(&trailer).unwrap();
        let raw = format!("{m}\n{v}\n{unknown}\n{t}\n");
        let compressed = zstd::stream::encode_all(raw.as_bytes(), 0).unwrap();

        let mut r = AntReader::new(&compressed[..]).unwrap();
        let mut kinds = Vec::new();
        while let Some(rec) = r.next_record().unwrap() {
            kinds.push(matches!(rec, AntRecord::Vertex { .. }));
        }
        assert!(r.verified);
        assert_eq!(kinds, vec![true], "unknown kind skipped, vertex kept");
    }

    /// Revision 30, finding 1: an unknown kind is skipped only when its line
    /// is valid JSON to the shared parser. Malformed JSON (`1.2.3`) and a
    /// number that parser refuses (`1e999`) in an unknown record are errors,
    /// even with a trailer adjusted to verify the line; a valid unknown line
    /// still skips (`unknown_kinds_are_skipped_for_forward_compat`).
    #[test]
    fn a_malformed_unknown_kind_is_refused_not_skipped() {
        use sha2::{Digest, Sha256};
        for data in [r#"{"x":1.2.3}"#, r#"{"x":1e999}"#] {
            let m = serde_json::to_string(&AntRecord::Manifest(manifest())).unwrap();
            let unknown = format!(r#"{{"kind":"hologram","data":{data}}}"#);
            let mut hasher = Sha256::new();
            for line in [&m, &unknown] {
                hasher.update(line.as_bytes());
                hasher.update(b"\n");
            }
            let trailer = AntRecord::Trailer {
                counts: Counts::default(),
                sha256: format!("{:x}", hasher.finalize()),
            };
            let t = serde_json::to_string(&trailer).unwrap();
            let raw = format!("{m}\n{unknown}\n{t}\n");
            let compressed = zstd::stream::encode_all(raw.as_bytes(), 0).unwrap();
            let mut r = AntReader::new(&compressed[..]).expect("the manifest opens");
            let result = r.next_record();
            assert!(
                matches!(result, Err(AntError::Json { .. })),
                "{data}: refused as JSON, not skipped to a verified trailer: {result:?}"
            );
            assert!(!r.verified, "{data}: never verified");
        }
    }

    /// Revision 30, finding 2 at the archive boundary: a selected opaque
    /// value must be valid JSON; a leading-zero number is refused, not
    /// read as 1.
    #[test]
    fn a_selected_opaque_value_with_invalid_json_is_refused() {
        let line = r#"{"kind":"original_source","data":{"evidenceId":"ev","assetId":"asset_original_0001","sha256":"0000000000000000000000000000000000000000000000000000000000000000","byteLength":1,"referenceId":"ref-1","source":{"n":01},"recordedAt":"2026-09-24T00:00:00Z"}}"#;
        let err = decode_line(line).expect_err("refused");
        assert!(err.contains("invalid number"), "{err}");
    }

    /// Revision 29: the 1.0 opaque field is taken out before the shared
    /// parser reads the line. A token that parser refuses as out of range
    /// (17976931348623158e292 is f64::MAX correctly rounded) decodes, and a
    /// duplicate key inside the opaque object resolves as serde_json::Value
    /// does: the last wins.
    #[test]
    fn a_source_the_shared_parser_refuses_decodes_exactly() {
        let line = r#"{"kind":"original_source","data":{"evidenceId":"ev","assetId":"asset_original_0001","sha256":"0000000000000000000000000000000000000000000000000000000000000000","byteLength":1,"referenceId":"ref-1","source":{"max":17976931348623158e292,"d":1,"d":2},"recordedAt":"2026-09-24T00:00:00Z"}}"#;
        assert!(
            serde_json::from_str::<AntRecord>(line).is_err(),
            "precondition: the shared parser alone refuses this line"
        );
        match decode_line(line).expect("decodes") {
            AntRecord::OriginalSource { data } => {
                assert_eq!(data.source["max"].as_f64(), Some(f64::MAX));
                assert_eq!(data.source["d"], 2, "the last duplicate key wins");
            }
            other => panic!("decoded as {other:?}"),
        }
        // Every other kind decodes exactly as the shared parser decodes it.
        let vertex = serde_json::to_string(&AntRecord::Vertex {
            data: sample_vertex(),
        })
        .unwrap();
        assert_eq!(
            format!("{:?}", decode_line(&vertex).unwrap()),
            format!("{:?}", serde_json::from_str::<AntRecord>(&vertex).unwrap())
        );
    }

    /// A KNOWN kind that does not decode is refused, not skipped as if it
    /// were unknown — even with a trailer adjusted to leave it uncounted.
    /// Here an evidence record whose derivation carries a field the type
    /// denies.
    #[test]
    fn a_malformed_known_kind_is_refused_not_skipped() {
        use sha2::{Digest, Sha256};
        let m = serde_json::to_string(&AntRecord::Manifest(manifest())).unwrap();
        let v = serde_json::to_string(&AntRecord::Vertex {
            data: sample_vertex(),
        })
        .unwrap();
        let malformed = r#"{"kind":"evidence","data":{"id":"ev1","tenant_id":1,"project_id":1,"source_uri":"u","source_type":"t","source_id":"s","content":"x","derivation":{"surprise":true}}}"#;
        let mut hasher = Sha256::new();
        for line in [&m, &v, &malformed.to_string()] {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        let trailer = AntRecord::Trailer {
            counts: Counts {
                vertices: 1,
                ..Default::default()
            },
            sha256: format!("{:x}", hasher.finalize()),
        };
        let t = serde_json::to_string(&trailer).unwrap();
        let raw = format!("{m}\n{v}\n{malformed}\n{t}\n");
        let compressed = zstd::stream::encode_all(raw.as_bytes(), 0).unwrap();
        let mut r = AntReader::new(&compressed[..]).unwrap();
        let mut result = Ok(None);
        for _ in 0..4 {
            result = r.next_record();
            if !matches!(result, Ok(Some(_))) {
                break;
            }
        }
        let err = result.expect_err("a malformed evidence record is an error");
        assert!(
            err.to_string().contains("malformed `evidence` record"),
            "refused for the malformed known kind: {err}"
        );
    }

    /// What keeps the v0.4 count key ADDITIVE: a reader that does not
    /// know a kind skips its records and must also ignore the trailer
    /// key that counts them. This is the v0.3 reader's situation with a
    /// v0.4 file, played by this build against a key it does not know.
    #[test]
    fn a_trailer_count_key_this_reader_does_not_know_is_ignored() {
        use sha2::{Digest, Sha256};
        let m = serde_json::to_string(&AntRecord::Manifest(manifest())).unwrap();
        let v = serde_json::to_string(&AntRecord::Vertex {
            data: sample_vertex(),
        })
        .unwrap();
        let unknown = r#"{"kind":"hologram","data":{"future":true}}"#;
        let mut hasher = Sha256::new();
        for line in [&m, &v, &unknown.to_string()] {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        // A trailer as a FUTURE writer would emit it: the kinds this
        // build knows at their true counts, plus a key for the kind it
        // skipped.
        let trailer = format!(
            r#"{{"kind":"trailer","counts":{{"schemaTypes":0,"vertices":1,"edges":0,"observations":0,"evidence":0,"beliefs":0,"vectors":0,"vertexTombstones":0,"edgeTombstones":0,"contradictionCases":0,"holograms":1}},"sha256":"{:x}"}}"#,
            hasher.finalize()
        );
        let raw = format!("{m}\n{v}\n{unknown}\n{trailer}\n");
        let compressed = zstd::stream::encode_all(raw.as_bytes(), 0).unwrap();
        let mut r = AntReader::new(&compressed[..]).unwrap();
        let mut n = 0;
        while r.next_record().unwrap().is_some() {
            n += 1;
        }
        assert!(
            r.verified,
            "an unknown count key must not fail verification, or every new kind is a breaking change"
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn wrong_version_and_non_ant_input_rejected() {
        let mut bad_manifest = manifest();
        bad_manifest.version = "9.9".into();
        let w = AntWriter::new(Vec::new(), bad_manifest, 0).unwrap();
        let bytes = w.finish().unwrap();
        assert!(matches!(
            AntReader::new(&bytes[..]),
            Err(AntError::Version(_))
        ));
        assert!(AntReader::new(&b"not zstd at all"[..]).is_err());
    }
}

#[cfg(all(test, feature = "schemars"))]
mod schema_facts {
    use super::*;
    use std::collections::BTreeSet;

    fn root(schema: schemars::Schema) -> serde_json::Value {
        serde_json::to_value(schema).unwrap()
    }

    /// Every data kind the record enum declares has a count key, every
    /// `Counts` field is some kind's tally, and nothing is listed twice.
    #[test]
    fn zero_counts_serialize_without_exactly_the_omitted_keys() {
        // `OMITTED_WHEN_ZERO_COUNT_KEYS` is what the bindings are told; the
        // serde attributes are what the Rust writer does. They must agree,
        // or a binding-written v0.7 trailer differs from Rust's.
        let zero = serde_json::to_value(Counts::default()).unwrap();
        let all: BTreeSet<&str> = DATA_KIND_COUNT_KEYS.iter().map(|(_, k)| *k).collect();
        let present: BTreeSet<&str> = zero
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        let omitted: BTreeSet<&str> = all.difference(&present).copied().collect();
        let declared: BTreeSet<&str> = OMITTED_WHEN_ZERO_COUNT_KEYS.iter().copied().collect();
        assert_eq!(
            omitted, declared,
            "zero Counts omits {omitted:?}, the list says {declared:?}"
        );
        let one = Counts {
            original_chunks: 1,
            ..Default::default()
        };
        assert_eq!(serde_json::to_value(one).unwrap()["originalChunks"], 1);
    }

    #[test]
    fn data_kind_count_keys_cover_the_types() {
        let record = root(schemars::schema_for!(AntRecord));
        let kinds: BTreeSet<String> = record["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arm| {
                arm["properties"]["kind"]["const"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .filter(|k| k != "manifest" && k != "trailer")
            .collect();
        let counts = root(schemars::schema_for!(Counts));
        let fields: BTreeSet<String> = counts["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let table_kinds: BTreeSet<String> = DATA_KIND_COUNT_KEYS
            .iter()
            .map(|(k, _)| k.to_string())
            .collect();
        let table_keys: BTreeSet<String> = DATA_KIND_COUNT_KEYS
            .iter()
            .map(|(_, v)| v.to_string())
            .collect();
        assert_eq!(table_kinds, kinds, "data kinds vs DATA_KIND_COUNT_KEYS");
        assert_eq!(table_keys, fields, "Counts fields vs DATA_KIND_COUNT_KEYS");
        assert_eq!(
            DATA_KIND_COUNT_KEYS.len(),
            kinds.len(),
            "one entry per kind"
        );
    }
}
