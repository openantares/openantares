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
//! {"kind":"trailer", "counts":{...}, "sha256":"..."}   exactly one, last line
//! ```
//!
//! - `data` payloads are the serde JSON of the corresponding
//!   `ant-types` records (the same property encoding the wire and store
//!   use).
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

#![warn(missing_docs)]

use std::io::{BufRead, BufReader, Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ant_types::{Belief, Evidence, Observation, SchemaType, Vertex};

/// The `MAJOR.MINOR` format version written into new manifests.
pub const FORMAT_VERSION: &str = "0.3";
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
pub const FORMAT_MINOR: u32 = 3;

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Always "antares" — belt for the zstd-magic braces.
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
    /// `vertex_tombstone` records. Added in v0.2; `#[serde(default)]`
    /// means a v0.1 trailer still deserializes with these at zero.
    #[serde(default)]
    pub vertex_tombstones: u64,
    /// `edge_tombstone` records. Added in v0.2.
    #[serde(default)]
    pub edge_tombstones: u64,
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

/// One record in the stream.
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
    /// Deletion of a vertex. Cascades to its edges on import, exactly
    /// as a live delete does.
    VertexTombstone {
        /// The deletion.
        data: Tombstone,
    },
    /// Deletion of an edge.
    EdgeTombstone {
        /// The deletion.
        data: Tombstone,
    },
    /// The stream footer: per-kind counts and the integrity hash.
    /// Exactly one, last line.
    Trailer {
        /// Per-kind record tallies.
        counts: Counts,
        /// Hex sha256 over every preceding uncompressed line.
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
    enc: zstd::stream::write::Encoder<'static, W>,
    hasher: Sha256,
    counts: Counts,
    finished: bool,
}

impl<W: Write> AntWriter<W> {
    /// Compression level 0 = zstd default (currently 3). Levels up to
    /// 19 trade speed for size; text-heavy evidence compresses 5-10x
    /// at the default already.
    pub fn new(out: W, manifest: Manifest, level: i32) -> Result<Self, AntError> {
        let enc = zstd::stream::write::Encoder::new(out, level)?;
        let mut w = Self {
            enc,
            hasher: Sha256::new(),
            counts: Counts::default(),
            finished: false,
        };
        w.write_record(&AntRecord::Manifest(manifest))?;
        Ok(w)
    }

    fn write_record(&mut self, rec: &AntRecord) -> Result<(), AntError> {
        let mut line = serde_json::to_string(rec).map_err(|e| AntError::Json {
            line: 0,
            err: e.to_string(),
        })?;
        line.push('\n');
        self.hasher.update(line.as_bytes());
        self.enc.write_all(line.as_bytes())?;
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
        Ok(self.enc.finish()?)
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
        if manifest.format != "antares" {
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
            observed_at: "2026-08-09T00:00:00Z".parse().unwrap(),
            extracted_at: "2026-08-09T00:00:01Z".parse().unwrap(),
            confidence: Some(0.9),
            evidence_ids: vec![],
            extractor_version: Some("test/1".into()),
            metadata: serde_json::Value::Null,
            author: None,
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
        assert_eq!(got.len(), 3);
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
