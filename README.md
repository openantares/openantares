# OpenAntares

The canonical Rust implementation of the **`.ant` interchange format**: the
record types, the reader and writer, and a CLI to validate and inspect files.

`.ant` is a self-contained, compressed, streamable container for exchanging a
selection of a knowledge graph — schema types, vertices, edges, observations,
evidence, beliefs, vector documents and deletions — as plain JSON records
inside a standard zstd frame, checksummed end to end. The format is open and
fully specified: a conformant reader is about a hundred lines in any language
with zstd and SHA-256.

This repository is the **method**, not the product. The Antares engine that
produces and consumes these files is a separate, private system; nothing here
depends on it, and you need none of it to read, write or verify a `.ant` file.

## Crates

| crate | what it is |
|-------|------------|
| [`ant-types`](crates/ant-types) | the record types — ids, graph, observation, evidence, belief, schema, typed property values |
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
antares-format = "0.1"
```

```rust
use antares_format::AntReader;

let mut reader = AntReader::new(std::io::BufReader::new(std::fs::File::open("world.ant")?))?;
while let Some(record) = reader.next_record()? {
    // records stream; the trailer's SHA-256 and counts are verified as you go
}
```

## Versions: crate `0.1.x`, format `0.3.x`

**The crate version and the format version are deliberately not the same
number.** These crates are at `0.1.x`; the format they implement is `0.3`.

Under pre-1.0 semver the *minor* is the breaking slot, so aligning the crate
version to the format version would force a lie the first time the Rust API
breaks without the format changing — or the reverse. Instead:

- the crate version tracks **this code's API**;
- the format version a build supports is declared by
  `antares_format::SUPPORTED_FORMAT_VERSION`, reported by `openantares info`,
  and enforced by the reader itself.

A reader accepts **any MINOR at the same MAJOR** and tells you when a file is
ahead of it; a **different MAJOR is refused** rather than misread. That rule is
normative — see §3 of the spec.

## Specification and conformance

The normative specification, the JSON Schema, the reference bindings for
Python and JavaScript, and the golden files every implementation is verified
against live in **[openantares/ant](https://github.com/openantares/ant)** —
see [SPEC.md](https://github.com/openantares/ant/blob/main/SPEC.md) and the
[v0.5.0 release](https://github.com/openantares/ant/releases/tag/v0.5.0).

CI here runs this crate's conformance suite against those goldens, pinned at
the format's release tag, so the canonical writer and the published format
cannot drift apart silently.

API documentation is published on docs.rs for each crate:
[ant-types](https://docs.rs/ant-types), [antares-format](https://docs.rs/antares-format),
[openantares](https://docs.rs/openantares).

## License

Apache-2.0 — see [LICENSE](LICENSE).
