//! Exact decoding of the format 1.0 opaque fields: a derivative's
//! `segment.coverage` and a source reference's `source`.
//!
//! The shared serde_json parser this workspace links is built without
//! `float_roundtrip`, so it can read a double's shortest text as a
//! neighbouring double (`51.248178375505404` as `51.24817837550541`). That
//! parser stays as it is: format 0.7 replay equality and the canonical
//! ontology digests depend on it. Only these two 1.0 fields are re-read
//! here, from the record's raw line, with correctly rounded finite doubles
//! (`str::parse::<f64>`), exact integers in [-2^63, 2^64-1], `-0` as -0.0
//! (as serde_json holds it), and a number no double can hold refused.
//!
//! These fields are decoded BEFORE the shared parser sees them: every
//! occurrence of the field's path is replaced with `null` in a scratch copy
//! of the text, the shared typed decoder reads that copy (the fields are
//! `serde_json::Value`, so `null` always decodes), and the exact value is
//! put back. A finite double the shared parser would round, or would refuse
//! as out of range (`17976931348623158e292` is `f64::MAX`), therefore
//! reaches the record exactly. Stored and hashed bytes are never rewritten.
//!
//! Duplicate object keys follow `serde_json::Value`: the LAST occurrence of
//! a key wins, at every level of the path, so the exact value is always the
//! one the typed decode keeps. A blanked span is checked as the shared
//! parser would check it — JSON grammar, and nesting counted from the top of
//! the whole document — so the only documents read here that it would
//! refuse are those whose one fault is a valid number inside these fields.
//! Every reader of these fields uses this module — the `.ant` reader, HTTP
//! input, stored rows and the feed — so none of them rounds a fact the
//! others keep.

use serde::de::DeserializeOwned;
use serde_json::{Map, Number, Value};

/// One step of a path into a JSON document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<'a> {
    /// An object member.
    Key(&'a str),
    /// An array element.
    Index(usize),
}

/// serde_json's nesting bound: it refuses the 128th array or object nested
/// in a document (its `remaining_depth` starts at 128 and is spent entering
/// each one). Checked here at every container's depth in the whole
/// document, never relative to a span.
const MAX_NESTING: usize = 127;

/// Object keys as path steps.
pub fn keys<'a>(path: &[&'a str]) -> Vec<Step<'a>> {
    path.iter().map(|&k| Step::Key(k)).collect()
}

/// The value at `path` (object keys from the top of `text`), decoded
/// exactly; `None` when the path is absent. Duplicate keys: the last wins.
pub fn field(text: &[u8], path: &[&str]) -> Result<Option<Value>, String> {
    field_at(text, &keys(path))
}

/// [`field`] with array steps.
pub fn field_at(text: &[u8], path: &[Step<'_>]) -> Result<Option<Value>, String> {
    let found = locate(text, path)?;
    found
        .effective
        .map(|span| decode_span(text, span))
        .transpose()
}

/// Number of elements of the array at `path`; `None` when the path is
/// absent or not an array.
pub fn array_len(text: &[u8], path: &[Step<'_>]) -> Result<Option<usize>, String> {
    let Some((start, _)) = locate(text, path)?.effective else {
        return Ok(None);
    };
    let mut c = Cursor {
        b: text,
        i: start,
        strict: false,
    };
    if c.peek() != Some(b'[') {
        return Ok(None);
    }
    c.i += 1;
    c.ws();
    if c.peek() == Some(b']') {
        return Ok(Some(0));
    }
    let mut n = 0usize;
    loop {
        c.ws();
        c.skip(0)?;
        n += 1;
        c.ws();
        match c.peek() {
            Some(b',') => c.i += 1,
            Some(b']') => return Ok(Some(n)),
            _ => return Err(format!("expected `,` or `]` at byte {}", c.i)),
        }
    }
}

/// Byte spans of the elements of the array at `path` (the effective one,
/// last key wins), in one pass; empty when absent or not an array.
pub fn array_items(text: &[u8], path: &[Step<'_>]) -> Result<Vec<(usize, usize)>, String> {
    match locate(text, path)?.effective {
        Some(span) => elements(text, span),
        None => Ok(Vec::new()),
    }
}

/// Every occurrence of `path` (duplicate keys included, in document order)
/// and the effective one a `serde_json::Value` decode keeps. Each
/// occurrence's span is checked for JSON grammar (lexically: no number is
/// converted).
#[allow(clippy::type_complexity)]
pub fn occurrences(
    text: &[u8],
    path: &[Step<'_>],
) -> Result<(Vec<(usize, usize)>, Option<(usize, usize)>), String> {
    let found = locate(text, path)?;
    Ok((found.all, found.effective))
}

/// Byte spans of the elements of the array occupying `span`; empty when
/// the value there is not an array.
pub fn elements(text: &[u8], (start, _): (usize, usize)) -> Result<Vec<(usize, usize)>, String> {
    let mut items = Vec::new();
    let mut c = Cursor {
        b: text,
        i: start,
        strict: false,
    };
    if c.peek() != Some(b'[') {
        return Ok(items);
    }
    c.i += 1;
    c.ws();
    if c.peek() == Some(b']') {
        return Ok(items);
    }
    loop {
        c.ws();
        let begin = c.i;
        c.skip(0)?;
        items.push((begin, c.i));
        c.ws();
        match c.peek() {
            Some(b',') => c.i += 1,
            Some(b']') => return Ok(items),
            _ => return Err(format!("expected `,` or `]` at byte {}", c.i)),
        }
    }
}

/// A copy of `text` with every occurrence of `path` replaced by `null`, each
/// occurrence first checked for JSON grammar without converting a number;
/// `None` when the path does not occur. For a value the caller will not
/// keep (a shadowed duplicate), so nothing is decoded.
pub fn blank_at(text: &[u8], path: &[Step<'_>]) -> Result<Option<Vec<u8>>, String> {
    let found = locate(text, path)?;
    if found.all.is_empty() {
        return Ok(None);
    }
    Ok(Some(splice_null(text, found.all)))
}

fn splice_null(text: &[u8], mut spans: Vec<(usize, usize)>) -> Vec<u8> {
    spans.sort_unstable();
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0usize;
    for (start, end) in spans {
        out.extend_from_slice(&text[at..start]);
        out.extend_from_slice(b"null");
        at = end;
    }
    out.extend_from_slice(&text[at..]);
    out
}

/// The exact value at `path` and, when the path occurs at all, a copy of
/// `text` with every occurrence replaced by `null` — so a shared-parser
/// decode of the copy never reads these numbers.
pub fn take_at(text: &[u8], path: &[Step<'_>]) -> Result<(Option<Vec<u8>>, Option<Value>), String> {
    let found = locate(text, path)?;
    if found.all.is_empty() {
        return Ok((None, None));
    }
    let value = found
        .effective
        .map(|span| decode_span(text, span))
        .transpose()?;
    Ok((Some(splice_null(text, found.all)), value))
}

/// Decode `text` as `T` with the shared parser, after taking each of
/// `paths` out exactly (see [`take_at`]); returns the exact values, in
/// `paths` order, for the caller to put back.
pub fn from_slice_exact<T: DeserializeOwned>(
    text: &[u8],
    paths: &[Vec<Step<'_>>],
) -> Result<(T, Vec<Option<Value>>), String> {
    let mut scratch: Option<Vec<u8>> = None;
    let mut values = Vec::with_capacity(paths.len());
    for path in paths {
        let current = scratch.as_deref().unwrap_or(text);
        let (rewritten, value) = take_at(current, path)?;
        if let Some(r) = rewritten {
            scratch = Some(r);
        }
        values.push(value);
    }
    let decoded =
        serde_json::from_slice(scratch.as_deref().unwrap_or(text)).map_err(|e| e.to_string())?;
    Ok((decoded, values))
}

/// A stored or transported Evidence record, its derivation coverage exact.
pub fn evidence_from_slice(text: &[u8]) -> Result<crate::Evidence, String> {
    let (mut e, mut values): (crate::Evidence, _) =
        from_slice_exact(text, &[keys(&["derivation", "segment", "coverage"])])?;
    if let (Some(d), Some(coverage)) = (e.derivation.as_mut(), values.remove(0)) {
        d.segment.coverage = coverage;
    }
    Ok(e)
}

/// A stored or transported source reference, its `source` exact.
pub fn source_reference_from_slice(text: &[u8]) -> Result<crate::SourceReference, String> {
    let (mut r, mut values): (crate::SourceReference, _) =
        from_slice_exact(text, &[keys(&["source"])])?;
    if let Some(source) = values.remove(0) {
        r.source = source;
    }
    Ok(r)
}

/// Where a path occurs: every occurrence (each is blanked) and the one a
/// `serde_json::Value` decode keeps (last key wins at every level).
struct Located {
    all: Vec<(usize, usize)>,
    effective: Option<(usize, usize)>,
}

fn locate(text: &[u8], path: &[Step<'_>]) -> Result<Located, String> {
    let mut c = Cursor {
        b: text,
        i: 0,
        strict: false,
    };
    c.ws();
    c.locate(path, 0)
}

fn decode_span(text: &[u8], (start, _): (usize, usize)) -> Result<Value, String> {
    let mut c = Cursor {
        b: text,
        i: start,
        strict: false,
    };
    c.value(0)
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
    /// Validate JSON grammar while skipping: set for every span that is
    /// blanked, so a value serde_json will never see is still valid JSON
    /// (strings with their escapes, number grammar) — without converting a
    /// number, so a lexically valid shadowed `1e999` is kept.
    strict: bool,
}

impl Cursor<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", byte as char, self.i))
        }
    }

    /// Step into the array or object at the cursor, which sits `depth`
    /// containers deep in its document (the top value at 0): refused where
    /// the shared parser refuses it.
    fn enter(&mut self, depth: usize) -> Result<(), String> {
        if depth >= MAX_NESTING {
            return Err(format!("recursion limit exceeded at byte {}", self.i));
        }
        self.i += 1;
        Ok(())
    }

    /// Walk `path` from the value at the cursor, consuming that whole
    /// value, and record where the path occurs.
    fn locate(&mut self, path: &[Step<'_>], depth: usize) -> Result<Located, String> {
        let mut found = Located {
            all: Vec::new(),
            effective: None,
        };
        let Some((step, rest)) = path.split_first() else {
            // This span is blanked for the shared decode, which will never
            // read it: validate it here, shadowed duplicates included.
            let start = self.i;
            let lax = std::mem::replace(&mut self.strict, true);
            let skipped = self.skip(depth);
            self.strict = lax;
            skipped?;
            found.all.push((start, self.i));
            found.effective = Some((start, self.i));
            return Ok(found);
        };
        let (open, close) = match step {
            Step::Key(_) => (b'{', b'}'),
            Step::Index(_) => (b'[', b']'),
        };
        if self.peek() != Some(open) {
            // Not the container the step needs: the path is absent here.
            self.skip(depth)?;
            return Ok(found);
        }
        self.enter(depth)?;
        self.ws();
        if self.peek() == Some(close) {
            self.i += 1;
            return Ok(found);
        }
        let mut at = 0usize;
        loop {
            self.ws();
            let matched = match *step {
                Step::Key(key) => {
                    let k = self.string()?;
                    self.ws();
                    self.expect(b':')?;
                    self.ws();
                    k == key
                }
                Step::Index(n) => at == n,
            };
            if matched {
                let inner = self.locate(rest, depth + 1)?;
                found.all.extend(inner.all);
                // A later duplicate key replaces an earlier one, even when
                // the later one does not contain the rest of the path.
                found.effective = inner.effective;
            } else {
                self.skip(depth + 1)?;
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(byte) if byte == close => {
                    self.i += 1;
                    return Ok(found);
                }
                _ => return Err(format!("unexpected byte at {}", self.i)),
            }
            at += 1;
        }
    }

    /// The raw text of a string token, decoded by serde_json (string
    /// decoding does not depend on the float parser).
    fn string(&mut self) -> Result<String, String> {
        let start = self.i;
        self.skip_string()?;
        let token = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
        serde_json::from_str::<String>(token).map_err(|e| e.to_string())
    }

    fn skip_string(&mut self) -> Result<(), String> {
        let start = self.i;
        self.expect(b'"')?;
        while let Some(byte) = self.peek() {
            self.i += 1;
            match byte {
                b'"' => {
                    if self.strict {
                        // serde_json's own string grammar (escapes, control
                        // characters, UTF-8): unaffected by the float parser.
                        serde_json::from_slice::<String>(&self.b[start..self.i])
                            .map_err(|e| e.to_string())?;
                    }
                    return Ok(());
                }
                b'\\' => self.i += 1,
                _ => {}
            }
        }
        Err("unterminated string".into())
    }

    fn number_token(&mut self) -> Result<&str, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i == start {
            return Err(format!("expected a value at byte {start}"));
        }
        let token = &self.b[start..self.i];
        if self.strict && !is_json_number(token) {
            return Err(format!(
                "invalid number `{}` at byte {start}",
                String::from_utf8_lossy(token)
            ));
        }
        std::str::from_utf8(token).map_err(|e| e.to_string())
    }

    fn skip(&mut self, depth: usize) -> Result<(), String> {
        match self.peek() {
            Some(b'"') => self.skip_string(),
            Some(b'{') | Some(b'[') => {
                let close = if self.peek() == Some(b'{') {
                    b'}'
                } else {
                    b']'
                };
                let object = close == b'}';
                self.enter(depth)?;
                self.ws();
                if self.peek() == Some(close) {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    self.ws();
                    if object {
                        self.skip_string()?;
                        self.ws();
                        self.expect(b':')?;
                        self.ws();
                    }
                    self.skip(depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(byte) if byte == close => {
                            self.i += 1;
                            return Ok(());
                        }
                        _ => return Err(format!("unexpected byte at {}", self.i)),
                    }
                }
            }
            Some(b't') => self.literal("true"),
            Some(b'f') => self.literal("false"),
            Some(b'n') => self.literal("null"),
            _ => self.number_token().map(|_| ()),
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(())
        } else {
            Err(format!("expected `{word}` at byte {}", self.i))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        match self.peek() {
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true").map(|_| Value::Bool(true)),
            Some(b'f') => self.literal("false").map(|_| Value::Bool(false)),
            Some(b'n') => self.literal("null").map(|_| Value::Null),
            Some(b'[') => {
                self.enter(depth)?;
                let mut items = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(Value::Array(items));
                }
                loop {
                    self.ws();
                    items.push(self.value(depth + 1)?);
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::Array(items));
                        }
                        _ => return Err(format!("expected `,` or `]` at byte {}", self.i)),
                    }
                }
            }
            Some(b'{') => {
                self.enter(depth)?;
                let mut map = Map::new();
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Value::Object(map));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    self.expect(b':')?;
                    self.ws();
                    let v = self.value(depth + 1)?;
                    map.insert(k, v);
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Object(map));
                        }
                        _ => return Err(format!("expected `,` or `}}` at byte {}", self.i)),
                    }
                }
            }
            _ => {
                let token = self.number_token()?;
                number(token).map(Value::Number)
            }
        }
    }
}

/// JSON number grammar (RFC 8259):
/// `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`. Lexical only: a
/// valid token is not converted here, so an out-of-range one is still valid
/// JSON.
fn is_json_number(t: &[u8]) -> bool {
    let digits = |i: &mut usize| {
        let start = *i;
        while t.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i > start
    };
    let mut i = 0;
    if t.first() == Some(&b'-') {
        i += 1;
    }
    match t.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if t.get(i) == Some(&b'.') {
        i += 1;
        if !digits(&mut i) {
            return false;
        }
    }
    if matches!(t.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(t.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return false;
        }
    }
    i == t.len()
}

/// One JSON number token, exactly: an integer in [-2^63, 2^64-1] as an
/// integer (`-0` excepted: serde_json holds it as the double -0.0), every
/// other number as its correctly rounded double; a number no double can
/// hold is refused.
fn number(token: &str) -> Result<Number, String> {
    if !is_json_number(token.as_bytes()) {
        return Err(format!("invalid number `{token}`"));
    }
    let integer = !token.contains(['.', 'e', 'E']);
    if integer && token != "-0" {
        if let Ok(n) = token.parse::<u64>() {
            return Ok(Number::from(n));
        }
        if let Ok(n) = token.parse::<i64>() {
            return Ok(Number::from(n));
        }
    }
    let x: f64 = token
        .parse()
        .map_err(|_| format!("invalid number `{token}`"))?;
    Number::from_f64(x).ok_or_else(|| format!("number out of range: {token}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, path: &[&str]) -> Option<Value> {
        field(line.as_bytes(), path).unwrap()
    }

    #[test]
    fn doubles_are_correctly_rounded_and_integers_exact() {
        let line = r#"{"kind":"x","data":{"skip":[1,{"a":"}"}],"source":{"d":51.248178375505404,"n":9007199254740993,"m":18446744073709551615,"i":-9223372036854775808,"z":-0,"f":1.0}}}"#;
        let v = at(line, &["data", "source"]).unwrap();
        assert_eq!(
            serde_json::to_string(&v["d"]).unwrap(),
            "51.248178375505404"
        );
        assert_eq!(serde_json::to_string(&v["n"]).unwrap(), "9007199254740993");
        assert_eq!(
            serde_json::to_string(&v["m"]).unwrap(),
            "18446744073709551615"
        );
        assert_eq!(
            serde_json::to_string(&v["i"]).unwrap(),
            "-9223372036854775808"
        );
        assert_eq!(serde_json::to_string(&v["z"]).unwrap(), "-0.0");
        assert_eq!(serde_json::to_string(&v["f"]).unwrap(), "1.0");
    }

    #[test]
    fn an_absent_path_is_none_and_out_of_range_is_refused() {
        assert_eq!(at(r#"{"data":{"other":1}}"#, &["data", "source"]), None);
        // A non-object on the path is stepped over, not misread.
        assert_eq!(
            at(r#"{"data":7,"later":{"source":1}}"#, &["data", "source"]),
            None
        );
        let err = field(br#"{"data":{"source":{"x":1e999}}}"#, &["data", "source"]).unwrap_err();
        assert!(err.contains("number out of range"), "{err}");
    }

    #[test]
    fn array_steps_reach_a_feed_entry() {
        let body = br#"{"entries":[{"payload":{"source":{"d":1.0}}},{"payload":{"source":{"d":51.248178375505404}}}]}"#;
        let v = field_at(
            body,
            &[
                Step::Key("entries"),
                Step::Index(1),
                Step::Key("payload"),
                Step::Key("source"),
            ],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::to_string(&v["d"]).unwrap(),
            "51.248178375505404"
        );
        let none = field_at(body, &[Step::Key("entries"), Step::Index(5)]).unwrap();
        assert_eq!(none, None);
    }

    /// Revision 29, finding 2: duplicate keys resolve as `serde_json::Value`
    /// resolves them — the last occurrence wins, at the leaf and at every
    /// ancestor — so the exact value is the one the typed decode keeps.
    #[test]
    fn the_last_duplicate_key_wins_at_every_level() {
        let raw = br#"{"payload":{"reference":{"source":{"d":1},"source":{"d":2}}}}"#;
        let typed: Value = serde_json::from_slice(raw).unwrap();
        let path = keys(&["payload", "reference", "source"]);
        let exact = field_at(raw, &path).unwrap().unwrap();
        assert_eq!(
            exact, typed["payload"]["reference"]["source"],
            "leaf duplicate"
        );
        assert_eq!(exact["d"], 2);

        // A duplicate ancestor: the last `reference` wins, source and all.
        let raw = br#"{"payload":{"reference":{"source":{"d":1}},"reference":{"source":{"d":3}}}}"#;
        let typed: Value = serde_json::from_slice(raw).unwrap();
        let exact = field_at(raw, &path).unwrap().unwrap();
        assert_eq!(
            exact, typed["payload"]["reference"]["source"],
            "ancestor duplicate"
        );

        // The last ancestor has no source: absent, as in the typed value.
        let raw = br#"{"payload":{"reference":{"source":{"d":1}},"reference":{"other":0}}}"#;
        let typed: Value = serde_json::from_slice(raw).unwrap();
        assert!(typed["payload"]["reference"].get("source").is_none());
        assert_eq!(
            field_at(raw, &path).unwrap(),
            None,
            "the later ancestor wins"
        );
    }

    /// Revision 29, finding 1: the shared parser refuses this valid token
    /// (its significand-times-power path overflows); correctly rounded it is
    /// f64::MAX. Taking the field out first lets the typed decode succeed and
    /// the exact value return.
    #[test]
    fn a_token_the_shared_parser_refuses_decodes_exactly_when_taken_first() {
        let raw = br#"{"reference":{"source":{"max":17976931348623158e292},"sourceId":"s"}}"#;
        assert!(
            serde_json::from_slice::<Value>(raw).is_err(),
            "precondition: the shared parser refuses 17976931348623158e292"
        );
        let path = keys(&["reference", "source"]);
        let (decoded, values): (Value, _) = from_slice_exact(raw, &[path]).unwrap();
        assert_eq!(
            decoded["reference"]["source"],
            Value::Null,
            "blanked for the shared decode"
        );
        assert_eq!(
            decoded["reference"]["sourceId"], "s",
            "the rest decodes as before"
        );
        let source = values[0].clone().unwrap();
        assert_eq!(source["max"].as_f64(), Some(f64::MAX));
        assert_eq!(
            serde_json::to_string(&source["max"]).unwrap(),
            "1.7976931348623157e+308"
        );
    }

    /// Every occurrence is blanked, so an earlier duplicate the shared
    /// parser would refuse cannot fail the typed decode either.
    #[test]
    fn every_duplicate_occurrence_is_blanked() {
        let raw = br#"{"a":{"s":{"x":1e999}},"a":{"s":{"x":1}}}"#;
        let (rewritten, value) = take_at(raw, &keys(&["a", "s"])).unwrap();
        assert_eq!(
            String::from_utf8(rewritten.unwrap()).unwrap(),
            r#"{"a":{"s":null},"a":{"s":null}}"#
        );
        assert_eq!(value.unwrap()["x"], 1, "the last occurrence's value");
        assert_eq!(
            take_at(raw, &keys(&["b"])).unwrap(),
            (None, None),
            "absent: untouched"
        );
    }

    /// Revision 30, finding 2: every blanked span is valid JSON, whether it
    /// is the selected value or a shadowed duplicate — the shared decode
    /// never sees them, so it cannot be the one to refuse them.
    #[test]
    fn a_blanked_span_must_be_valid_json() {
        let source = keys(&["reference", "source"]);
        let refused = |raw: &[u8]| {
            assert!(
                serde_json::from_slice::<Value>(raw).is_err(),
                "precondition: invalid JSON"
            );
            from_slice_exact::<Value>(raw, std::slice::from_ref(&source)).expect_err("refused")
        };
        // The selected value: a leading zero is not a JSON number.
        let e = refused(br#"{"reference":{"source":{"n":01}}}"#);
        assert!(e.contains("invalid number"), "{e}");
        // A shadowed duplicate with a malformed number.
        let e = refused(br#"{"reference":{"source":{"n":1.2.3},"source":{"ok":1}}}"#);
        assert!(e.contains("invalid number"), "{e}");
        // A shadowed duplicate with an invalid escape, and a raw control
        // character.
        refused(br#"{"reference":{"source":{"s":"\q"},"source":{"ok":1}}}"#);
        refused(b"{\"reference\":{\"source\":{\"s\":\"a\x01b\"},\"source\":{\"ok\":1}}}");
    }

    /// Validation is lexical: a well-formed but out-of-range number in a
    /// shadowed duplicate is valid JSON and is not converted; valid escapes
    /// pass; the effective value is still decoded exactly.
    #[test]
    fn a_lexically_valid_shadowed_value_is_kept_out_of_conversion() {
        let raw = br#"{"reference":{"source":{"x":1e999,"s":"\u00e9\n\"\\/"},"source":{"x":17976931348623158e292}}}"#;
        let (decoded, values): (Value, _) =
            from_slice_exact(raw, &[keys(&["reference", "source"])]).unwrap();
        assert_eq!(decoded["reference"]["source"], Value::Null);
        assert_eq!(values[0].as_ref().unwrap()["x"].as_f64(), Some(f64::MAX));
    }

    /// Nesting is bounded where the shared parser bounds it, counted from
    /// the top of the whole document: 127 nested arrays and objects read,
    /// 128 do not, in the kept value and in a shadowed duplicate.
    #[test]
    fn nesting_is_bounded_where_the_shared_parser_bounds_it() {
        // `levels` containers in all, `wrap` of them above the chain.
        let chain = |levels: usize, wrap: usize| {
            let inner = levels - wrap;
            format!("{}0{}", r#"{"d":"#.repeat(inner), "}".repeat(inner))
        };
        let path = keys(&["reference", "source"]);
        for levels in [126, 127, 128, 129] {
            let deep = chain(levels, 2);
            for raw in [
                format!(r#"{{"reference":{{"source":{deep}}}}}"#),
                format!(r#"{{"reference":{{"source":{deep},"source":{{}}}}}}"#),
            ] {
                let shared = serde_json::from_slice::<Value>(raw.as_bytes());
                assert_eq!(
                    shared.is_ok(),
                    levels <= 127,
                    "precondition: serde_json reads 127 nested containers, not 128"
                );
                let exact = from_slice_exact::<Value>(raw.as_bytes(), std::slice::from_ref(&path));
                assert_eq!(
                    exact.is_ok(),
                    shared.is_ok(),
                    "{levels} nested containers: shared parser {:?}, exact {:?}",
                    shared.err(),
                    exact.err()
                );
            }
        }
    }

    #[test]
    fn json_number_grammar() {
        for ok in [
            "0",
            "-0",
            "1",
            "10",
            "1.5",
            "-1.5e10",
            "1E+2",
            "1e-7",
            "1e999",
            "17976931348623158e292",
        ] {
            assert!(is_json_number(ok.as_bytes()), "{ok}");
        }
        for bad in [
            "01", "-01", "1.", ".5", "+1", "1e", "1e+", "1.2.3", "--1", "1-", "", "-",
        ] {
            assert!(!is_json_number(bad.as_bytes()), "{bad}");
        }
    }

    #[test]
    fn array_items_span_each_element_once() {
        let raw = br#"{"entries":[ {"a":1} , "x",[2]]}"#;
        let spans = array_items(raw, &keys(&["entries"])).unwrap();
        let items: Vec<&[u8]> = spans.iter().map(|&(s, e)| &raw[s..e]).collect();
        assert_eq!(items, vec![&br#"{"a":1}"#[..], &br#""x""#[..], &b"[2]"[..]]);
    }

    #[test]
    fn array_len_counts_the_effective_array() {
        let raw = br#"{"entries":[1,{"a":[2]},"x"],"other":[]}"#;
        assert_eq!(array_len(raw, &keys(&["entries"])).unwrap(), Some(3));
        assert_eq!(array_len(raw, &keys(&["other"])).unwrap(), Some(0));
        assert_eq!(array_len(raw, &keys(&["missing"])).unwrap(), None);
    }

    #[test]
    fn strings_keep_their_escapes_and_nested_structure() {
        let line = r#"{"data":{"source":{"s":"a\"b\\c\u00e9","arr":[true,false,null,[]],"o":{}}}}"#;
        let v = at(line, &["data", "source"]).unwrap();
        assert_eq!(v["s"], "a\"b\\cé");
        assert_eq!(v["arr"], serde_json::json!([true, false, null, []]));
        assert_eq!(v["o"], serde_json::json!({}));
    }
}
