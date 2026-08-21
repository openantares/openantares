//! Identifier newtypes: tenant, project, vertex, edge, namespace,
//! type name. Each is a thin wrapper so ids of different planes can't
//! be mixed up silently.

use serde::{Deserialize, Serialize};

/// Tenant identifier. Tenants are the top-level isolation boundary.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TenantId(pub u64);

// `#[serde(transparent)]` newtypes serialize as their inner value, so
// the schema has to say `integer` and not `object` — the derive would
// otherwise disagree with the wire.
#[cfg(feature = "utoipa")]
impl<'s> utoipa::ToSchema<'s> for TenantId {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::Schema>) {
        ("TenantId", transparent_u64_schema("Tenant identifier."))
    }
}

/// Project identifier. A project lives under one tenant and has its own
/// schema namespace.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ProjectId(pub u64);

#[cfg(feature = "utoipa")]
impl<'s> utoipa::ToSchema<'s> for ProjectId {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::Schema>) {
        ("ProjectId", transparent_u64_schema("Project identifier."))
    }
}

#[cfg(feature = "utoipa")]
fn transparent_u64_schema(description: &str) -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
    use utoipa::openapi::schema::{ObjectBuilder, SchemaFormat, SchemaType};
    ObjectBuilder::new()
        .schema_type(SchemaType::Integer)
        .format(Some(SchemaFormat::KnownFormat(
            utoipa::openapi::KnownFormat::Int64,
        )))
        .minimum(Some(0.0))
        .description(Some(description))
        .into()
}

/// Project namespace (e.g. "Antares"). Prepended to all type names at the
/// SPG layer (`Antares.Deal`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Namespace(pub String);

/// Namespace-qualified SPG type name, e.g. `Antares.Deal` or `Antares.Chunk`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeName(pub String);

impl TypeName {
    /// Build a namespace-qualified type name from an unqualified label.
    pub fn qualified(ns: &Namespace, label: &str) -> Self {
        if label.contains('.') {
            // Already qualified.
            Self(label.to_owned())
        } else {
            Self(format!("{}.{}", ns.0, label))
        }
    }
}

/// Vertex business id (the `id` field on the wire, e.g. "deal_hooli_001").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VertexId(pub String);

/// Edge id (assigned by the caller in `writerGraph`, e.g. "e1").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_name_prepends_namespace() {
        let ns = Namespace("Antares".into());
        assert_eq!(TypeName::qualified(&ns, "Deal").0, "Antares.Deal");
    }

    #[test]
    fn qualified_name_is_idempotent() {
        let ns = Namespace("Antares".into());
        assert_eq!(TypeName::qualified(&ns, "Antares.Deal").0, "Antares.Deal");
    }
}
