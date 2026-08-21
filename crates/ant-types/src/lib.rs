//! Record types of the open Antares format.
//!
//! This is the publishable subset of the engine's domain model: the
//! things that appear IN a `.ant` file and nothing else. `antares-core`
//! re-exports all of it, so engine code is unaffected — but the split
//! is what lets `ant-types` + `antares-format` be published to
//! `openantares/ant` without dragging along engine-internal concerns,
//! which stay private.
//!
//! Apache-2.0, and free of `utoipa` unless the `utoipa` feature is
//! enabled by the engine.

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
