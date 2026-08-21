//! Graph data model: Vertex, Edge, PropertyValue, SubGraph.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::evidence::EvidenceId;
use crate::ids::{EdgeId, TypeName, VertexId};

pub use crate::property::PropertyValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    pub name: String,
    /// Namespace-qualified, e.g. "Antares.Deal".
    pub label: TypeName,
    pub properties: BTreeMap<String, PropertyValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub src: VertexId,
    pub src_type: TypeName,
    pub dst: VertexId,
    pub dst_type: TypeName,
    /// Relation name (e.g. "hasStakeholder"), NOT namespace-qualified.
    pub label: String,
    pub properties: BTreeMap<String, PropertyValue>,

    // --- Antares-native bitemporal annotations ---
    //
    // None on valid_from = valid from -infinity (always was).
    // None on valid_to   = still holds (no end).
    // observed_at        = wall-clock time we recorded this fact.
    // extracted_at       = wall-clock time the extractor produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_at: Option<DateTime<Utc>>,

    /// Confidence in [0,1] for the fact. `None` is treated as 1.0 for
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubGraph {
    pub nodes: Vec<Vertex>,
    pub edges: Vec<Edge>,
}
