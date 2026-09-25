# OpenAntares

The canonical Rust implementation of the **`.ant` interchange format**: the
record types, the reader and writer, and a CLI to validate and inspect files.

`.ant` is a self-contained, compressed, streamable container for exchanging a
selection of a knowledge graph — schema types, vertices, edges, observations,
evidence, beliefs, vector documents, deletions, contradiction cases,
relationship proposals, elected ontology revisions and the original files
evidence was cut from — as plain JSON records
inside a standard zstd frame, checksummed end to end. The format is open and
fully specified: a conformant reader is about a hundred lines in any language
with zstd and SHA-256.

This repository is the **method**, not the product. The Antares engine that
produces and consumes these files is a separate, private system; nothing here
depends on it, and you need none of it to read, write or verify a `.ant` file.

## Crates

| crate | what it is |
|-------|------------|
| [`ant-types`](crates/ant-types) | the record types — ids, graph, observation (with event time that may be explicitly unknown), evidence (with its stored original, provenance and cleaned-text derivation), belief, contradiction case, relationship proposal, ontology revision, schema, typed property values |
| [`antares-format`](crates/antares-format) | the container: streaming reader and writer, integrity checking, version policy |
| [`openantares`](crates/openantares) | the CLI — `validate` and `info` |

## Quick start

```sh
cargo install openantares

openantares validate world.ant      # exit 0 clean, 65 on a bad file
openantares info world.ant          # manifest, counts, format version
```

As a library:

```toml
[dependencies]
antares-format = "0.6"
```

```rust
use antares_format::AntReader;

let mut reader = AntReader::new(std::io::BufReader::new(std::fs::File::open("world.ant")?))?;
while let Some(record) = reader.next_record()? {
    // records stream; the trailer's SHA-256 and counts are verified as you go
}
```

## Versions: crate `0.6.x`, format `0.7.x` and `1.0`

**The crate version and the format version are deliberately not the same
number.** These crates are at `0.6.x`; they write format `0.7`, and format
`1.0` for a selection that carries stored original files.

Under pre-1.0 semver the *minor* is the breaking slot, so aligning the crate
version to the format version would force a lie the first time the Rust API
breaks without the format changing — or the reverse. Instead:

- the crate version tracks **this code's API**;
- the format version a build supports is declared by
  `antares_format::SUPPORTED_FORMAT_VERSION`, reported by `openantares info`,
  and enforced by the reader itself.

A reader accepts **any MINOR at the same MAJOR** and tells you when a file is
ahead of it; a **different MAJOR is refused** rather than misread. That rule is
normative — see §3 of the spec. Stored originals are a major (`1.0`) for exactly
that reason: a `0.x` reader refuses such a file instead of silently dropping its
originals. This build reads both majors it writes.

## Specification and conformance

The normative specification, the JSON Schema, the reference bindings for
Python and JavaScript, and the golden files every implementation is verified
against live in **[openantares/ant](https://github.com/openantares/ant)** —
see [SPEC.md](https://github.com/openantares/ant/blob/main/SPEC.md) and the
[v1.0.0 release](https://github.com/openantares/ant/releases/tag/v1.0.0).

CI here runs this crate's conformance suite against those goldens, pinned at
the format's release tag, so the canonical writer and the published format
cannot drift apart silently.

API documentation is published on docs.rs for the two library crates,
[ant-types](https://docs.rs/ant-types) and [antares-format](https://docs.rs/antares-format);
the CLI is on [crates.io](https://crates.io/crates/openantares).

## License

Apache-2.0 — see [LICENSE](LICENSE).
