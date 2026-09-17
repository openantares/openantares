//! Generates the published `.ant` JSON Schema and the mechanical parts of
//! both reference bindings FROM THE RUST TYPES, so none of them can drift
//! from the writer. Run from the workspace root:
//!
//! ```text
//! cargo run -p antares-format --features schemars --example gen_schema
//! ```
//!
//! CI runs exactly that and fails when `git diff` is not empty afterwards
//! — the same pattern as the monorepo's OpenAPI snapshot. A type changed
//! without regenerating turns the check red; regenerating turns it green.
//!
//! Derived, and from where:
//!
//! - every record arm and payload shape: `AntRecord` and the types it
//!   carries, through schemars derives that follow the serde attributes
//!   the writer uses (renames, `flatten`, defaults, `deny_unknown_fields`);
//! - the `status` conditionals of a relationship proposal: serde's
//!   internally tagged `ProposalStatus`, which renders as exclusive arms —
//!   a status that requires a `receipt` or a `reason` says so because the
//!   variant carries that field;
//! - the sampling conditionals: `SamplingMethod::field_rule`, the table
//!   `Sampling::validate` applies — one source, two consumers;
//! - the property-value grammar: the `JsonSchema` impl beside
//!   `PropertyValue`'s `Serialize`;
//! - the binding facts: `FORMAT_MAJOR`/`FORMAT_MINOR`, the data kinds (the
//!   arms minus manifest and trailer), `DATA_KIND_COUNT_KEYS`, the later
//!   count keys (`Counts` fields with a serde default), the `Counts`
//!   fields themselves;
//! - descriptions: every doc comment.
//!
//! Shaped, so the published document keeps its structure:
//!
//! - each arm becomes `$defs/<kind>`, referenced from the top-level
//!   `oneOf`; a payload type used by exactly one arm is inlined into it,
//!   nested types stay as `$defs` renamed to snake_case;
//! - scalar newtypes (ids) are inlined where they are used, the field's
//!   doc line winning over the type's;
//! - `additionalProperties: true` is stated on every open object — the
//!   spec's forward-compatibility rule, made explicit;
//! - the `unknown_kind` arm is synthesized from the known kinds;
//! - `$id`, `title` and the root `description` are the normative ones.

use ant_types::SamplingMethod;
use antares_format::{AntRecord, Counts, DATA_KIND_COUNT_KEYS, FORMAT_MAJOR, FORMAT_MINOR};
use schemars::generate::SchemaSettings;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SCHEMA_ID: &str = "https://openantares.org/schema/ant.schema.json";
const ROOT_DESCRIPTION: &str = "Schema for ONE decompressed NDJSON line of a .ant stream. The \
     container-level rules (manifest first, trailer last, sha256 over preceding lines) live in \
     SPEC.md and cannot be expressed per-line.";

/// What the bindings need to know, in the order the Rust types declare it.
struct Facts {
    data_kinds: Vec<String>,
    /// (kind, trailer key), in kind order.
    count_keys: Vec<(String, String)>,
    /// Trailer keys added after the first version: absent in an older
    /// trailer, where they mean zero.
    later_keys: Vec<String>,
    /// (snake_case attribute, camelCase trailer key), in tally order.
    counts_fields: Vec<(String, String)>,
}

fn main() {
    let root = workspace_root();
    let (schema, facts) = generate();
    let schema_path = root.join("openantares/schema/ant.schema.json");
    std::fs::write(&schema_path, emit(&schema)).expect("write schema");
    println!("wrote {}", schema_path.display());
    for (path, comment, regions) in [
        (
            root.join("openantares/bindings/python/openantares.py"),
            "#",
            python_regions(&facts),
        ),
        (
            root.join("openantares/bindings/js/openantares.mjs"),
            "//",
            js_regions(&facts),
        ),
    ] {
        rewrite_regions(&path, comment, &regions);
        println!("wrote {}", path.display());
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

// ---------------------------------------------------------------------
// Derive + shape
// ---------------------------------------------------------------------

fn generate() -> (Value, Facts) {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let raw = serde_json::to_value(generator.into_root_schema_for::<AntRecord>())
        .expect("schema is json");
    let counts_raw =
        serde_json::to_value(schemars::schema_for!(Counts)).expect("counts schema is json");

    let mut defs: Map<String, Value> = raw["$defs"].as_object().cloned().unwrap_or_default();
    let mut arms: Vec<Value> = raw["oneOf"].as_array().cloned().expect("record arms");

    // 1. Scalar newtypes and enums are inlined where they are used: their
    //    value is a scalar, and the field's doc line is the context.
    //    Structs stay named definitions.
    let scalar: Vec<String> = defs
        .iter()
        .filter(|(_, v)| is_scalar_schema(v) || is_enum_schema(v))
        .map(|(k, _)| k.clone())
        .collect();
    for name in &scalar {
        let def = defs.remove(name).expect("scalar def");
        for arm in arms.iter_mut() {
            inline_ref(arm, name, &def, true);
        }
        for (_, other) in defs.iter_mut() {
            inline_ref(other, name, &def, true);
        }
    }

    // 2. The sampling conditionals, from the validator's own table.
    if let Some(sampling) = defs.get_mut("Sampling") {
        sampling["allOf"] = Value::Array(sampling_rules());
    }

    // 3. Each arm becomes `$defs/<kind>`; a payload used once is inlined.
    let kinds: Vec<String> = arms.iter().map(|a| kind_of(a).to_string()).collect();
    // How often each shared type is referenced across arms AND defs: a
    // payload used by exactly one arm is inlined into it, a payload two
    // arms share (the tombstone) stays a named definition.
    let uses: BTreeMap<String, usize> = defs
        .keys()
        .map(|name| {
            let in_defs = ref_count(&defs, name);
            let in_arms: usize = arms.iter().map(|a| ref_count_in(a, name)).sum();
            (name.clone(), in_defs + in_arms)
        })
        .collect();
    let mut arm_defs: Vec<(String, Value)> = Vec::new();
    for mut arm in arms {
        let kind = kind_of(&arm).to_string();
        if let Some(target) = arm.get("$ref").and_then(Value::as_str).map(str::to_string) {
            // A newtype variant (the manifest): the record IS the payload,
            // plus `kind`. Merge the payload's shape into the arm.
            let name = def_name(&target);
            let payload = defs.remove(&name).expect("newtype variant payload");
            arm.as_object_mut().unwrap().remove("$ref");
            merge_object_schema(&mut arm, &payload);
        }
        if let Some(data) = arm.get_mut("properties").and_then(|p| p.get_mut("data")) {
            if let Some(target) = data.get("$ref").and_then(Value::as_str).map(str::to_string) {
                let name = def_name(&target);
                if uses[&name] == 1 {
                    let payload = defs.remove(&name).expect("single-use payload");
                    *data = payload;
                }
            }
        }
        arm_defs.push((kind, arm));
    }

    // 4. Remaining shared types are named in snake_case.
    let renames: BTreeMap<String, String> = defs
        .keys()
        .map(|key| {
            let mut name = snake_case(key);
            if kinds.contains(&name) {
                name.push_str("_payload");
            }
            (key.clone(), name)
        })
        .collect();
    let mut renamed: Map<String, Value> = Map::new();
    for (old, mut value) in defs {
        let new = renames[&old].clone();
        rename_refs(&mut value, &renames);
        renamed.insert(new, value);
    }
    let mut defs = Map::new();
    for (kind, mut arm) in arm_defs {
        rename_refs(&mut arm, &renames);
        assert!(
            !renamed.contains_key(&kind),
            "arm `{kind}` collides with a shared type of the same name"
        );
        defs.insert(kind, arm);
    }
    for (k, v) in renamed {
        defs.insert(k, v);
    }

    // 5. The forward-compatibility arm.
    defs.insert(
        "unknown_kind".to_string(),
        json!({
            "description": format!(
                "Forward compatibility: any object with a string `kind` outside the \
                 v{FORMAT_MAJOR}.{FORMAT_MINOR} vocabulary is valid at the container level and \
                 MUST be skipped by readers."
            ),
            "type": "object",
            "required": ["kind"],
            "properties": { "kind": { "type": "string", "not": { "enum": kinds } } },
            "additionalProperties": true
        }),
    );

    // 6. Open objects say so; doc comments become one-line descriptions.
    let mut defs = Value::Object(defs);
    finish(&mut defs);

    let one_of: Vec<Value> = kinds
        .iter()
        .chain(std::iter::once(&"unknown_kind".to_string()))
        .map(|k| json!({ "$ref": format!("#/$defs/{k}") }))
        .collect();
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": SCHEMA_ID,
        "title": format!("OpenAntares .ant record (format {FORMAT_MAJOR}.{FORMAT_MINOR})"),
        "description": ROOT_DESCRIPTION,
        "type": "object",
        "required": ["kind"],
        "oneOf": one_of,
        "$defs": defs
    });

    // The binding facts, from the same derivation.
    let data_kinds: Vec<String> = kinds
        .iter()
        .filter(|k| k.as_str() != "manifest" && k.as_str() != "trailer")
        .cloned()
        .collect();
    let count_keys: Vec<(String, String)> = data_kinds
        .iter()
        .map(|k| {
            let key = DATA_KIND_COUNT_KEYS
                .iter()
                .find(|(kind, _)| kind == k)
                .map(|(_, key)| key.to_string())
                .unwrap_or_else(|| panic!("DATA_KIND_COUNT_KEYS has no entry for `{k}`"));
            (k.clone(), key)
        })
        .collect();
    let later_keys: Vec<String> = count_keys
        .iter()
        .map(|(_, key)| key.clone())
        .filter(|key| counts_raw["properties"][key].get("default").is_some())
        .collect();
    let counts_fields: Vec<(String, String)> = count_keys
        .iter()
        .map(|(_, key)| (snake_case(key), key.clone()))
        .collect();
    (
        schema,
        Facts {
            data_kinds,
            count_keys,
            later_keys,
            counts_fields,
        },
    )
}

/// `if method == X then require/forbid …`, one rule per distinct shape,
/// methods sharing a shape grouped under `enum` — read from the table
/// `Sampling::validate` applies.
fn sampling_rules() -> Vec<Value> {
    let mut rules: Vec<(Vec<&'static str>, Vec<&'static str>, Vec<&'static str>)> = Vec::new();
    for method in SamplingMethod::ALL {
        let rule = method.field_rule();
        match rules
            .iter_mut()
            .find(|(_, req, forb)| req == rule.required && forb == rule.forbidden)
        {
            Some((methods, _, _)) => methods.push(method.wire_name()),
            None => rules.push((
                vec![method.wire_name()],
                rule.required.to_vec(),
                rule.forbidden.to_vec(),
            )),
        }
    }
    rules
        .into_iter()
        .map(|(methods, required, forbidden)| {
            let selector = if methods.len() == 1 {
                json!({ "const": methods[0] })
            } else {
                json!({ "enum": methods })
            };
            let mut then = Map::new();
            if !required.is_empty() {
                then.insert("required".into(), json!(required));
            }
            if !forbidden.is_empty() {
                let absent: Map<String, Value> = forbidden
                    .iter()
                    .map(|f| (f.to_string(), Value::Bool(false)))
                    .collect();
                then.insert("properties".into(), Value::Object(absent));
            }
            json!({
                "if": { "properties": { "method": selector }, "required": ["method"] },
                "then": Value::Object(then)
            })
        })
        .collect()
}

// ---------------------------------------------------------------------
// Value surgery
// ---------------------------------------------------------------------

fn kind_of(arm: &Value) -> &str {
    arm["properties"]["kind"]["const"]
        .as_str()
        .expect("every arm is tagged by `kind`")
}

fn def_name(reference: &str) -> String {
    reference
        .strip_prefix("#/$defs/")
        .unwrap_or_else(|| panic!("unexpected reference {reference}"))
        .to_string()
}

/// A closed set of string constants (a Rust enum without payloads).
fn is_enum_schema(v: &Value) -> bool {
    let Some(arms) = v.get("oneOf").and_then(Value::as_array) else {
        return false;
    };
    let obj = v.as_object().unwrap();
    !obj.contains_key("properties")
        && arms.iter().all(|a| {
            a.get("const").is_some()
                && a.get("type").and_then(Value::as_str) == Some("string")
                && a.get("properties").is_none()
        })
}

fn is_scalar_schema(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    let structural = [
        "properties",
        "oneOf",
        "anyOf",
        "allOf",
        "items",
        "additionalProperties",
        "$ref",
        "patternProperties",
        "not",
    ];
    obj.contains_key("type") && structural.iter().all(|k| !obj.contains_key(*k))
}

/// Replace every `{"$ref": "#/$defs/<name>", …siblings}` under `v` with
/// the definition. `site_wins` says whose description survives when both
/// have one.
fn inline_ref(v: &mut Value, name: &str, def: &Value, site_wins: bool) {
    let target = format!("#/$defs/{name}");
    match v {
        Value::Object(obj) => {
            if obj.get("$ref").and_then(Value::as_str) == Some(target.as_str()) {
                let siblings = obj.clone();
                let mut merged = def.as_object().cloned().unwrap_or_default();
                for (k, sv) in siblings {
                    if k == "$ref" {
                        continue;
                    }
                    if k == "description" && !site_wins && merged.contains_key("description") {
                        continue;
                    }
                    merged.insert(k, sv);
                }
                *obj = merged;
            } else {
                for (_, child) in obj.iter_mut() {
                    inline_ref(child, name, def, site_wins);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_ref(item, name, def, site_wins);
            }
        }
        _ => {}
    }
}

fn ref_count(defs: &Map<String, Value>, name: &str) -> usize {
    defs.values().map(|v| ref_count_in(v, name)).sum()
}

fn ref_count_in(v: &Value, name: &str) -> usize {
    let target = format!("#/$defs/{name}");
    fn count(v: &Value, target: &str) -> usize {
        match v {
            Value::Object(obj) => {
                let here = usize::from(obj.get("$ref").and_then(Value::as_str) == Some(target));
                here + obj.values().map(|c| count(c, target)).sum::<usize>()
            }
            Value::Array(items) => items.iter().map(|c| count(c, target)).sum(),
            _ => 0,
        }
    }
    count(v, &target)
}

/// Merge a payload object schema into an arm that carries `kind` beside
/// the payload's own fields (a newtype variant). The arm's `kind` comes
/// first in `required`; the payload's description wins, the arm's doc is
/// the variant's one-liner.
fn merge_object_schema(arm: &mut Value, payload: &Value) {
    let a = arm.as_object_mut().unwrap();
    let p = payload.as_object().unwrap();
    if let (Some(Value::Object(ap)), Some(Value::Object(pp))) =
        (a.get_mut("properties"), p.get("properties"))
    {
        for (k, v) in pp {
            ap.insert(k.clone(), v.clone());
        }
    }
    let mut required: Vec<Value> = a
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for r in p
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !required.contains(r) {
            required.push(r.clone());
        }
    }
    a.insert("required".into(), Value::Array(required));
    for key in ["description", "additionalProperties"] {
        if let Some(v) = p.get(key) {
            a.insert(key.into(), v.clone());
        }
    }
}

fn rename_refs(v: &mut Value, renames: &BTreeMap<String, String>) {
    match v {
        Value::Object(obj) => {
            if let Some(Value::String(r)) = obj.get_mut("$ref") {
                if let Some(name) = r.strip_prefix("#/$defs/") {
                    if let Some(new) = renames.get(name) {
                        *r = format!("#/$defs/{new}");
                    }
                }
            }
            for (_, child) in obj.iter_mut() {
                rename_refs(child, renames);
            }
        }
        Value::Array(items) => {
            for item in items {
                rename_refs(item, renames);
            }
        }
        _ => {}
    }
}

/// Every open object states `additionalProperties: true`; an optional
/// scalar is a nullable type rather than an `anyOf` pair; every
/// description is one line. A `true`/`false` schema is left alone.
fn finish(v: &mut Value) {
    match v {
        Value::Object(obj) => {
            collapse_nullable(obj);
            let is_object = match obj.get("type") {
                Some(Value::String(t)) => t == "object",
                Some(Value::Array(ts)) => ts.iter().any(|t| t == "object"),
                _ => false,
            };
            if is_object
                && !obj.contains_key("additionalProperties")
                && !obj.contains_key("patternProperties")
            {
                obj.insert("additionalProperties".into(), Value::Bool(true));
            }
            if let Some(Value::String(d)) = obj.get_mut("description") {
                *d = d.split_whitespace().collect::<Vec<_>>().join(" ");
            }
            for (_, child) in obj.iter_mut() {
                finish(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                finish(item);
            }
        }
        _ => {}
    }
}

/// `anyOf: [<scalar>, {type: null}]` — what an `Option<Newtype>` derives
/// to — becomes the scalar with `"null"` added to its type, the form
/// schemars already uses for an `Option<primitive>`.
fn collapse_nullable(obj: &mut Map<String, Value>) {
    let Some(Value::Array(arms)) = obj.get("anyOf") else {
        return;
    };
    if arms.len() != 2 {
        return;
    }
    let is_null = |a: &Value| a.get("type").and_then(Value::as_str) == Some("null");
    let (scalar, null) = match (is_null(&arms[0]), is_null(&arms[1])) {
        (false, true) => (arms[0].clone(), &arms[1]),
        (true, false) => (arms[1].clone(), &arms[0]),
        _ => return,
    };
    let _ = null;
    if !is_scalar_schema(&scalar) && !is_enum_schema(&scalar) {
        return;
    }
    let Some(mut merged) = scalar.as_object().cloned() else {
        return;
    };
    match merged.get("type").cloned() {
        Some(Value::String(t)) => {
            merged.insert("type".into(), json!([t, "null"]));
        }
        Some(Value::Array(mut ts)) => {
            ts.push(json!("null"));
            merged.insert("type".into(), Value::Array(ts));
        }
        _ => {
            // An enum without a bare type: the null joins its constants.
            if let Some(Value::Array(one_of)) = merged.get_mut("oneOf") {
                one_of.push(json!({ "type": "null" }));
            } else {
                return;
            }
        }
    }
    obj.remove("anyOf");
    for (k, v) in merged {
        obj.entry(k).or_insert(v);
    }
}

fn snake_case(name: &str) -> String {
    // A name that does not start with a capital was chosen on purpose
    // (`propertyValue`, `propertyEnvelope`) and is kept.
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------
// Emit: deterministic key order a reader can follow
// ---------------------------------------------------------------------

const KEY_ORDER: &[&str] = &[
    "$schema",
    "$id",
    "title",
    "description",
    "type",
    "format",
    "const",
    "enum",
    "pattern",
    "minimum",
    "maximum",
    "default",
    "contentEncoding",
    "required",
    "properties",
    "additionalProperties",
    "items",
    "oneOf",
    "anyOf",
    "allOf",
    "if",
    "then",
    "not",
    "$ref",
    "$defs",
];

fn emit(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, 0, &mut out);
    out.push('\n');
    out
}

fn write_value(v: &Value, indent: usize, out: &mut String) {
    let pad = |n: usize| "  ".repeat(n);
    match v {
        Value::Object(obj) if obj.is_empty() => out.push_str("{}"),
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort_by_key(|k| {
                (
                    KEY_ORDER
                        .iter()
                        .position(|o| o == k)
                        .unwrap_or(KEY_ORDER.len()),
                    k.as_str(),
                )
            });
            // `properties` and `$defs` keep declaration-ish order by name,
            // which is what serde_json gives us (sorted) — deterministic.
            out.push_str("{\n");
            for (i, k) in keys.iter().enumerate() {
                out.push_str(&pad(indent + 1));
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write_value(&obj[*k], indent + 1, out);
                if i + 1 < keys.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push('}');
        }
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad(indent + 1));
                write_value(item, indent + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(indent));
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

// ---------------------------------------------------------------------
// Bindings: the regions between markers
// ---------------------------------------------------------------------

fn python_regions(f: &Facts) -> Vec<(&'static str, String)> {
    let mut facts = String::new();
    facts.push_str(&format!(
        "FORMAT_MAJOR = {FORMAT_MAJOR}\nFORMAT_MINOR = {FORMAT_MINOR}\n"
    ));
    facts.push_str("FORMAT_VERSION = f\"{FORMAT_MAJOR}.{FORMAT_MINOR}\"\n\n");
    facts.push_str("DATA_KINDS = (\n");
    for k in &f.data_kinds {
        facts.push_str(&format!("    \"{k}\",\n"));
    }
    facts.push_str(
        ")\n\n# trailer count key per kind (counts are camelCase per spec §5)\n_COUNT_KEY = {\n",
    );
    for (k, key) in &f.count_keys {
        facts.push_str(&format!("    \"{k}\": \"{key}\",\n"));
    }
    facts.push_str(
        "}\n\n# Trailer keys added after the first version. Absent in an older\n\
         # trailer, where they mean zero.\n_LATER_COUNT_KEYS = (\n",
    );
    for key in &f.later_keys {
        facts.push_str(&format!("    \"{key}\",\n"));
    }
    facts.push_str(")\n");

    let mut counts = String::from("@dataclass\nclass Counts:\n");
    for (attr, _) in &f.counts_fields {
        counts.push_str(&format!("    {attr}: int = 0\n"));
    }
    counts.push_str("\n    def as_trailer_dict(self) -> dict:\n        return {\n");
    for (attr, key) in &f.counts_fields {
        counts.push_str(&format!("            \"{key}\": self.{attr},\n"));
    }
    counts.push_str("        }\n\n    def bump(self, kind: str) -> None:\n        attr = {\n");
    for (kind, key) in &f.count_keys {
        let attr = &f.counts_fields.iter().find(|(_, k)| k == key).unwrap().0;
        counts.push_str(&format!("            \"{kind}\": \"{attr}\",\n"));
    }
    counts.push_str("        }[kind]\n        setattr(self, attr, getattr(self, attr) + 1)\n");
    vec![("format facts", facts), ("counts", counts)]
}

fn js_regions(f: &Facts) -> Vec<(&'static str, String)> {
    let facts = format!(
        "export const FORMAT_MAJOR = {FORMAT_MAJOR};\nexport const FORMAT_MINOR = {FORMAT_MINOR};\n\
         export const FORMAT_VERSION = `${{FORMAT_MAJOR}}.${{FORMAT_MINOR}}`;\n"
    );
    let mut kinds = String::from("const DATA_KINDS = new Set([\n");
    for k in &f.data_kinds {
        kinds.push_str(&format!("  \"{k}\",\n"));
    }
    kinds.push_str("]);\n\nconst COUNT_KEYS = {\n");
    for (k, key) in &f.count_keys {
        kinds.push_str(&format!("  {k}: \"{key}\",\n"));
    }
    kinds.push_str(
        "};\n\n// Trailer keys added after the first version. Absent in an older\n\
         // trailer, where they mean zero. Keys this binding does not know are\n\
         // ignored (spec §7/§8): they count kinds it skipped.\nconst LATER_COUNT_KEYS = [\n",
    );
    for key in &f.later_keys {
        kinds.push_str(&format!("  \"{key}\",\n"));
    }
    kinds.push_str("];\n");
    let mut counts = String::from("function emptyCounts() {\n  return {\n");
    for (_, key) in &f.counts_fields {
        counts.push_str(&format!("    {key}: 0,\n"));
    }
    counts.push_str("  };\n}\n");
    vec![
        ("format facts", facts),
        ("kinds", kinds),
        ("counts", counts),
    ]
}

fn rewrite_regions(path: &Path, comment: &str, regions: &[(&str, String)]) {
    let text = std::fs::read_to_string(path).expect("read binding");
    let mut out = text.clone();
    for (name, body) in regions {
        let begin =
            format!("{comment} ---- generated from the Rust types by gen_schema: {name} ----\n");
        let end = format!("{comment} ---- end generated: {name} ----");
        let start = out
            .find(&begin)
            .unwrap_or_else(|| panic!("{}: no `{name}` region begin marker", path.display()))
            + begin.len();
        let stop = out[start..]
            .find(&end)
            .unwrap_or_else(|| panic!("{}: no `{name}` region end marker", path.display()))
            + start;
        out.replace_range(start..stop, body);
    }
    if out != text {
        std::fs::write(path, out).expect("write binding");
    }
}
