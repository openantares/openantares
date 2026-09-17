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
- **Contradiction cases** — immutable revisions comparing two or more
  exact claim revisions, carrying references (never copies) and three
  independent states: epistemic, business impact, workflow (format 0.4).
- **Relationship proposals** — immutable measured proposals and their
  evidence/review closure (format 0.5).
- **Ontology revisions** — immutable elected semantic manifests with
  exact vault/head pins, typed definitions, publication closure,
  approval binding, publisher identity, and conditional position
  (format 0.7).
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
