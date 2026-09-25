//! Record types of the open Antares (`.ant`) interchange format.
//!
//! Everything that can appear in a `.ant` file lives here, one module
//! per record plane:
//!
//! - [`graph`] — [`Vertex`] and [`Edge`], with typed properties and
//!   optional bitemporal validity.
//! - [`observation`] — append-only, source-bound atomic facts. Each
//!   carries two times — when the thing happened and when it was
//!   extracted — and both are an [`EventTime`].
//! - [`evidence`] — the source material observations and edges cite.
//!   A primary Evidence may name its stored original ([`SourceBlob`]),
//!   where that original came from ([`SourceReference`]), and cleaned
//!   text may be bound to the exact original it was normalized from
//!   ([`Derivation`]) (v1.0).
//! - [`belief`] — versioned inferred state derived from observations.
//! - [`contradiction`] — cases comparing two or more exact claim
//!   revisions, with three independent state families (v0.4).
//! - [`proposal`] — relationship proposals: what the reconnaissance
//!   loop proposed, what it measured, and what was decided (v0.5).
//! - [`event_time`] — [`EventTime`], [`TimeBasis`] and [`UnknownTime`]:
//!   an observation time that is known, optionally with the basis it
//!   was read from, or explicitly unknown with a reason — never null
//!   and never a sentinel (v0.6).
//! - [`ontology`] — [`OntologyRevision`]: immutable elected semantic
//!   manifests (v0.7).
//! - [`schema`] — OpenSPG-compatible type declarations.
//! - [`author`] — the provenance stamp records can carry.
//! - [`exact_json`] — exact decoding of the two v1.0 opaque fields
//!   (a derivation's `segment.coverage` and a source reference's
//!   `source`), so a double is never rounded on the way in.
//!
//! Property values are typed at SQL fidelity ([`PropertyValue`]).
//! Legacy scalars stay bare JSON on the wire; the typed additions
//! (decimal, date, time, timestamp, uuid, bytes, sized ints, arrays)
//! travel in a tagged `{"$ant": ..., "v": ...}` envelope, so a plain
//! JSON document with a `$ant` field still round-trips as a document.
//! [`Decimal`] is exact — an i128 of unscaled digits plus a scale,
//! never `f64` — so `DECIMAL`/`NUMERIC` columns survive digit for
//! digit.
//!
//! The container that carries these records (compression, manifest,
//! trailer, hashing) is the `antares-format` crate; this crate is just
//! the record vocabulary.

pub mod author;
pub mod belief;
pub mod contradiction;
pub mod decimal;
pub mod error;
pub mod event_time;
pub mod evidence;
pub mod exact_json;
pub mod graph;
pub mod ids;
pub mod observation;
pub mod ontology;
pub mod property;
pub mod proposal;
pub mod schema;

pub use author::{AuthorStamp, SubjectType, TokenId, UserId};
pub use belief::{Belief, BeliefId};
pub use contradiction::{
    BusinessImpact, CaseReferences, CaseRevisionId, ClaimKind, ClaimRef, ComparatorIdentity,
    ContradictionCase, ContradictionCaseId, EpistemicState, EvidenceRef, Material, MeasurementRef,
    SourceDependency, SourcePointer, VaultOccurrence, WorkflowState,
};
pub use decimal::Decimal;
pub use error::CoreError;
pub use event_time::{EventTime, TimeBasis, UnknownTime};
pub use evidence::{
    Derivation, DerivationSegment, Evidence, EvidenceId, Normalizer, SourceBlob, SourceReference,
    NORMALIZED_TEXT_CONTRACT,
};
pub use graph::{Edge, PropertyValue, Vertex};
pub use ids::{EdgeId, Namespace, ProjectId, TenantId, TypeName, VertexId};
pub use observation::{ConditionalRevision, Observation, ObservationId};
pub use ontology::*;
pub use proposal::{
    CastTarget, Normalization, NormalizationOp, ProbeRef, ProposalOrigin, ProposalReferences,
    ProposalRevisionId, ProposalStatus, ProposedRelation, RelationSupport, RelationshipProposal,
    RelationshipProposalId, ReviewerReceipt, Sampling, SamplingFieldRule, SamplingMethod,
    SourceManifestRef, SupportMethod, SUPPORT_CONTRACT_VERSION,
};
pub use schema::*;
