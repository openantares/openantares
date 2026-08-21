//! Graph data model: Vertex, Edge, PropertyValue, SubGraph.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::evidence::EvidenceId;
use crate::ids::{EdgeId, TypeName, VertexId};

pub use crate::property::PropertyValue;

/// A graph node: business id, display name, qualified type label,
/// and typed properties. The payload of a `vertex` record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    /// Business id, unique within the type (e.g. `deal_1`).
    pub id: VertexId,
    /// Human-readable display name.
    pub name: String,
    /// Namespace-qualified, e.g. "Antares.Deal".
    pub label: TypeName,
    /// Typed properties, keyed by property name.
    pub properties: BTreeMap<String, PropertyValue>,
}

/// A directed, labeled edge between two vertices, with optional
/// bitemporal validity. The payload of an `edge` record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Edge id, unique within the scope.
    pub id: EdgeId,
    /// Source vertex id.
    pub src: VertexId,
    /// Source vertex type.
    pub src_type: TypeName,
    /// Destination vertex id.
    pub dst: VertexId,
    /// Destination vertex type.
    pub dst_type: TypeName,
    /// Relation name (e.g. "hasStakeholder"), NOT namespace-qualified.
    pub label: String,
    /// Typed properties carried on the edge, keyed by property name.
    pub properties: BTreeMap<String, PropertyValue>,

    // --- Antares-native bitemporal annotations ---
    //
    // None on valid_from = valid from -infinity (always was).
    // None on valid_to   = still holds (no end).
    // observed_at        = wall-clock time the fact was recorded.
    // extracted_at       = wall-clock time the extractor produced it.
    /// Start of real-world validity; `None` = always was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateTime<Utc>>,
    /// End of real-world validity (exclusive); `None` = still holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateTime<Utc>>,
    /// Wall-clock time the fact was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    /// Wall-clock time the extractor produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_at: Option<DateTime<Utc>>,

    /// Confidence in `[0,1]` for the fact. `None` is treated as 1.0 for
    /// matching purposes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// First-class evidence references. Empty by default
    /// (backwards-compatible with older payloads).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidenced_by: Vec<EvidenceId>,
}

impl Edge {
    /// Returns true iff this edge is valid at the given instant.
    ///
    /// Edges with no `valid_from` are treated as valid from -infinity.
    /// Edges with no `valid_to` are treated as still-holding.
    pub fn valid_at(&self, t: DateTime<Utc>) -> bool {
        match (self.valid_from, self.valid_to) {
            (None, None) => true,
            (Some(f), None) => t >= f,
            (None, Some(u)) => t < u,
            (Some(f), Some(u)) => t >= f && t < u,
        }
    }

    /// Returns true iff this edge's validity interval overlaps `[t1, t2)`.
    pub fn valid_between(&self, t1: DateTime<Utc>, t2: DateTime<Utc>) -> bool {
        if t2 <= t1 {
            return false;
        }
        let lo = self.valid_from.unwrap_or(DateTime::<Utc>::MIN_UTC);
        let hi = self.valid_to.unwrap_or(DateTime::<Utc>::MAX_UTC);
        lo < t2 && t1 < hi
    }

    /// True if the edge is currently active (valid_to is None).
    pub fn still_holds(&self) -> bool {
        self.valid_to.is_none()
    }
}

/// A set of vertices and edges, as returned by queries or carried in
/// bulk payloads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubGraph {
    /// The vertices.
    pub nodes: Vec<Vertex>,
    /// The edges.
    pub edges: Vec<Edge>,
}
