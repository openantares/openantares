//! Bitemporal times that may be explicitly unknown (PRODUCT-231).
//!
//! An observation carries two times — the event time (`observed_at`,
//! when the underlying thing happened) and the provenance time
//! (`extracted_at`, when an extractor produced the record). Some
//! genuinely dateless originals have neither: a model claim drawn from
//! prose that names no business date, backed by an extraction receipt
//! but tied to no clock. Before v0.6 those could not be represented at
//! all — both fields were required bare timestamps — so 2,664 such
//! originals were retained in a local journal instead of the graph,
//! leaving 7,990 references to them dangling.
//!
//! [`EventTime`] makes the unknown state first-class. Unknown is **not
//! null and not a sentinel**: it carries a [`UnknownTime`] reason, and
//! no reader may stand `epoch`/`now`/`0` in for it. The vocabulary is
//! reused verbatim from the server's process-mining model
//! (PRODUCT-209 / PR #56, `apps/server/src/mining/model.rs`) rather
//! than invented a second time here.
//!
//! # Wire form — additive over .ant v0.5
//!
//! A dated observation serializes exactly as it did in v0.5, so every
//! v0.5 golden and every v0.5 reader is unchanged for dated records:
//!
//! ```text
//! "observed_at": "2026-08-09T10:00:00Z"                    // Known, no basis
//! "observed_at": {"known":{"at":"2026-08-09T10:00:00Z",     // Known, with a basis
//!                          "basis":"source_record_time"}}
//! "observed_at": {"unknown":{"reason":"no_source_time"}}    // Unknown, the new state
//! ```
//!
//! The three forms are disjoint (a string, an object keyed `known`, an
//! object keyed `unknown`), so the representation is unambiguous in
//! both directions and self-describing.

use chrono::{DateTime, Utc};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Why a time has no usable instant. Each reason is REPORTED, never
/// papered over with a substitute timestamp. Aligned with the server's
/// `UnknownTime` (PRODUCT-209).
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownTime {
    /// The source carries no time at all.
    NoSourceTime,
    /// A statement about a step or fact that names no business date.
    AssertedWithoutDate,
    /// A row describing what is true now. It cannot become a past
    /// transition, so it has no event time of its own.
    CurrentStateOnly,
    /// Several candidate times disagree and none is authoritative.
    AmbiguousSourceTime,
    /// The source stands in for "no date" with a placeholder far
    /// outside any business horizon (`1900-01-01`, `9999-12-31`, …).
    /// A placeholder is not an occurrence.
    ImplausibleSourceTime,
}

impl UnknownTime {
    /// The stable snake_case token, matching the server vocabulary.
    pub fn as_str(self) -> &'static str {
        match self {
            UnknownTime::NoSourceTime => "no_source_time",
            UnknownTime::AssertedWithoutDate => "asserted_without_date",
            UnknownTime::CurrentStateOnly => "current_state_only",
            UnknownTime::AmbiguousSourceTime => "ambiguous_source_time",
            UnknownTime::ImplausibleSourceTime => "implausible_source_time",
        }
    }
}

/// How a KNOWN time was arrived at. Provenance time populated from an
/// extraction receipt carries its basis; a dated event time from a
/// legacy record carries none. Aligned with the server's `TimeBasis`
/// (PRODUCT-209).
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasis {
    /// The source record's own recorded time.
    SourceRecordTime,
    /// A business date the source statement itself carries.
    AssertedValidFrom,
    /// A native source field an operator explicitly bound to time.
    SourceFieldBinding,
}

impl TimeBasis {
    /// The stable snake_case token, matching the server vocabulary.
    pub fn as_str(self) -> &'static str {
        match self {
            TimeBasis::SourceRecordTime => "source_record_time",
            TimeBasis::AssertedValidFrom => "asserted_valid_from",
            TimeBasis::SourceFieldBinding => "source_field_binding",
        }
    }
}

/// A bitemporal time: either a real instant or explicitly unknown with
/// a reason.
///
/// Ordering treats every `Unknown` as greater than every `Known` and
/// orders unknowns by reason — so a time-sorted plane is deterministic
/// and dateless records sort together at one end rather than at
/// `epoch`. Time-window readers must EXCLUDE `Unknown` (it is not in
/// any window), never clamp it to a bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTime {
    /// A real instant, with an optional basis for how it was arrived at.
    Known {
        /// The instant the time refers to.
        at: DateTime<Utc>,
        /// How the instant was arrived at, when a producer recorded it.
        basis: Option<TimeBasis>,
    },
    /// No usable instant; the reason is carried, never a stand-in time.
    Unknown {
        /// Why there is no usable instant.
        reason: UnknownTime,
    },
}

impl EventTime {
    /// A dated time with no recorded basis — the shape every v0.5
    /// observation had.
    pub fn known(at: DateTime<Utc>) -> Self {
        EventTime::Known { at, basis: None }
    }

    /// A dated time whose basis is recorded (provenance from a receipt).
    pub fn known_with(at: DateTime<Utc>, basis: TimeBasis) -> Self {
        EventTime::Known {
            at,
            basis: Some(basis),
        }
    }

    /// An explicitly-unknown time.
    pub fn unknown(reason: UnknownTime) -> Self {
        EventTime::Unknown { reason }
    }

    /// The instant, if this time is known. `None` is UNKNOWN, and a
    /// caller must handle it as unknown — not substitute a default.
    pub fn at(&self) -> Option<DateTime<Utc>> {
        match self {
            EventTime::Known { at, .. } => Some(*at),
            EventTime::Unknown { .. } => None,
        }
    }

    /// Whether this time is a real instant.
    pub fn is_known(&self) -> bool {
        matches!(self, EventTime::Known { .. })
    }

    /// Whether this time is explicitly unknown.
    pub fn is_unknown(&self) -> bool {
        matches!(self, EventTime::Unknown { .. })
    }
}

/// `Known` sorts before `Unknown`; known times sort by instant then
/// nothing else; unknown times sort by reason. Deterministic, and
/// dateless records land together at the high end.
impl PartialOrd for EventTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for EventTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (EventTime::Known { at: a, .. }, EventTime::Known { at: b, .. }) => a.cmp(b),
            (EventTime::Known { .. }, EventTime::Unknown { .. }) => std::cmp::Ordering::Less,
            (EventTime::Unknown { .. }, EventTime::Known { .. }) => std::cmp::Ordering::Greater,
            (EventTime::Unknown { reason: a }, EventTime::Unknown { reason: b }) => a.cmp(b),
        }
    }
}

// ---------------------------------------------------------------------
// Wire form (additive over v0.5): bare string | {known:{..}} | {unknown:{..}}
// ---------------------------------------------------------------------

impl Serialize for EventTime {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            // v0.5-identical: a Known time with no basis is the bare
            // RFC3339 string it always was.
            EventTime::Known { at, basis: None } => at.serialize(s),
            EventTime::Known {
                at,
                basis: Some(basis),
            } => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry(
                    "known",
                    &KnownBody {
                        at: *at,
                        basis: *basis,
                    },
                )?;
                m.end()
            }
            EventTime::Unknown { reason } => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("unknown", &UnknownBody { reason: *reason })?;
                m.end()
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct KnownBody {
    at: DateTime<Utc>,
    basis: TimeBasis,
}

#[derive(Serialize, Deserialize)]
struct UnknownBody {
    reason: UnknownTime,
}

impl<'de> Deserialize<'de> for EventTime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = EventTime;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(
                    "an RFC3339 timestamp string, or a {\"known\":{…}} / {\"unknown\":{…}} object",
                )
            }
            // A bare string is a Known time with no basis — the v0.5 shape.
            fn visit_str<E: de::Error>(self, v: &str) -> Result<EventTime, E> {
                let at = v
                    .parse::<DateTime<Utc>>()
                    .map_err(|e| E::custom(format!("event time `{v}` is not RFC3339: {e}")))?;
                Ok(EventTime::Known { at, basis: None })
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<EventTime, A::Error> {
                let key: String = m
                    .next_key()?
                    .ok_or_else(|| de::Error::custom("event time object is empty"))?;
                match key.as_str() {
                    "known" => {
                        let b: KnownBody = m.next_value()?;
                        Ok(EventTime::Known {
                            at: b.at,
                            basis: Some(b.basis),
                        })
                    }
                    "unknown" => {
                        let b: UnknownBody = m.next_value()?;
                        Ok(EventTime::Unknown { reason: b.reason })
                    }
                    other => Err(de::Error::custom(format!(
                        "event time object key must be `known` or `unknown`, got `{other}`"
                    ))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// Schema shadow: the generated JSON Schema is a `oneOf` of the three
/// wire forms above. Kept in lockstep with the manual serde by shape.
#[cfg(feature = "schemars")]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum EventTimeSchema {
    /// A Known time with no basis — an RFC3339 string.
    BareInstant(DateTime<Utc>),
    /// A Known time with a recorded basis.
    Known { known: KnownBodySchema },
    /// An explicitly-unknown time.
    Unknown { unknown: UnknownBodySchema },
}

#[cfg(feature = "schemars")]
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct KnownBodySchema {
    at: DateTime<Utc>,
    basis: TimeBasis,
}

#[cfg(feature = "schemars")]
#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct UnknownBodySchema {
    reason: UnknownTime,
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for EventTime {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EventTime".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        EventTimeSchema::json_schema(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn known_no_basis_is_the_bare_v0_5_string() {
        let e = EventTime::known(ts("2026-08-09T10:00:00Z"));
        let j = serde_json::to_value(e).unwrap();
        assert_eq!(j, serde_json::json!("2026-08-09T10:00:00Z"));
        // and a bare v0.5 string reads back to exactly that
        let back: EventTime = serde_json::from_value(j).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn known_with_basis_and_unknown_round_trip() {
        let k = EventTime::known_with(ts("2026-08-09T10:00:00Z"), TimeBasis::SourceRecordTime);
        let j = serde_json::to_value(k).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"known":{"at":"2026-08-09T10:00:00Z","basis":"source_record_time"}})
        );
        assert_eq!(serde_json::from_value::<EventTime>(j).unwrap(), k);

        let u = EventTime::unknown(UnknownTime::NoSourceTime);
        let j = serde_json::to_value(u).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"unknown":{"reason":"no_source_time"}})
        );
        assert_eq!(serde_json::from_value::<EventTime>(j).unwrap(), u);
    }

    #[test]
    fn unknown_is_not_a_time_and_sorts_after_known() {
        let u = EventTime::unknown(UnknownTime::AssertedWithoutDate);
        assert_eq!(u.at(), None);
        assert!(u.is_unknown() && !u.is_known());
        let k = EventTime::known(ts("2000-01-01T00:00:00Z"));
        assert!(k < u, "a known instant sorts before any unknown");
        // never epoch: the unknown has no instant to compare as epoch
        assert_ne!(u.at(), Some(DateTime::<Utc>::UNIX_EPOCH));
    }

    #[test]
    fn every_reason_and_basis_token_matches_the_server_vocabulary() {
        for (r, s) in [
            (UnknownTime::NoSourceTime, "no_source_time"),
            (UnknownTime::AssertedWithoutDate, "asserted_without_date"),
            (UnknownTime::CurrentStateOnly, "current_state_only"),
            (UnknownTime::AmbiguousSourceTime, "ambiguous_source_time"),
            (
                UnknownTime::ImplausibleSourceTime,
                "implausible_source_time",
            ),
        ] {
            assert_eq!(r.as_str(), s);
            assert_eq!(serde_json::to_value(r).unwrap(), serde_json::json!(s));
        }
        for (b, s) in [
            (TimeBasis::SourceRecordTime, "source_record_time"),
            (TimeBasis::AssertedValidFrom, "asserted_valid_from"),
            (TimeBasis::SourceFieldBinding, "source_field_binding"),
        ] {
            assert_eq!(b.as_str(), s);
            assert_eq!(serde_json::to_value(b).unwrap(), serde_json::json!(s));
        }
    }
}
