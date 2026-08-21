//! Exact decimal (SQL DECIMAL/NUMERIC, money).
//!
//! `f64` is not a decimal type and never was. It cannot represent `0.1`,
//! and past ~15 significant digits it silently rounds — so an invoice
//! total of `12345678901234567.89` comes back as `12345678901234568`,
//! with no error, no warning, and no way for the caller to tell. That is
//! the single most damaging thing a knowledge graph can do to financial
//! data, so this type never touches binary floating point.
//!
//! Representation is an `i128` of unscaled digits plus a decimal `scale`
//! — `12.3400` is `unscaled = 123400, scale = 4`. Scale is preserved
//! rather than trimmed: in SQL, `DECIMAL(10,4)` carrying `12.3400` is
//! not the same column value as `12.34`, and a round-trip that quietly
//! drops the trailing zeros has changed the data.
//!
//! `i128` holds 38 significant digits, which covers `DECIMAL(38, s)` —
//! the maximum precision of Postgres, MySQL, SQL Server, and Oracle
//! alike. Input beyond that is rejected (`None`) rather than truncated.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An exact base-10 number: `unscaled / 10^scale`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    unscaled: i128,
    scale: u32,
}

/// Beyond this, `unscaled` would overflow `i128`.
const MAX_PRECISION: u32 = 38;

impl Decimal {
    /// Construct from raw parts. `None` if `scale` exceeds the maximum
    /// precision, which would make the value unrepresentable.
    pub fn from_parts(unscaled: i128, scale: u32) -> Option<Self> {
        (scale <= MAX_PRECISION).then_some(Self { unscaled, scale })
    }

    pub fn unscaled(&self) -> i128 {
        self.unscaled
    }

    /// Digits after the decimal point, as declared. Preserved exactly:
    /// `12.3400` reports 4, not 2.
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Total significant digits — the `p` of `DECIMAL(p, s)`. Zero has
    /// precision 1.
    pub fn precision(&self) -> u32 {
        let mut n = self.unscaled.unsigned_abs();
        if n == 0 {
            return 1;
        }
        let mut digits = 0;
        while n > 0 {
            digits += 1;
            n /= 10;
        }
        // A pure fraction still needs its leading zero counted against
        // the scale: 0.004 is DECIMAL(3,3), not DECIMAL(1,3).
        digits.max(self.scale)
    }

    pub fn is_zero(&self) -> bool {
        self.unscaled == 0
    }

    /// Parse a decimal literal: optional sign, digits, optional
    /// fraction, optional `e±nn` exponent. The exponent is folded into
    /// the scale, so `1.5e3` parses as `1500` and `15e-3` as `0.015` —
    /// both exact.
    ///
    /// Returns `None` for anything that is not a decimal literal, or
    /// that needs more than 38 significant digits. NEVER falls back to
    /// float parsing: silently accepting a value we cannot represent
    /// exactly is the bug this type exists to prevent.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        // Split off the exponent first.
        let (mantissa, exp) = match s.find(['e', 'E']) {
            Some(i) => {
                let e: i32 = s[i + 1..].parse().ok()?;
                (&s[..i], e)
            }
            None => (s, 0),
        };

        let (neg, digits) = match mantissa.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, mantissa.strip_prefix('+').unwrap_or(mantissa)),
        };

        let (int_part, frac_part) = match digits.find('.') {
            Some(i) => (&digits[..i], &digits[i + 1..]),
            None => (digits, ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_part.bytes().all(|b| b.is_ascii_digit())
            || !frac_part.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }

        // Accumulate every digit; overflow means we cannot be exact.
        let mut unscaled: i128 = 0;
        for b in int_part.bytes().chain(frac_part.bytes()) {
            unscaled = unscaled.checked_mul(10)?.checked_add((b - b'0') as i128)?;
        }

        let scale = i64::from(frac_part.len() as u32) - i64::from(exp);
        let (unscaled, scale) = if scale < 0 {
            // Negative scale: multiply out so the value stays an
            // integer with scale 0 rather than growing a synthetic
            // fraction.
            let mut u = unscaled;
            for _ in 0..(-scale) {
                u = u.checked_mul(10)?;
            }
            (u, 0u32)
        } else {
            (unscaled, u32::try_from(scale).ok()?)
        };
        if scale > MAX_PRECISION {
            return None;
        }

        Some(Self {
            unscaled: if neg { -unscaled } else { unscaled },
            scale,
        })
    }

    /// Rescale to `target` digits after the point. `None` when that
    /// would drop non-zero digits (an inexact narrowing) or overflow.
    pub fn rescale(&self, target: u32) -> Option<Self> {
        if target > MAX_PRECISION {
            return None;
        }
        match target.cmp(&self.scale) {
            Ordering::Equal => Some(*self),
            Ordering::Greater => {
                let mut u = self.unscaled;
                for _ in 0..(target - self.scale) {
                    u = u.checked_mul(10)?;
                }
                Some(Self {
                    unscaled: u,
                    scale: target,
                })
            }
            Ordering::Less => {
                let mut u = self.unscaled;
                for _ in 0..(self.scale - target) {
                    if u % 10 != 0 {
                        return None; // would lose a significant digit
                    }
                    u /= 10;
                }
                Some(Self {
                    unscaled: u,
                    scale: target,
                })
            }
        }
    }

    /// Exact ordering, independent of scale: `1.50` equals `1.5` in
    /// value even though they are distinct column values.
    ///
    /// Compares by aligning scales in `i128`; if alignment would
    /// overflow it falls back to comparing sign, integer-digit count,
    /// and then digits pairwise — which needs no arithmetic at all and
    /// so is exact at any magnitude.
    pub fn cmp_value(&self, other: &Self) -> Ordering {
        let target = self.scale.max(other.scale);
        if let (Some(a), Some(b)) = (self.rescale(target), other.rescale(target)) {
            return a.unscaled.cmp(&b.unscaled);
        }
        digitwise_cmp(self, other)
    }
}

/// Scale-independent comparison that never multiplies, for values too
/// large to align. Compares sign, then magnitude by integer-digit
/// count, then the digit strings pairwise.
fn digitwise_cmp(a: &Decimal, b: &Decimal) -> Ordering {
    let (sa, sb) = (a.unscaled.signum(), b.unscaled.signum());
    if sa != sb {
        return sa.cmp(&sb);
    }
    let flip = sa < 0;
    let (ai, af) = split_digits(a);
    let (bi, bf) = split_digits(b);

    // Integer parts: more digits (after stripping leading zeros) wins.
    let (ai, bi) = (ai.trim_start_matches('0'), bi.trim_start_matches('0'));
    let ord = ai
        .len()
        .cmp(&bi.len())
        .then_with(|| ai.cmp(bi))
        .then_with(|| {
            // Fractions compare left-aligned, zero-padded.
            let n = af.len().max(bf.len());
            let pad = |s: &str| {
                let mut t = s.to_string();
                t.extend(std::iter::repeat_n('0', n - s.len()));
                t
            };
            pad(&af).cmp(&pad(&bf))
        });
    if flip {
        ord.reverse()
    } else {
        ord
    }
}

/// `(integer digits, fraction digits)` of the magnitude, unsigned.
fn split_digits(d: &Decimal) -> (String, String) {
    let digits = d.unscaled.unsigned_abs().to_string();
    let scale = d.scale as usize;
    if digits.len() > scale {
        let (i, f) = digits.split_at(digits.len() - scale);
        (i.to_string(), f.to_string())
    } else {
        let mut f = "0".repeat(scale - digits.len());
        f.push_str(&digits);
        ("0".into(), f)
    }
}

impl fmt::Display for Decimal {
    /// The canonical literal, with exactly `scale` fractional digits —
    /// the round-trip form. `Decimal::parse(&d.to_string()) == Some(d)`
    /// for every representable value, trailing zeros included.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (int, frac) = split_digits(self);
        if self.unscaled < 0 {
            f.write_str("-")?;
        }
        f.write_str(&int)?;
        if self.scale > 0 {
            write!(f, ".{frac}")?;
        }
        Ok(())
    }
}

/// Value ordering, NOT the derived field order — `1.50` and `1.5` are
/// `Ordering::Equal` here while remaining distinct under `Eq`.
impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp_value(other))
    }
}

/// Serialized as its canonical STRING, never a JSON number: a JSON
/// number goes through `f64` in most parsers (including JavaScript),
/// which is exactly the corruption this type exists to prevent.
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let raw = serde_json::Value::deserialize(d)?;
        let text = match &raw {
            serde_json::Value::String(s) => s.clone(),
            // A bare JSON number is accepted ONLY when the parser
            // already held it exactly — i.e. an integer. A fractional
            // literal has been through `f64` before it ever reaches
            // here (serde_json parses it that way), so `1.10` arrives
            // as `1.1` with the scale gone and `0.1` as an approximation
            // of itself. Accepting those would launder precisely the
            // corruption this type exists to prevent, so they are
            // refused and the caller is told to send a string.
            serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
            serde_json::Value::Number(n) => {
                return Err(D::Error::custom(format!(
                    "decimal {n} arrived as a JSON number, which is parsed as f64 and has \
                     already lost precision; send it as a string"
                )))
            }
            other => {
                return Err(D::Error::custom(format!(
                    "decimal must be a string: {other}"
                )))
            }
        };
        Decimal::parse(&text)
            .ok_or_else(|| D::Error::custom(format!("not an exact decimal: {text}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_that_f64_corrupts_survives_to_the_digit() {
        // 19 significant digits: f64 carries ~15-16.
        let s = "12345678901234567.89";
        let d = Decimal::parse(s).unwrap();
        assert_eq!(d.to_string(), s);
        assert_eq!(d.unscaled(), 1_234_567_890_123_456_789);
        assert_eq!(d.scale(), 2);
        // Demonstrate the corruption being avoided.
        assert_ne!(s.parse::<f64>().unwrap().to_string(), s);
    }

    #[test]
    fn scale_is_preserved_not_trimmed() {
        let d = Decimal::parse("12.3400").unwrap();
        assert_eq!(d.scale(), 4);
        assert_eq!(d.to_string(), "12.3400");
        // Equal in VALUE, distinct as column values.
        let e = Decimal::parse("12.34").unwrap();
        assert_eq!(d.cmp_value(&e), Ordering::Equal);
        assert_ne!(d, e);
    }

    #[test]
    fn parses_sign_fraction_and_exponent_exactly() {
        assert_eq!(Decimal::parse("-0.004").unwrap().to_string(), "-0.004");
        assert_eq!(Decimal::parse("1.5e3").unwrap().to_string(), "1500");
        assert_eq!(Decimal::parse("15e-3").unwrap().to_string(), "0.015");
        assert_eq!(Decimal::parse(".5").unwrap().to_string(), "0.5");
        assert_eq!(Decimal::parse("+7").unwrap().to_string(), "7");
    }

    #[test]
    fn rejects_what_it_cannot_represent_exactly() {
        assert!(Decimal::parse("abc").is_none());
        assert!(Decimal::parse("1.2.3").is_none());
        assert!(Decimal::parse("").is_none());
        assert!(Decimal::parse("NaN").is_none());
        // 40 digits > i128's 38.
        assert!(Decimal::parse(&"9".repeat(40)).is_none());
    }

    #[test]
    fn ordering_is_exact_across_scales_and_signs() {
        let ordered = [
            "-100", "-2.5", "-0.001", "0", "0.001", "0.0010", "1.5", "1.50", "2.5", "100",
        ];
        for w in ordered.windows(2) {
            let (a, b) = (Decimal::parse(w[0]).unwrap(), Decimal::parse(w[1]).unwrap());
            assert!(
                a.cmp_value(&b) != Ordering::Greater,
                "{} should sort <= {}",
                w[0],
                w[1]
            );
        }
        // Differing only past the f64 precision wall.
        let a = Decimal::parse("100000000000000000.01").unwrap();
        let b = Decimal::parse("100000000000000000.02").unwrap();
        assert_eq!(a.cmp_value(&b), Ordering::Less);
    }

    #[test]
    fn digitwise_fallback_matches_aligned_compare() {
        // Scales that cannot be aligned without overflowing i128 take
        // the no-arithmetic path; it must agree with the fast path.
        let a = Decimal::from_parts(i128::MAX, 0).unwrap();
        let b = Decimal::from_parts(1, 30).unwrap();
        assert_eq!(a.cmp_value(&b), Ordering::Greater);
        assert_eq!(b.cmp_value(&a), Ordering::Less);
        let c = Decimal::from_parts(-1, 30).unwrap();
        assert_eq!(c.cmp_value(&b), Ordering::Less);
    }

    #[test]
    fn precision_counts_significant_digits() {
        assert_eq!(Decimal::parse("0").unwrap().precision(), 1);
        assert_eq!(Decimal::parse("123.45").unwrap().precision(), 5);
        assert_eq!(Decimal::parse("0.004").unwrap().precision(), 3);
    }

    #[test]
    fn json_is_a_string_and_round_trips() {
        let d = Decimal::parse("12345678901234567.89").unwrap();
        let j = serde_json::to_string(&d).unwrap();
        assert_eq!(j, "\"12345678901234567.89\"");
        assert_eq!(serde_json::from_str::<Decimal>(&j).unwrap(), d);
        // Integers are exact in the parser, so they are accepted.
        assert_eq!(
            serde_json::from_str::<Decimal>("5").unwrap().to_string(),
            "5"
        );
        // Fractional JSON numbers have already been through f64 by the
        // time we see them ("1.10" arrives as 1.1), so they are refused
        // rather than silently accepted at the wrong scale.
        let err = serde_json::from_str::<Decimal>("1.10")
            .unwrap_err()
            .to_string();
        assert!(err.contains("send it as a string"), "{err}");
    }
}
