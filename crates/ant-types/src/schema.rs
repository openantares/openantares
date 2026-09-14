//! Schema records: the OpenSPG-compatible type declarations that a
//! `.ant` file can carry as `schema_type` records.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::{Namespace, TypeName};

/// OpenSPG type kind, serialized in SCREAMING_SNAKE_CASE on the wire.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpgTypeKind {
    /// Built-in scalar type (Text, Integer, Float...).
    BasicType,
    /// Reusable constrained value type (e.g. a phone number).
    StandardType,
    /// Entity: a thing with identity and properties.
    EntityType,
    /// Index type in the OpenSPG sense.
    IndexType,
    /// Concept: a taxonomy/category node.
    ConceptType,
    /// Event: something that happened, usually with participants.
    EventType,
}

/// How a property is indexed for retrieval.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IndexKind {
    /// Full-text index only.
    Text,
    /// Vector (embedding) index only.
    Vector,
    /// Both full-text and vector.
    TextAndVector,
}

/// Declared value type of a property.
///
/// The names accepted on the wire are wider than the variants (see
/// [`ValueType::from_object_type_name`]); the variants are the
/// canonical set.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    /// Free-form text.
    Text,
    /// 64-bit integer (SQL BIGINT). Wire names "Integer"/"Long" keep
    /// mapping here for backward compatibility with existing schemas.
    Long,
    /// 64-bit float (SQL DOUBLE PRECISION / FLOAT8).
    Float,
    /// Calendar date (SQL DATE). Values validate + normalize to
    /// `YYYY-MM-DD`; invalid input coerces to Null.
    Date,
    /// Boolean.
    Bool,
    // --- SQL-parity additions (values stay wire-compatible scalars;
    // the schema carries the SQL fidelity, coerce.rs validates and
    // normalizes on ingress) ---
    /// SQL SMALLINT: range-checked to i16 on ingress, stored as Long.
    SmallInt,
    /// SQL INT/INTEGER (32-bit): range-checked to i32, stored as Long.
    /// Wire name "Int32"/"Int" (plain "Integer" stays Long, see above).
    Int32,
    /// SQL DECIMAL/NUMERIC. Values coerce to [`crate::Decimal`] — i128
    /// unscaled digits plus a scale, exact, never `f64`.
    ///
    /// UNPARAMETERIZED, deliberately. The declared `(precision, scale)`
    /// stays in the source-schema mapping rather than here, because the
    /// value already carries its own exact scale and reports its own
    /// precision, which is enough for storage, comparison and
    /// round-tripping. Adding them here would change the serde shape of
    /// this variant from the string `"Decimal"` to a struct, and every
    /// stored schema record and `.ant` schema_type payload is written
    /// in the current shape.
    ///
    /// REVISIT WHEN: the SQL auto-mapper needs to VALIDATE values
    /// against declared column types — rejecting a scale-6 value
    /// written into a `DECIMAL(10,4)` column, rather than storing it at
    /// whatever scale it arrived with. That check cannot be made from
    /// the value alone; it needs the declaration, and at that point the
    /// declaration has to live here. Doing it will need a
    /// backward-compatible deserializer that still accepts the bare
    /// `"Decimal"` string.
    Decimal,
    /// SQL TIME: normalized `HH:MM:SS.ffffff` (fixed 6-digit fraction
    /// so lexicographic order == chronological order).
    Time,
    /// SQL TIMESTAMP/TIMESTAMPTZ: normalized UTC RFC3339 with fixed
    /// 6-digit fraction (`YYYY-MM-DDTHH:MM:SS.ffffffZ`) so lexicographic
    /// order == chronological order. Offset-less input is taken as UTC.
    Timestamp,
    /// UUID/UNIQUEIDENTIFIER: validated 8-4-4-4-12 hex, lowercased.
    Uuid,
    /// BLOB/BYTEA/VARBINARY: base64 text, charset/padding validated.
    Bytes,
    /// JSON/JSONB, document-store subdocuments: stored as real JSON
    /// (PropertyValue::Json), not stringified.
    Json,
    /// Typed array (Postgres arrays, document-store arrays): every
    /// element coerced against the inner type; stored as a JSON array.
    Array(Box<ValueType>),
    /// Reference to another declared type by name.
    Ref(TypeName),
}

impl ValueType {
    /// Parse an OpenSPG basic-type name (e.g. "Text", "Integer", "Float")
    /// or a user-defined type reference.
    pub fn from_object_type_name(name: &str) -> Self {
        // Array<Inner> (any nesting depth) before the flat names.
        if let Some(inner) = name
            .strip_prefix("Array<")
            .and_then(|r| r.strip_suffix('>'))
        {
            return Self::Array(Box::new(Self::from_object_type_name(inner)));
        }
        match name {
            "Text" => Self::Text,
            // Compat: existing schemas declared "Integer" meaning i64.
            "Integer" | "Long" | "BigInt" => Self::Long,
            "Float" | "Double" => Self::Float,
            "Date" => Self::Date,
            "Boolean" | "Bool" => Self::Bool,
            "SmallInt" | "Int16" => Self::SmallInt,
            "Int32" | "Int" => Self::Int32,
            "Decimal" | "Numeric" => Self::Decimal,
            "Time" => Self::Time,
            "Timestamp" | "DateTime" | "TimestampTz" => Self::Timestamp,
            "Uuid" | "UUID" | "Guid" => Self::Uuid,
            "Bytes" | "Binary" | "Blob" => Self::Bytes,
            "Json" | "JSONB" | "Object" => Self::Json,
            // Anything else — treat as a reference to another SPG type.
            other => Self::Ref(TypeName(other.to_string())),
        }
    }
}

/// A property declaration on a [`SchemaType`].
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDef {
    /// Property name as it appears on records.
    pub name: String,
    /// Chinese display name, when the schema source provides one.
    pub name_zh: Option<String>,
    /// Declared value type.
    pub value_type: ValueType,
    /// Index declaration, if the property is indexed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexKind>,
}

/// A relation (edge type) declaration on a [`SchemaType`].
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationDef {
    /// Relation name as it appears on edges.
    pub name: String,
    /// Chinese display name, when the schema source provides one.
    pub name_zh: Option<String>,
    /// Target type the relation points at.
    pub target: TypeName,
    /// Properties carried on the edge itself (rare; commonly empty).
    #[serde(default)]
    pub properties: Vec<PropertyDef>,
}

impl RelationDef {
    /// The edge-property declarations, keyed by property name.
    pub fn properties_lookup(&self) -> std::collections::BTreeMap<&str, &PropertyDef> {
        self.properties
            .iter()
            .map(|p| (p.name.as_str(), p))
            .collect()
    }
}

/// One declared type: kind, qualified name, properties, relations.
///
/// This is the payload of a `schema_type` record in a `.ant` file.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaType {
    /// What kind of OpenSPG type this is.
    pub kind: SpgTypeKind,
    /// Namespace-qualified name, e.g. `Antares.Deal`.
    pub name: TypeName,
    /// Chinese display name if provided by marklang.
    pub name_zh: Option<String>,
    /// Property declarations.
    pub properties: Vec<PropertyDef>,
    /// Relation (edge type) declarations.
    pub relations: Vec<RelationDef>,
}

impl SchemaType {
    /// An empty type declaration of the given kind and name.
    pub fn new(kind: SpgTypeKind, name: TypeName) -> Self {
        Self {
            kind,
            name,
            name_zh: None,
            properties: Vec::new(),
            relations: Vec::new(),
        }
    }
}

/// A whole schema: every declared type, keyed by qualified name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    /// The declared types, keyed by their qualified [`TypeName`].
    pub types: BTreeMap<TypeName, SchemaType>,
}

impl Schema {
    /// Look up a type by qualified name.
    pub fn get(&self, name: &TypeName) -> Option<&SchemaType> {
        self.types.get(name)
    }

    /// Insert (replacing on conflict) a list of `SchemaType`s.
    pub fn upsert_all(&mut self, types: impl IntoIterator<Item = SchemaType>) {
        for t in types {
            self.types.insert(t.name.clone(), t);
        }
    }

    /// All declared relation names that resolve to a real SPG type (i.e.
    /// not BASIC/STANDARD value-typed properties). Used by the planner.
    pub fn relation_names(&self) -> Vec<&str> {
        self.types
            .values()
            .flat_map(|t| t.relations.iter().map(|r| r.name.as_str()))
            .collect()
    }
}

/// Convenience: namespace-qualify an unqualified label (`Deal`) into
/// `Antares.Deal` using the project's namespace.
pub fn qualify(ns: &Namespace, label: &str) -> TypeName {
    TypeName::qualified(ns, label)
}
