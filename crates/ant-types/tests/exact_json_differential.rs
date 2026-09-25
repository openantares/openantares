//! Randomized differential test: `exact_json` against the shared parser.
//!
//! Each review round of this decoder found one more document the pre-parse
//! read differently from serde_json: a first duplicate key taken, a shadowed
//! malformed number or escape never checked, a shadowed array never blanked,
//! nesting counted per span. This test states the contract once, for every
//! document the generator builds, rather than one case at a time.
//!
//! An independent model of each document (neither parser) says what must
//! happen, with the opaque path `a.s`:
//! - serde_json accepts exactly the documents without a fault. The model is
//!   checked against serde_json on every document, so a wrong model fails
//!   the test instead of passing it.
//! - `from_slice_exact` accepts those, plus the documents whose only faults
//!   are valid numbers serde_json refuses inside an occurrence of `a.s`:
//!   `17976931348623158e292` (f64::MAX, correctly rounded) in any
//!   occurrence, `1e999` in an occurrence it only blanks. In the value it
//!   keeps, no double holds `1e999`, so it is refused there.
//! - Where it accepts, its value is the model's, the kept `a.s` exact
//!   (`51.248178375505404`, which serde_json reads as a neighbour).
//!
//! Faults generated: malformed numbers, bad escapes, raw control
//! characters, lone surrogates, invalid UTF-8, broken literals, missing or
//! trailing commas, missing colons, and nesting past serde_json's 127
//! levels. Keys repeat at every level, some spelled with escapes, so `a.s`
//! occurs several times with one effective (last-key-wins) occurrence.

use ant_types::exact_json::{from_slice_exact, keys};
use serde_json::{Map, Value};

const CASES: u64 = 20_000;
/// serde_json refuses the 128th nested array or object.
const MAX_NESTING: usize = 127;
const PATH: [&str; 2] = ["a", "s"];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// What a scalar token is, to both parsers.
#[derive(Clone, Debug)]
enum Class {
    /// Read to this value by both parsers.
    Exact(Value),
    /// Valid and finite; serde_json reads a neighbouring double.
    Lossy(f64),
    /// Valid; correctly rounded it is f64::MAX; serde_json refuses it.
    MaxDouble,
    /// Valid JSON that no double holds.
    Huge,
    /// Not valid JSON: every reader refuses it.
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fault {
    MissingComma,
    TrailingComma,
    MissingColon,
}

#[derive(Debug)]
enum Node {
    Object(Vec<(Vec<u8>, Option<String>, Node)>, Option<Fault>),
    Array(Vec<Node>, Option<Fault>),
    Scalar(Vec<u8>, Class),
}

/// A backslash, spelled so no `\u` sequence appears in this source.
const BS: char = '\\';

fn quoted(inner: &str) -> Vec<u8> {
    format!("\"{inner}\"").into_bytes()
}

fn key_pool() -> Vec<(Vec<u8>, Option<String>)> {
    vec![
        (quoted("a"), Some("a".into())),
        (quoted("s"), Some("s".into())),
        (quoted("x"), Some("x".into())),
        (quoted(&format!("{BS}u0061")), Some("a".into())),
        (quoted(&format!("{BS}u0073")), Some("s".into())),
        (quoted(&format!("s{BS}u0000")), Some("s\0".into())),
        (quoted(&format!("{BS}q")), None),
        (vec![b'"', b's', 1, b'"'], None),
        (quoted(&format!("{BS}ud800")), None),
    ]
}

fn string_pool() -> Vec<(Vec<u8>, Class)> {
    let ok = |s: &str| Class::Exact(Value::String(s.into()));
    vec![
        (quoted("hi"), ok("hi")),
        (quoted(""), ok("")),
        (
            quoted(&format!("{BS}n{BS}\"{BS}{BS}{BS}/{BS}b{BS}f{BS}r{BS}t")),
            ok("\n\"\\/\u{8}\u{c}\r\t"),
        ),
        (quoted(&format!("{BS}u00e9")), ok("\u{e9}")),
        ("\"\u{e9}\"".as_bytes().to_vec(), ok("\u{e9}")),
        (quoted(&format!("{BS}ud83d{BS}ude00")), ok("\u{1f600}")),
        (quoted("}]"), ok("}]")),
        (quoted(&format!("{BS}\"}}")), ok("\"}")),
        (quoted(&format!("{BS}q")), Class::Invalid),
        (vec![b'"', b'a', 1, b'b', b'"'], Class::Invalid),
        (quoted(&format!("{BS}ud800")), Class::Invalid),
        (vec![b'"', 0xff, b'"'], Class::Invalid),
        (quoted(&format!("{BS}u12")), Class::Invalid),
    ]
}

fn number_pool() -> Vec<(Vec<u8>, Class)> {
    let n = |t: &str, v: Value| (t.as_bytes().to_vec(), Class::Exact(v));
    let bad = |t: &str| (t.as_bytes().to_vec(), Class::Invalid);
    vec![
        n("0", Value::from(0u64)),
        n("-1", Value::from(-1i64)),
        n("42", Value::from(42u64)),
        n("18446744073709551615", Value::from(u64::MAX)),
        n("-9223372036854775808", Value::from(i64::MIN)),
        n("9007199254740993", Value::from(9_007_199_254_740_993u64)),
        n("1.5", Value::from(1.5f64)),
        n("-2.25", Value::from(-2.25f64)),
        n("1e2", Value::from(100.0f64)),
        n("1E-3", Value::from(0.001f64)),
        n("-0", Value::from(-0.0f64)),
        n("0.1", Value::from(0.1f64)),
        (b"17976931348623158e292".to_vec(), Class::MaxDouble),
        (b"1e999".to_vec(), Class::Huge),
        (b"-1e999".to_vec(), Class::Huge),
        bad("01"),
        bad("1."),
        bad(".5"),
        bad("+1"),
        bad("1e"),
        bad("1.2.3"),
        bad("-"),
        bad("--1"),
        bad("1e+"),
        bad("00"),
    ]
}

fn literal_pool() -> Vec<(Vec<u8>, Class)> {
    vec![
        (b"true".to_vec(), Class::Exact(Value::Bool(true))),
        (b"false".to_vec(), Class::Exact(Value::Bool(false))),
        (b"null".to_vec(), Class::Exact(Value::Null)),
        (b"tru".to_vec(), Class::Invalid),
        (b"True".to_vec(), Class::Invalid),
    ]
}

struct Gen {
    rng: Rng,
    /// Half the documents carry faults; the rest only the numeric
    /// exceptions, so acceptance is exercised as often as refusal.
    faults: bool,
    keys: Vec<(Vec<u8>, Option<String>)>,
    scalars: Vec<(Vec<u8>, Class)>,
}

impl Gen {
    fn new(seed: u64) -> Self {
        let mut scalars = number_pool();
        scalars.extend(string_pool());
        scalars.extend(literal_pool());
        let mut rng = Rng(seed);
        let faults = rng.chance(50);
        Gen {
            rng,
            faults,
            keys: key_pool(),
            scalars,
        }
    }

    fn fault(&mut self) -> Option<Fault> {
        if !self.faults || !self.rng.chance(3) {
            return None;
        }
        Some(
            [
                Fault::MissingComma,
                Fault::TrailingComma,
                Fault::MissingColon,
            ][self.rng.below(3)],
        )
    }

    /// A scalar: invalid ones rarely, and `51.248178375505404` only inside
    /// an occurrence of the opaque path (elsewhere both parsers read it
    /// alike, lossily, and the model does not model the lossy reading).
    fn scalar(&mut self, in_path_value: bool) -> Node {
        if in_path_value && self.rng.chance(15) {
            return Node::Scalar(
                b"51.248178375505404".to_vec(),
                Class::Lossy(51.248_178_375_505_404),
            );
        }
        loop {
            let (text, class) = self.scalars[self.rng.below(self.scalars.len())].clone();
            let percent = match class {
                Class::Invalid if !self.faults => continue,
                Class::Invalid if in_path_value => 20,
                Class::Invalid => 6,
                Class::Huge | Class::MaxDouble if in_path_value => 30,
                Class::Huge | Class::MaxDouble => 5,
                Class::Exact(_) | Class::Lossy(_) => 100,
            };
            if self.rng.chance(percent) {
                return Node::Scalar(text, class);
            }
        }
    }

    /// A value `depth` containers deep. `rest` is the part of the opaque
    /// path still to match from here (None once off it); `in_path_value` is
    /// set inside an occurrence of the whole path.
    fn node(&mut self, depth: usize, rest: Option<&[&str]>, in_path_value: bool) -> Node {
        if self.rng.chance(1) {
            // A long chain straddling serde_json's 127-level bound.
            let levels = 110 + self.rng.below(24);
            let mut node = self.scalar(in_path_value);
            for _ in 0..levels.saturating_sub(depth) {
                node = Node::Array(vec![node], None);
            }
            return node;
        }
        let container = depth < 5 && (rest.is_some() || self.rng.chance(45));
        if !container {
            return self.scalar(in_path_value);
        }
        if rest.is_none() && self.rng.chance(30) {
            let items = (0..self.rng.below(4))
                .map(|_| self.node(depth + 1, None, in_path_value))
                .collect();
            return Node::Array(items, self.fault());
        }
        let mut members = Vec::new();
        for _ in 0..self.rng.below(5) {
            let (text, name) = match rest {
                // On the path: its next key often, sometimes spelled with
                // an escape, so it repeats (and shadows) at this level.
                Some([want, ..]) if self.rng.chance(55) => {
                    let spellings: Vec<_> = self
                        .keys
                        .iter()
                        .filter(|(_, n)| n.as_deref() == Some(*want))
                        .cloned()
                        .collect();
                    spellings[self.rng.below(spellings.len())].clone()
                }
                _ => {
                    let k = &self.keys[self.rng.below(self.keys.len())];
                    if k.1.is_none() && !(self.faults && self.rng.chance(15)) {
                        (quoted("x"), Some("x".into()))
                    } else {
                        k.clone()
                    }
                }
            };
            let child = match (rest, name.as_deref()) {
                (Some([want, tail @ ..]), Some(n)) if n == *want => {
                    if tail.is_empty() {
                        self.node(depth + 1, None, true)
                    } else {
                        self.node(depth + 1, Some(tail), in_path_value)
                    }
                }
                _ => self.node(depth + 1, None, in_path_value),
            };
            members.push((text, name, child));
        }
        Node::Object(members, self.fault())
    }

    fn ws(&mut self, out: &mut Vec<u8>) {
        let pads: [&[u8]; 5] = [b"", b"", b" ", b"\n", b"\t\r "];
        out.extend_from_slice(pads[self.rng.below(5)]);
    }

    fn emit(&mut self, node: &Node, out: &mut Vec<u8>) {
        self.ws(out);
        match node {
            Node::Scalar(text, _) => out.extend_from_slice(text),
            Node::Array(items, fault) => {
                out.push(b'[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        let dropped = *fault == Some(Fault::MissingComma) && i == 1;
                        out.push(if dropped { b' ' } else { b',' });
                    }
                    self.emit(item, out);
                }
                if *fault == Some(Fault::TrailingComma) && !items.is_empty() {
                    out.push(b',');
                }
                self.ws(out);
                out.push(b']');
            }
            Node::Object(members, fault) => {
                out.push(b'{');
                for (i, (key, _, value)) in members.iter().enumerate() {
                    if i > 0 {
                        let dropped = *fault == Some(Fault::MissingComma) && i == 1;
                        out.push(if dropped { b' ' } else { b',' });
                    }
                    self.ws(out);
                    out.extend_from_slice(key);
                    self.ws(out);
                    if !(*fault == Some(Fault::MissingColon) && i == 0) {
                        out.push(b':');
                    }
                    self.emit(value, out);
                }
                if *fault == Some(Fault::TrailingComma) && !members.is_empty() {
                    out.push(b',');
                }
                self.ws(out);
                out.push(b'}');
            }
        }
        self.ws(out);
    }
}

/// Where a value sits relative to the opaque path.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Zone {
    /// Not inside an occurrence of the path: serde_json reads it.
    Plain,
    /// Inside an occurrence the decoded value does not keep: blanked.
    Blanked,
    /// Inside the occurrence it keeps: decoded exactly.
    Kept,
}

#[derive(Default, Debug)]
struct Facts {
    fault: bool,
    serde_refuses: bool,
    exact_refuses: bool,
    lossy_kept: bool,
    occurrences: usize,
    kept: bool,
}

/// Does `node`'s fault show? A fault only counts when it is visible in the
/// text: a missing comma between two members, a trailing comma after one.
fn visible(fault: Option<Fault>, len: usize, object: bool) -> bool {
    match fault {
        Some(Fault::MissingComma) => len >= 2,
        Some(Fault::TrailingComma) => len >= 1,
        Some(Fault::MissingColon) => object && len >= 1,
        None => false,
    }
}

fn scan(
    node: &Node,
    depth: usize,
    zone: Zone,
    rest: Option<&[&str]>,
    effective: bool,
    f: &mut Facts,
) {
    match node {
        Node::Scalar(_, class) => match class {
            Class::Exact(_) => {}
            Class::Invalid => f.fault = true,
            Class::Lossy(_) => {
                // The generator puts it only inside an occurrence: outside
                // one both parsers round it, which the model does not model.
                assert_ne!(
                    zone,
                    Zone::Plain,
                    "generator bug: a lossy token outside the path"
                );
                f.lossy_kept |= zone == Zone::Kept;
            }
            Class::MaxDouble => {
                f.serde_refuses = true;
                f.exact_refuses |= zone == Zone::Plain;
            }
            Class::Huge => {
                f.serde_refuses = true;
                f.exact_refuses |= zone != Zone::Blanked;
            }
        },
        Node::Array(items, fault) => {
            f.fault |= depth >= MAX_NESTING || visible(*fault, items.len(), false);
            for item in items {
                scan(item, depth + 1, zone, None, false, f);
            }
        }
        Node::Object(members, fault) => {
            f.fault |= depth >= MAX_NESTING || visible(*fault, members.len(), true);
            let want = rest.and_then(|r| r.first().copied());
            let last = members
                .iter()
                .rposition(|(_, name, _)| want.is_some() && name.as_deref() == want);
            for (i, (_, name, value)) in members.iter().enumerate() {
                f.fault |= name.is_none();
                match (rest, name.as_deref()) {
                    (Some([w, tail @ ..]), Some(n)) if n == *w => {
                        let keeps = effective && Some(i) == last;
                        if tail.is_empty() {
                            f.occurrences += 1;
                            f.kept |= keeps;
                            let z = if keeps { Zone::Kept } else { Zone::Blanked };
                            scan(value, depth + 1, z, None, false, f);
                        } else {
                            scan(value, depth + 1, zone, Some(tail), keeps, f);
                        }
                    }
                    _ => scan(value, depth + 1, zone, None, false, f),
                }
            }
        }
    }
}

/// The document's value as every reader must hold it (last key wins), with
/// numbers exact. Only called on documents without a fault.
fn model(node: &Node) -> Value {
    match node {
        Node::Scalar(_, class) => match class {
            Class::Exact(v) => v.clone(),
            Class::Lossy(x) => Value::from(*x),
            Class::MaxDouble => Value::from(f64::MAX),
            // Only ever in a blanked occurrence, which a later key replaces.
            Class::Huge => Value::Null,
            Class::Invalid => unreachable!("a faulty document has no value"),
        },
        Node::Array(items, _) => Value::Array(items.iter().map(model).collect()),
        Node::Object(members, _) => {
            let mut map = Map::new();
            for (_, name, value) in members {
                map.insert(name.clone().expect("valid key"), model(value));
            }
            Value::Object(map)
        }
    }
}

#[test]
fn exact_json_accepts_and_reads_what_the_shared_parser_does_but_exact_numbers() {
    let path = keys(&PATH);
    let mut seen = [0u64; 6];
    for case in 0..CASES {
        let seed = 0x5eed_0000_0000_0000 ^ (case + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut g = Gen::new(seed);
        let root = if g.rng.chance(92) {
            g.node(0, Some(&PATH), false)
        } else {
            g.node(0, None, false)
        };
        let mut doc = Vec::new();
        g.emit(&root, &mut doc);
        let mut facts = Facts::default();
        scan(&root, 0, Zone::Plain, Some(&PATH), true, &mut facts);
        let text = String::from_utf8_lossy(&doc);

        let serde_ok = !facts.fault && !facts.serde_refuses;
        let exact_ok = !facts.fault && !facts.exact_refuses;
        let shared = serde_json::from_slice::<Value>(&doc);
        assert_eq!(
            shared.is_ok(),
            serde_ok,
            "case {case} (seed {seed:#x}): the MODEL disagrees with serde_json ({:?}) on {text}\nfacts: {facts:?}",
            shared.as_ref().err()
        );
        let exact = from_slice_exact::<Value>(&doc, std::slice::from_ref(&path));
        assert_eq!(
            exact.is_ok(),
            exact_ok,
            "case {case} (seed {seed:#x}): exact_json {} a document the model says it must {}: {text}\nexact: {:?}\nserde_json: {:?}\nfacts: {facts:?}",
            if exact.is_ok() { "accepted" } else { "refused" },
            if exact_ok { "accept" } else { "refuse" },
            exact.as_ref().err(),
            shared.as_ref().err()
        );
        if !exact_ok {
            seen[0] += 1;
            continue;
        }
        let (mut decoded, values) = exact.unwrap();
        if let Some(v) = values.into_iter().next().flatten() {
            *decoded.pointer_mut("/a/s").unwrap_or_else(|| {
                panic!("case {case}: no blanked slot for the exact value in {text}")
            }) = v;
        }
        let want = model(&root);
        assert_eq!(
            serde_json::to_string(&decoded).unwrap(),
            serde_json::to_string(&want).unwrap(),
            "case {case} (seed {seed:#x}): exact_json's value differs from the model on {text}"
        );
        if serde_ok && !facts.lossy_kept {
            assert_eq!(
                serde_json::to_string(shared.as_ref().unwrap()).unwrap(),
                serde_json::to_string(&want).unwrap(),
                "case {case} (seed {seed:#x}): the MODEL's value differs from serde_json's on {text}"
            );
        }
        seen[1] += 1;
        seen[2] += u64::from(!serde_ok);
        seen[3] += u64::from(facts.occurrences > 1);
        seen[4] += u64::from(facts.kept);
        seen[5] += u64::from(facts.lossy_kept);
    }
    // The generator must actually reach every class, or a green run proves
    // nothing: refusals, acceptances, the numeric exceptions, duplicates, a
    // kept exact value, and a kept value serde_json would round.
    let [refused, accepted, exceptions, duplicates, kept, rounded] = seen;
    let coverage = format!(
        "refused {refused}, accepted {accepted}, accepted only by exact_json {exceptions}, \
         duplicate occurrences {duplicates}, kept value {kept}, kept value serde_json rounds {rounded}"
    );
    assert!(
        seen.iter().all(|&n| n >= 100),
        "the generator under-covers a class: {coverage}"
    );
    eprintln!("{CASES} documents: {coverage}");
}
