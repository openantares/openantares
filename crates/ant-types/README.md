# ant-types

Record types of the open Antares (`.ant`) interchange format: the
vocabulary that appears inside a `.ant` file, and nothing else.

- **Graph** — `Vertex` and `Edge`, with typed properties and optional
  bitemporal validity (`valid_from`/`valid_to`, `observed_at`,
  `extracted_at`).
- **Observations** — append-only, source-bound atomic facts.
- **Evidence** — the source material observations and edges cite, with
  span offsets into the source.
- **Beliefs** — versioned inferred state derived from observations.
- **Schema** — OpenSPG-compatible type declarations.
- **Authorship** — the provenance stamp records can carry.

Property values are typed at SQL fidelity. Legacy scalars stay bare
JSON on the wire; the typed additions (decimal, date, time, timestamp,
uuid, bytes, sized ints, arrays) travel in a tagged
`{"$ant": ..., "v": ...}` envelope — an object is an envelope only
when it has exactly those two keys and `$ant` names a known type, so a
plain JSON document with a `$ant` field still round-trips as a
document.

`Decimal` is exact: an `i128` of unscaled digits plus a scale, never
`f64`. `DECIMAL`/`NUMERIC` columns survive digit for digit, scale
included (`12.3400` keeps its four fraction digits).

The container that carries these records — compression, manifest,
trailer, integrity hashing — is the
[`antares-format`](https://crates.io/crates/antares-format) crate;
this crate is just the record vocabulary.

## License

Apache-2.0.
