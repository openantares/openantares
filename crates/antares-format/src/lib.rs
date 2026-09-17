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

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ant_types::{
    Belief, ContradictionCase, Evidence, Observation, OntologyRevision, RelationshipProposal,
    SchemaType, Vertex,
};

/// The `MAJOR.MINOR` format version written into new manifests.
pub const FORMAT_VERSION: &str = "0.7";
/// Conventional file extension for the container.
pub const EXTENSION: &str = "ant";

/// The `.ant` format version this crate reads and writes, as a
/// `MAJOR.MINOR` string. Crate version and format version are
/// formally independent: the crate at any semver may support format
/// `0.3.x`. Alias of [`FORMAT_VERSION`], named for README/consumer
/// use.
pub const SUPPORTED_FORMAT_VERSION: &str = FORMAT_VERSION;

/// Major version this reader implements. See [`FormatVersion`].
pub const FORMAT_MAJOR: u32 = 0;
/// Minor version this reader implements.
pub const FORMAT_MINOR: u32 = 7;

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

    /// Can this build read a file written at `self`?
    pub fn readable_by_current(&self) -> bool {
        self.major == FORMAT_MAJOR
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
];

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
    lines: std::io::Lines<BufReader<zstd::stream::read::Decoder<'static, BufReader<R>>>>,
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
}

impl<R: Read> AntReader<R> {
    /// Open a `.ant` stream: decode the framing, read and validate the
    /// manifest, and apply the version compatibility policy.
    pub fn new(input: R) -> Result<Self, AntError> {
        let dec = zstd::stream::read::Decoder::new(input)
            .map_err(|e| AntError::NotAnt(format!("zstd: {e}")))?;
        let mut lines = BufReader::new(dec).lines();
        let first = lines
            .next()
            .ok_or_else(|| AntError::NotAnt("empty stream".into()))??;
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
                "file is format v{file_version}, this reader implements v{}. \
                 Major versions are not compatible: a major bump means field \
                 meanings or the container framing changed, so reading it here \
                 would silently misinterpret records. Upgrade the reader to a \
                 v{}.x build, or re-export the file at v{}.",
                FormatVersion::CURRENT,
                file_version.major,
                FORMAT_MAJOR,
            )));
        }
        let minor_ahead = file_version.minor > FORMAT_MINOR;
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
        })
    }

    /// Next data record; `None` after a VERIFIED trailer. Unknown-kind
    /// lines are skipped (forward compatibility) but still hashed.
    pub fn next_record(&mut self) -> Result<Option<AntRecord>, AntError> {
        loop {
            let Some(line) = self.lines.next() else {
                return Err(AntError::Integrity(
                    "stream ended without a trailer (truncated?)".into(),
                ));
            };
            let line = line?;
            self.line_no += 1;
            // Trailer hash covers everything BEFORE the trailer line.
            let pre_trailer_digest = format!("{:x}", self.hasher.clone().finalize());
            self.hasher.update(line.as_bytes());
            self.hasher.update(b"\n");
            match serde_json::from_str::<AntRecord>(&line) {
                Ok(AntRecord::Manifest(_)) => {
                    return Err(AntError::NotAnt("duplicate manifest".into()))
                }
                Ok(AntRecord::Trailer { counts, sha256 }) => {
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
                        AntRecord::Manifest(_) | AntRecord::Trailer { .. } => unreachable!(),
                    }
                    return Ok(Some(rec));
                }
                Err(e) => {
                    // Unknown kind => forward-compat skip. Anything
                    // else malformed is a hard error.
                    let probe: Result<serde_json::Value, _> = serde_json::from_str(&line);
                    match probe {
                        Ok(v) if v.get("kind").and_then(|k| k.as_str()).is_some() => continue,
                        _ => {
                            return Err(AntError::Json {
                                line: self.line_no,
                                err: e.to_string(),
                            })
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
