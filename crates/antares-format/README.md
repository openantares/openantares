# antares-format

Reader and writer for the Open Antares (`.ant`) container: a
self-contained, compressed, streamable file for exchanging graph data,
observations, evidence, beliefs, contradiction cases, relationship
proposals, elected ontology revisions, embeddings, and the stored
original files evidence was cut from.

This crate reads and writes `.ant` **format 0.7.x**
(`SUPPORTED_FORMAT_VERSION`) and, for a selection that carries stored
originals, **format 1.0** (`ORIGINALS_FORMAT_VERSION`); anything
without an original is still written as 0.7, byte for byte. Crate
version and format version are formally independent.

## The container

One zstd-compressed stream of NDJSON records: a `manifest` first, data
records in the middle, and a `trailer` last carrying per-kind counts
and a sha256 over every preceding uncompressed line — so truncation
and tampering are detectable in a single pass. Record payloads are the
serde JSON of the [`ant-types`](https://crates.io/crates/ant-types)
record types.

Compatibility is same-major: any minor at the same major is readable
(minor bumps are additive-only — unknown record kinds are skipped, and
`AntReader::minor_ahead` reports when a file is newer than the
reader). This build reads both majors it writes, 0 and 1; any other
major is refused explicitly, and a 0.x reader refuses a 1.0 file
rather than drop its originals.

## Use

```rust
use antares_format::{AntReader, AntRecord, AntWriter, Manifest, FORMAT_VERSION};

let manifest = Manifest {
    format: "antares".into(),
    version: FORMAT_VERSION.into(),
    tenant_id: 1,
    project_id: 1,
    selection: None,
    created_at: None,
    producer: Some("example/0.1".into()),
};

let mut writer = AntWriter::new(Vec::new(), manifest, 0)?;
writer.write(AntRecord::Evidence {
    data: ant_types::Evidence::quick(
        "ev1", ant_types::TenantId(1), ant_types::ProjectId(1),
        "note", "n1", "example content",
    ),
})?;
let bytes = writer.finish()?;

let mut reader = AntReader::new(bytes.as_slice())?;
while let Some(record) = reader.next_record()? {
    // ...
}
assert!(reader.verified); // trailer hash + counts checked
```

(The snippet uses `?`, so it lives in a function returning
`Result<_, antares_format::AntError>`; the crate docs carry the same
examples as runnable doctests.)

A conformance suite (golden files plus runners for Rust, Python, and
JavaScript) lives in the repository, alongside the normative
specification and the `openantares` CLI for validating and inspecting
files.

## License

Apache-2.0.
