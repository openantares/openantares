//! `PropertyValue` — the typed value of a vertex/edge property.
//!
//! ## Why the wire form is what it is
//!
//! This type used to be `#[serde(untagged)]`: a value serialized as
//! bare JSON and deserialized by trying the variants in order. That is
//! exactly right for `Null | Bool | Long | Float | Text | Json`, whose
//! JSON shapes are already distinct, and it is the shape every existing
//! client, every stored RocksDB record, and every `.ant` file on disk
//! is written in.
//!
//! It cannot express the SQL types. `DECIMAL`, `DATE`, `TIME`,
//! `TIMESTAMP`, `UUID` and `BLOB` all serialize as JSON strings, so an
//! untagged decode of `"2024-03-01"` cannot tell a DATE from a TEXT
//! that happens to look like one — the type is lost on the first
//! round-trip. Storing a type you cannot read back is not parity.
//!
//! So the encoding is split by variant:
//!
//! - **The six legacy variants serialize exactly as before** — bare
//!   `null`, `true`, `42`, `1.5`, `"hi"`, `{...}`. Byte-identical
//!   output, byte-identical parsing. Old records read back unchanged
//!   and old clients see no difference, because for these values there
//!   IS no difference.
//! - **The SQL-parity variants serialize as a tagged envelope**,
//!   `{"$ant":"decimal","v":"12.34"}`. Self-describing, so the type
//!   survives store → `.ant` → store, and unambiguous, so it can never
//!   be confused with a `Text` that looks similar.
//!
//! An object is only read as an envelope when it has exactly the two
//! keys `$ant` and `v` AND `$ant` names a known type. Anything else is
//! a `Json` value, so a caller's own document containing a `$ant` field
//! is still stored as their document (there is a test for precisely
//! this).
//!
//! ## The compatibility surface
//!
//! `/public/v1` is the OpenSPG/KAG-compatible API and must keep
//! emitting what it always emitted, so it projects through
//! [`PropertyValue::to_compat_json`], which flattens the typed variants
//! back to the bare scalars they would have been stored as before this
//! existed. The Antares-native API uses [`PropertyValue::to_json`] and
//! sees the real types.

use std::cmp::Ordering;

use base64::Engine as _;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::decimal::Decimal;

/// The key that marks a tagged envelope.
const TAG: &str = "$ant";
/// The key holding an envelope's payload.
const VAL: &str = "v";

/// Typed property value, at SQL fidelity.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    // ---- Legacy scalars: bare JSON on the wire, unchanged forever ----
    /// SQL NULL / JSON `null`.
    Null,
    /// Boolean.
    Bool(bool),
    /// SQL BIGINT.
    Long(i64),
    /// SQL DOUBLE PRECISION. Already a double — the gap this type had
    /// was never width, it was exactness, and `Decimal` fills that.
    Float(f64),
    /// Free-form text.
    Text(String),
    /// Arbitrary nested JSON (JSONB columns, document subdocuments).
    Json(Value),

    // ---- SQL parity: tagged envelope on the wire ----
    /// SQL INT/INTEGER. Distinct from `Long` so a 32-bit column does
    /// not silently come back 64-bit on round-trip.
    Int32(i32),
    /// SQL SMALLINT.
    Int16(i16),
    /// SQL DECIMAL/NUMERIC and money. Exact — never `f64`.
    Decimal(Decimal),
    /// SQL DATE.
    Date(NaiveDate),
    /// SQL TIME.
    Time(NaiveTime),
    /// SQL TIMESTAMP WITH TIME ZONE. The offset is part of the value
    /// and is preserved verbatim: `+02:00` does not come back as `Z`.
    Timestamp(DateTime<FixedOffset>),
    /// SQL UUID/UNIQUEIDENTIFIER.
    Uuid(uuid::Uuid),
    /// SQL BLOB/BYTEA/VARBINARY. Base64 in every JSON form.
    Bytes(Vec<u8>),
    /// SQL array / document-store array. Elements are themselves typed,
    /// so `Array<Decimal>` stays exact.
    Array(Vec<PropertyValue>),
}

impl PropertyValue {
    /// The `&str` inside a `Text` value; `None` for every other variant.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }

    /// The tag name used in the envelope and in error/type reporting.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Long(_) => "long",
            Self::Float(_) => "float",
            Self::Text(_) => "text",
            Self::Json(_) => "json",
            Self::Int32(_) => "int32",
            Self::Int16(_) => "int16",
            Self::Decimal(_) => "decimal",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
            Self::Timestamp(_) => "timestamp",
            Self::Uuid(_) => "uuid",
            Self::Bytes(_) => "bytes",
            Self::Array(_) => "array",
        }
    }

    /// Base64 alphabet used for `Bytes` everywhere (standard, padded).
    fn b64() -> base64::engine::general_purpose::GeneralPurpose {
        base64::engine::general_purpose::STANDARD
    }

    /// Canonical text of an envelope payload, for the variants whose
    /// payload is a string.
    fn payload(&self) -> Option<Value> {
        Some(match self {
            Self::Int32(i) => Value::from(*i),
            Self::Int16(i) => Value::from(*i),
            Self::Decimal(d) => Value::String(d.to_string()),
            Self::Date(d) => Value::String(d.format("%Y-%m-%d").to_string()),
            // Fixed 6-digit fraction so lexicographic order matches
            // chronological order for anyone sorting the raw strings.
            Self::Time(t) => Value::String(t.format("%H:%M:%S%.6f").to_string()),
            Self::Timestamp(ts) => Value::String(ts.to_rfc3339()),
            Self::Uuid(u) => Value::String(u.to_string()),
            Self::Bytes(b) => Value::String(Self::b64().encode(b)),
            Self::Array(items) => Value::Array(items.iter().map(Self::to_json).collect()),
            _ => return None,
        })
    }

    /// The type-PRESERVING JSON form: what serde emits, what RocksDB
    /// stores, and what a `.ant` file carries. Legacy variants are bare
    /// scalars; SQL-parity variants are envelopes.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => Value::Bool(*b),
            Self::Long(i) => Value::from(*i),
            Self::Float(f) => serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Self::Text(s) => Value::String(s.clone()),
            Self::Json(v) => v.clone(),
            other => {
                let mut o = serde_json::Map::with_capacity(2);
                o.insert(TAG.into(), Value::String(other.type_name().into()));
                o.insert(VAL.into(), other.payload().expect("parity variant"));
                Value::Object(o)
            }
        }
    }

    /// The BACKWARD-COMPATIBLE JSON form for `/public/v1`: every typed
    /// value flattened to the bare scalar it would have been stored as
    /// before SQL parity existed.
    ///
    /// Deliberately lossy. The KAG-compatible surface has a published
    /// shape and a generation of clients that parse it; emitting an
    /// envelope there would widen the contract for every caller, in
    /// exchange for a type they never asked for. Callers who want the
    /// types use the Antares-native API.
    pub fn to_compat_json(&self) -> Value {
        match self {
            // Ints widen to JSON numbers, as they did when the schema
            // carried the width and the value was stored as Long.
            Self::Int32(i) => Value::from(*i),
            Self::Int16(i) => Value::from(*i),
            // Decimal stayed a canonical string, and must keep doing so
            // — rendering it as a JSON number here would hand the
            // f64 corruption straight back to the client.
            Self::Decimal(d) => Value::String(d.to_string()),
            Self::Date(_) | Self::Time(_) | Self::Timestamp(_) | Self::Uuid(_) | Self::Bytes(_) => {
                self.payload().expect("string-payload variant")
            }
            Self::Array(items) => Value::Array(items.iter().map(Self::to_compat_json).collect()),
            legacy => legacy.to_json(),
        }
    }

    /// Read the type-preserving form back. Unknown/!malformed envelopes
    /// fall through to `Json`, so no input is ever rejected here — this
    /// runs against stored data, where refusing to decode would mean
    /// losing a record that is already durable.
    pub fn from_json(v: Value) -> Self {
        match v {
            Value::Null => Self::Null,
            Value::Bool(b) => Self::Bool(b),
            Value::Number(n) => n
                .as_i64()
                .map(Self::Long)
                .or_else(|| n.as_f64().map(Self::Float))
                .unwrap_or(Self::Null),
            Value::String(s) => Self::Text(s),
            Value::Array(_) => Self::Json(v),
            Value::Object(ref o) => match Self::from_object(o) {
                Some(typed) => typed,
                None => Self::Json(v),
            },
        }
    }

    /// `Some` only for a well-formed envelope: exactly `{$ant, v}` with
    /// a known tag and a payload that parses. Anything else is the
    /// caller's own JSON document and must be preserved as such.
    fn from_object(o: &serde_json::Map<String, Value>) -> Option<Self> {
        if o.len() != 2 {
            return None;
        }
        let tag = o.get(TAG)?.as_str()?;
        let v = o.get(VAL)?;
        let text = || v.as_str();
        Some(match tag {
            "int32" => Self::Int32(i32::try_from(v.as_i64()?).ok()?),
            "int16" => Self::Int16(i16::try_from(v.as_i64()?).ok()?),
            "decimal" => Self::Decimal(Decimal::parse(text()?)?),
            "date" => Self::Date(NaiveDate::parse_from_str(text()?, "%Y-%m-%d").ok()?),
            "time" => Self::Time(parse_time(text()?)?),
            "timestamp" => Self::Timestamp(DateTime::parse_from_rfc3339(text()?).ok()?),
            "uuid" => Self::Uuid(uuid::Uuid::parse_str(text()?).ok()?),
            "bytes" => Self::Bytes(Self::b64().decode(text()?).ok()?),
            "array" => Self::Array(v.as_array()?.iter().cloned().map(Self::from_json).collect()),
            _ => return None,
        })
    }
}

/// `HH:MM:SS`, with or without a fractional part.
fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S"))
        .ok()
}

impl Serialize for PropertyValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}

impl<'de> Deserialize<'de> for PropertyValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_json(Value::deserialize(d)?))
    }
}

// ---------------------------------------------------------------------
// Ordering
// ---------------------------------------------------------------------

/// Cross-type sort rank. A value that can be stored but not compared is
/// a trap — a WHERE clause silently matching nothing, an ORDER BY that
/// drops rows — so EVERY variant is comparable against every other, and
/// values of unlike types fall back to this ordering rather than being
/// declared incomparable.
fn rank(v: &PropertyValue) -> u8 {
    match v {
        PropertyValue::Null => 0,
        PropertyValue::Bool(_) => 1,
        // Every numeric shares a rank so they compare by value.
        PropertyValue::Long(_)
        | PropertyValue::Int32(_)
        | PropertyValue::Int16(_)
        | PropertyValue::Float(_)
        | PropertyValue::Decimal(_) => 2,
        PropertyValue::Date(_) => 3,
        PropertyValue::Time(_) => 4,
        PropertyValue::Timestamp(_) => 5,
        PropertyValue::Text(_) => 6,
        PropertyValue::Uuid(_) => 7,
        PropertyValue::Bytes(_) => 8,
        PropertyValue::Array(_) => 9,
        PropertyValue::Json(_) => 10,
    }
}

/// The integer value of any integral variant.
fn as_int(v: &PropertyValue) -> Option<i128> {
    Some(match v {
        PropertyValue::Long(i) => *i as i128,
        PropertyValue::Int32(i) => *i as i128,
        PropertyValue::Int16(i) => *i as i128,
        _ => return None,
    })
}

/// Any numeric as an exact decimal. `Float` is excluded on purpose:
/// binary floating point has no exact decimal form, so mixing it in
/// here would fake an exactness that isn't there.
fn as_exact(v: &PropertyValue) -> Option<Decimal> {
    match v {
        PropertyValue::Decimal(d) => Some(*d),
        other => Decimal::from_parts(as_int(other)?, 0),
    }
}

fn as_f64(v: &PropertyValue) -> Option<f64> {
    match v {
        PropertyValue::Float(f) => Some(*f),
        PropertyValue::Decimal(d) => d.to_string().parse().ok(),
        other => as_int(other).map(|i| i as f64),
    }
}

impl PropertyValue {
    /// Total ordering used by ORDER BY and by range comparisons.
    ///
    /// Within a rank the comparison is type-appropriate: integers and
    /// decimals compare EXACTLY (a decimal never round-trips through
    /// `f64` to be compared), timestamps compare as instants so a
    /// `+02:00` value orders correctly against a `Z` one, bytes compare
    /// lexicographically, and arrays compare element-wise then by
    /// length.
    pub fn cmp_value(&self, other: &Self) -> Ordering {
        use PropertyValue as P;
        match (self, other) {
            (P::Bool(a), P::Bool(b)) => a.cmp(b),
            (P::Text(a), P::Text(b)) => a.cmp(b),
            (P::Date(a), P::Date(b)) => a.cmp(b),
            (P::Time(a), P::Time(b)) => a.cmp(b),
            // DateTime<FixedOffset> compares by instant, which is the
            // SQL TIMESTAMPTZ rule.
            (P::Timestamp(a), P::Timestamp(b)) => a.cmp(b),
            (P::Uuid(a), P::Uuid(b)) => a.cmp(b),
            (P::Bytes(a), P::Bytes(b)) => a.cmp(b),
            (P::Array(a), P::Array(b)) => a
                .iter()
                .zip(b.iter())
                .map(|(x, y)| x.cmp_value(y))
                .find(|o| *o != Ordering::Equal)
                .unwrap_or_else(|| a.len().cmp(&b.len())),
            (P::Json(a), P::Json(b)) => a.to_string().cmp(&b.to_string()),
            _ if rank(self) == rank(other) => {
                // Numeric. Prefer the exact path; fall back to f64 only
                // when a Float is actually involved.
                match (as_exact(self), as_exact(other)) {
                    (Some(a), Some(b)) => a.cmp_value(&b),
                    _ => match (as_f64(self), as_f64(other)) {
                        (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
                        _ => Ordering::Equal,
                    },
                }
            }
            _ => rank(self).cmp(&rank(other)),
        }
    }

    /// Whether this carries one of the SQL-parity types — i.e. a value
    /// whose wire form is a tagged envelope.
    pub fn is_sql_typed(&self) -> bool {
        !matches!(
            self,
            Self::Null
                | Self::Bool(_)
                | Self::Long(_)
                | Self::Float(_)
                | Self::Text(_)
                | Self::Json(_)
        )
    }

    /// Reinterpret an untyped literal as the type of `like`.
    ///
    /// A query says `WHERE due = "2024-03-01"` or `WHERE amount >
    /// "10.50"`. The literal arrives as `Text` — the query language has
    /// no date or decimal literal — so comparing it against a stored
    /// `Date` or `Decimal` by cross-type rank would silently match
    /// nothing. This pulls the literal to the stored value's type so the
    /// comparison is the one the caller meant.
    ///
    /// Returns `self` unchanged when the coercion does not apply or the
    /// literal does not parse as that type; the comparison then falls
    /// back to cross-type ordering rather than inventing a match.
    pub fn coerce_like(&self, like: &Self) -> Self {
        // Already the same kind, or nothing to coerce toward.
        if rank(self) == rank(like) || !like.is_sql_typed() {
            return self.clone();
        }
        let text = match self {
            Self::Text(s) => s.clone(),
            // Numeric literals reach a decimal column as numbers.
            Self::Long(i) => i.to_string(),
            Self::Float(f) => f.to_string(),
            _ => return self.clone(),
        };
        let coerced = match like {
            Self::Decimal(_) => Decimal::parse(&text).map(Self::Decimal),
            Self::Date(_) => NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                .ok()
                .map(Self::Date),
            Self::Time(_) => parse_time(&text).map(Self::Time),
            Self::Timestamp(_) => DateTime::parse_from_rfc3339(&text)
                .ok()
                .map(Self::Timestamp),
            Self::Uuid(_) => uuid::Uuid::parse_str(&text).ok().map(Self::Uuid),
            Self::Bytes(_) => Self::b64().decode(&text).ok().map(Self::Bytes),
            _ => None,
        };
        coerced.unwrap_or_else(|| self.clone())
    }

    /// Equality by VALUE rather than by representation: `Long(1)`
    /// equals `Int32(1)` equals `Decimal("1.0")`, and a `+02:00`
    /// timestamp equals the same instant written as `Z`. This is what a
    /// filter comparison uses; derived `PartialEq` stays
    /// representation-exact for tests and dedup.
    pub fn eq_value(&self, other: &Self) -> bool {
        self.cmp_value(other) == Ordering::Equal && rank(self) == rank(other)
    }
}

impl PartialOrd for PropertyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp_value(other))
    }
}

// ---------------------------------------------------------------------
// OpenAPI
// ---------------------------------------------------------------------

/// Concrete `oneOf` so a generated client sees real types instead of
/// `any`. Downstream code generators build typed clients off this
/// spec, so each SQL variant is described as its own object schema
/// with its literal tag.
#[cfg(feature = "utoipa")]
impl<'s> utoipa::ToSchema<'s> for PropertyValue {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::Schema>) {
        use utoipa::openapi::schema::{ObjectBuilder, OneOfBuilder, SchemaType};
        use utoipa::openapi::RefOr;

        /// One tagged-envelope arm: `{"$ant":"<tag>","v":<payload>}`.
        fn arm(
            tag: &str,
            payload: RefOr<utoipa::openapi::Schema>,
            desc: &str,
        ) -> RefOr<utoipa::openapi::Schema> {
            ObjectBuilder::new()
                .description(Some(desc.to_string()))
                .property(
                    TAG,
                    ObjectBuilder::new()
                        .schema_type(SchemaType::String)
                        .enum_values(Some([tag])),
                )
                .required(TAG)
                .property(VAL, payload)
                .required(VAL)
                .into()
        }

        /// A scalar envelope payload with an optional OpenAPI format.
        fn envelope(
            tag: &str,
            ty: SchemaType,
            format: Option<&str>,
            desc: &str,
        ) -> RefOr<utoipa::openapi::Schema> {
            let mut v = ObjectBuilder::new().schema_type(ty);
            if let Some(f) = format {
                v = v.format(Some(utoipa::openapi::SchemaFormat::Custom(f.into())));
            }
            arm(tag, v.into(), desc)
        }

        let scalar = |t: SchemaType, desc: &str| -> RefOr<utoipa::openapi::Schema> {
            ObjectBuilder::new()
                .schema_type(t)
                .description(Some(desc.to_string()))
                .into()
        };

        let schema = OneOfBuilder::new()
            .description(Some(
                "A typed property value. Legacy scalars are bare JSON; SQL-parity types \
                 are tagged envelopes of the form {\"$ant\":\"<type>\",\"v\":<payload>}. \
                 The /public/v1 (OpenSPG-compatible) endpoints always emit the bare \
                 scalar form."
                    .to_string(),
            ))
            .item(scalar(SchemaType::String, "SQL TEXT/VARCHAR."))
            .item(scalar(SchemaType::Boolean, "SQL BOOLEAN."))
            .item(scalar(SchemaType::Integer, "SQL BIGINT."))
            .item(scalar(SchemaType::Number, "SQL DOUBLE PRECISION."))
            .item(scalar(SchemaType::Object, "JSON/JSONB document."))
            .item(envelope(
                "int32",
                SchemaType::Integer,
                Some("int32"),
                "SQL INT.",
            ))
            .item(envelope(
                "int16",
                SchemaType::Integer,
                Some("int32"),
                "SQL SMALLINT.",
            ))
            .item(envelope(
                "decimal",
                SchemaType::String,
                None,
                "SQL DECIMAL/NUMERIC. A canonical decimal STRING, never a JSON number: \
                 JSON numbers are parsed as f64 by most clients, which corrupts money.",
            ))
            .item(envelope(
                "date",
                SchemaType::String,
                Some("date"),
                "SQL DATE (YYYY-MM-DD).",
            ))
            .item(envelope(
                "time",
                SchemaType::String,
                None,
                "SQL TIME (HH:MM:SS.ffffff).",
            ))
            .item(envelope(
                "timestamp",
                SchemaType::String,
                Some("date-time"),
                "SQL TIMESTAMP WITH TIME ZONE, RFC3339. The UTC offset is part of the \
                 value and is preserved as sent.",
            ))
            .item(envelope(
                "uuid",
                SchemaType::String,
                Some("uuid"),
                "SQL UUID/UNIQUEIDENTIFIER.",
            ))
            .item(envelope(
                "bytes",
                SchemaType::String,
                Some("byte"),
                "SQL BLOB/BYTEA, base64 (standard alphabet, padded).",
            ))
            // Recursive: the items are PropertyValues, which is what
            // makes a typed array keep its element types in a
            // generated client rather than degrading to `any[]`.
            .item(arm(
                "array",
                utoipa::openapi::ArrayBuilder::new()
                    .items(utoipa::openapi::Ref::from_schema_name("PropertyValue"))
                    .into(),
                "SQL array. Elements are themselves PropertyValues, so a typed array \
                 keeps its element types.",
            ))
            .build();
        (
            "PropertyValue",
            RefOr::T(utoipa::openapi::Schema::OneOf(schema)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: &PropertyValue) -> PropertyValue {
        let s = serde_json::to_string(v).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn legacy_scalars_keep_their_exact_historical_wire_form() {
        // Byte-for-byte what the untagged encoding produced. Stored
        // records and existing clients depend on this.
        for (v, json) in [
            (PropertyValue::Null, "null"),
            (PropertyValue::Bool(true), "true"),
            (PropertyValue::Long(42), "42"),
            (PropertyValue::Float(1.5), "1.5"),
            (PropertyValue::Text("hi".into()), "\"hi\""),
        ] {
            assert_eq!(serde_json::to_string(&v).unwrap(), json);
            assert_eq!(round_trip(&v), v);
        }
        let j = PropertyValue::Json(serde_json::json!({"a":[1,2]}));
        assert_eq!(serde_json::to_string(&j).unwrap(), r#"{"a":[1,2]}"#);
        assert_eq!(round_trip(&j), j);
    }

    #[test]
    fn every_sql_type_round_trips_unchanged() {
        for v in sample_values() {
            assert_eq!(round_trip(&v), v, "{} did not round-trip", v.type_name());
        }
    }

    fn sample_values() -> Vec<PropertyValue> {
        vec![
            PropertyValue::Int32(-2_147_483_648),
            PropertyValue::Int16(-32_768),
            PropertyValue::Decimal(Decimal::parse("12345678901234567.89").unwrap()),
            PropertyValue::Date(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
            PropertyValue::Time(NaiveTime::from_hms_micro_opt(12, 30, 45, 123456).unwrap()),
            PropertyValue::Timestamp(
                DateTime::parse_from_rfc3339("2024-03-01T12:00:00+02:00").unwrap(),
            ),
            PropertyValue::Uuid(
                uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(),
            ),
            PropertyValue::Bytes(vec![0, 1, 2, 253, 254, 255]),
            PropertyValue::Array(vec![
                PropertyValue::Text("a".into()),
                PropertyValue::Long(1),
                PropertyValue::Float(2.5),
            ]),
        ]
    }

    #[test]
    fn money_survives_the_wire_to_the_digit() {
        let exact = "12345678901234567.89";
        let v = PropertyValue::Decimal(Decimal::parse(exact).unwrap());
        let wire = serde_json::to_string(&v).unwrap();
        assert_eq!(wire, r#"{"$ant":"decimal","v":"12345678901234567.89"}"#);
        match round_trip(&v) {
            PropertyValue::Decimal(d) => assert_eq!(d.to_string(), exact),
            other => panic!("became {other:?}"),
        }
    }

    #[test]
    fn timestamp_keeps_its_offset_rather_than_normalizing_to_utc() {
        let v = PropertyValue::Timestamp(
            DateTime::parse_from_rfc3339("2024-03-01T12:00:00+02:00").unwrap(),
        );
        assert!(serde_json::to_string(&v).unwrap().contains("+02:00"));
        match round_trip(&v) {
            PropertyValue::Timestamp(t) => {
                assert_eq!(t.to_rfc3339(), "2024-03-01T12:00:00+02:00");
                assert_eq!(t.offset().local_minus_utc(), 7200);
            }
            other => panic!("became {other:?}"),
        }
    }

    #[test]
    fn a_document_containing_the_tag_key_is_still_a_document() {
        // The envelope guard: three keys, so not an envelope.
        let doc = serde_json::json!({"$ant": "decimal", "v": "1.0", "mine": true});
        assert_eq!(
            PropertyValue::from_json(doc.clone()),
            PropertyValue::Json(doc)
        );
        // Two keys but an unknown tag.
        let unknown = serde_json::json!({"$ant": "wat", "v": 1});
        assert_eq!(
            PropertyValue::from_json(unknown.clone()),
            PropertyValue::Json(unknown)
        );
        // Two keys, known tag, unparseable payload -> preserved, not lost.
        let bad = serde_json::json!({"$ant": "date", "v": "not-a-date"});
        assert_eq!(
            PropertyValue::from_json(bad.clone()),
            PropertyValue::Json(bad)
        );
    }

    #[test]
    fn compat_json_flattens_to_the_pre_parity_shape() {
        use serde_json::json;
        let cases = [
            (PropertyValue::Int32(7), json!(7)),
            (PropertyValue::Int16(7), json!(7)),
            (
                PropertyValue::Decimal(Decimal::parse("12.34").unwrap()),
                json!("12.34"),
            ),
            (
                PropertyValue::Date(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
                json!("2024-03-01"),
            ),
            (
                PropertyValue::Uuid(
                    uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(),
                ),
                json!("6ba7b810-9dad-11d1-80b4-00c04fd430c8"),
            ),
            (PropertyValue::Bytes(vec![1, 2, 3]), json!("AQID")),
        ];
        for (v, want) in cases {
            assert_eq!(v.to_compat_json(), want, "{}", v.type_name());
            // And no envelope ever escapes onto the compat surface.
            assert!(!v.to_compat_json().to_string().contains(TAG));
        }
        // Arrays flatten element-wise.
        let arr = PropertyValue::Array(vec![PropertyValue::Int32(1), PropertyValue::Int32(2)]);
        assert_eq!(arr.to_compat_json(), json!([1, 2]));
    }

    #[test]
    fn numerics_compare_exactly_across_widths() {
        let long = PropertyValue::Long(10);
        let i32v = PropertyValue::Int32(10);
        let i16v = PropertyValue::Int16(10);
        let dec = PropertyValue::Decimal(Decimal::parse("10.00").unwrap());
        for a in [&long, &i32v, &i16v, &dec] {
            for b in [&long, &i32v, &i16v, &dec] {
                assert_eq!(a.cmp_value(b), Ordering::Equal, "{a:?} vs {b:?}");
            }
        }
        assert_eq!(
            PropertyValue::Int32(9).cmp_value(&PropertyValue::Long(10)),
            Ordering::Less
        );
        // Exactness past the f64 wall: these two differ only in the
        // 19th digit, which f64 cannot see.
        let a = PropertyValue::Decimal(Decimal::parse("100000000000000000.01").unwrap());
        let b = PropertyValue::Decimal(Decimal::parse("100000000000000000.02").unwrap());
        assert_eq!(a.cmp_value(&b), Ordering::Less);
    }

    #[test]
    fn each_type_orders_within_itself() {
        let d = |s: &str| PropertyValue::Date(NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap());
        assert_eq!(d("2024-01-01").cmp_value(&d("2024-06-01")), Ordering::Less);

        let t = |s: &str| PropertyValue::Time(parse_time(s).unwrap());
        assert_eq!(t("01:00:00").cmp_value(&t("23:59:59")), Ordering::Less);

        // Same instant, different offsets: equal, not ordered by text.
        let ts = |s: &str| PropertyValue::Timestamp(DateTime::parse_from_rfc3339(s).unwrap());
        assert_eq!(
            ts("2024-03-01T12:00:00+02:00").cmp_value(&ts("2024-03-01T10:00:00Z")),
            Ordering::Equal
        );
        assert_eq!(
            ts("2024-03-01T12:00:00+02:00").cmp_value(&ts("2024-03-01T12:00:00Z")),
            Ordering::Less
        );

        assert_eq!(
            PropertyValue::Bytes(vec![1, 2]).cmp_value(&PropertyValue::Bytes(vec![1, 3])),
            Ordering::Less
        );
        assert_eq!(
            PropertyValue::Bool(false).cmp_value(&PropertyValue::Bool(true)),
            Ordering::Less
        );
        assert_eq!(
            PropertyValue::Text("a".into()).cmp_value(&PropertyValue::Text("b".into())),
            Ordering::Less
        );
        // Arrays: element-wise, then length.
        let arr =
            |v: Vec<i64>| PropertyValue::Array(v.into_iter().map(PropertyValue::Long).collect());
        assert_eq!(arr(vec![1, 2]).cmp_value(&arr(vec![1, 3])), Ordering::Less);
        assert_eq!(arr(vec![1]).cmp_value(&arr(vec![1, 0])), Ordering::Less);
    }

    #[test]
    fn every_variant_is_comparable_against_every_other() {
        // A value that can be stored but not compared is a trap: it
        // makes a filter silently match nothing. Total order, no panic,
        // no "incomparable".
        let all: Vec<PropertyValue> = std::iter::once(PropertyValue::Null)
            .chain([
                PropertyValue::Bool(true),
                PropertyValue::Long(1),
                PropertyValue::Float(1.0),
                PropertyValue::Text("x".into()),
                PropertyValue::Json(serde_json::json!({})),
            ])
            .chain(sample_values())
            .collect();
        for a in &all {
            for b in &all {
                let ab = a.cmp_value(b);
                assert_eq!(ab.reverse(), b.cmp_value(a), "asymmetric: {a:?} vs {b:?}");
            }
            assert_eq!(a.cmp_value(a), Ordering::Equal);
        }
        // And sorting the whole mixed set terminates in a stable order.
        let mut sorted = all.clone();
        sorted.sort_by(|a, b| a.cmp_value(b));
        assert_eq!(sorted.len(), all.len());
    }

    #[test]
    fn eq_value_is_by_value_but_not_across_kinds() {
        assert!(PropertyValue::Long(1).eq_value(&PropertyValue::Int32(1)));
        assert!(PropertyValue::Decimal(Decimal::parse("1.0").unwrap())
            .eq_value(&PropertyValue::Long(1)));
        // Different kinds that merely share a rank neighbour must not
        // collapse: "1" is not 1.
        assert!(!PropertyValue::Text("1".into()).eq_value(&PropertyValue::Long(1)));
    }

    #[test]
    fn a_query_literal_is_pulled_to_the_stored_type() {
        // `WHERE due = "2024-03-01"` against a DATE column.
        let stored = PropertyValue::Date(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
        let lit = PropertyValue::Text("2024-03-01".into());
        assert!(lit.coerce_like(&stored).eq_value(&stored));

        // `WHERE amount > "10.50"` against a DECIMAL column — and the
        // comparison must be exact, not lexicographic ("9" > "10").
        let amount = PropertyValue::Decimal(Decimal::parse("10.50").unwrap());
        let nine = PropertyValue::Text("9".into()).coerce_like(&amount);
        assert_eq!(nine.cmp_value(&amount), Ordering::Less);

        // A numeric literal against a decimal column.
        assert!(PropertyValue::Long(10)
            .coerce_like(&amount)
            .cmp_value(&amount)
            .is_lt());

        // Timestamps compare as instants even when written differently.
        let ts =
            PropertyValue::Timestamp(DateTime::parse_from_rfc3339("2024-03-01T10:00:00Z").unwrap());
        let other = PropertyValue::Text("2024-03-01T12:00:00+02:00".into()).coerce_like(&ts);
        assert!(other.eq_value(&ts));

        // A literal that is not that type is left alone rather than
        // being made to match something it isn't.
        let junk = PropertyValue::Text("not-a-date".into());
        assert_eq!(junk.coerce_like(&stored), junk);
        assert!(!junk.coerce_like(&stored).eq_value(&stored));
    }

    #[test]
    fn bytes_survive_arbitrary_binary() {
        let raw: Vec<u8> = (0u8..=255).collect();
        let v = PropertyValue::Bytes(raw.clone());
        match round_trip(&v) {
            PropertyValue::Bytes(b) => assert_eq!(b, raw),
            other => panic!("became {other:?}"),
        }
    }

    #[test]
    fn nested_arrays_keep_element_types() {
        let v = PropertyValue::Array(vec![
            PropertyValue::Array(vec![PropertyValue::Decimal(
                Decimal::parse("0.10").unwrap(),
            )]),
            PropertyValue::Uuid(uuid::Uuid::nil()),
        ]);
        assert_eq!(round_trip(&v), v);
    }
}
