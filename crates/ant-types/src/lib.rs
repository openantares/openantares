//! Record types of the open Antares (`.ant`) interchange format.
//!
//! Everything that can appear in a `.ant` file lives here, one module
//! per record plane:
//!
//! - [`graph`] — [`Vertex`] and [`Edge`], with typed properties and
//!   optional bitemporal validity.
//! - [`observation`] — append-only, source-bound atomic facts.
//! - [`evidence`] — the source material observations and edges cite.
//! - [`belief`] — versioned inferred state derived from observations.
//! - [`schema`] — OpenSPG-compatible type declarations.
//! - [`author`] — the provenance stamp records can carry.
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

#![warn(missing_docs)]

pub mod author;
pub mod belief;
pub mod decimal;
pub mod error;
pub mod evidence;
pub mod graph;
pub mod ids;
pub mod observation;
pub mod property;
pub mod schema;

pub use author::{AuthorStamp, SubjectType, TokenId, UserId};
pub use belief::{Belief, BeliefId};
pub use decimal::Decimal;
pub use error::CoreError;
pub use evidence::{Evidence, EvidenceId};
pub use graph::{Edge, PropertyValue, Vertex};
pub use ids::{EdgeId, Namespace, ProjectId, TenantId, TypeName, VertexId};
pub use observation::{Observation, ObservationId};
pub use schema::*;
