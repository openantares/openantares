# Changelog — antares-format

Crate and format versions are independent, but a format minor that adds
a record kind is a crate MINOR here: `AntRecord` is exhaustively matched
by consumers (this repository's own CLI needed a new arm), and `Counts`
gains a field. See `ant-types/CHANGELOG.md` for the train rule.

## 0.3.0 — 2026-09-14 — reads and writes format 0.5

- `AntRecord::RelationshipProposal { data: Box<RelationshipProposal> }`,
  counted in the trailer as `relationshipProposals`
  (`Counts::relationship_proposals`, default zero).
- `FORMAT_VERSION` / `SUPPORTED_FORMAT_VERSION` "0.5", `FORMAT_MINOR` 5.
  Same-major policy unchanged: 0.3 and 0.4 archives still read.
- Conformance golden `relationship_proposals.ant` and its generator
  (`examples/gen_conformance.rs`); every golden regenerated at 0.5. The
  published set is `openantares/ant` at `v0.5.0`.
- Requires `ant-types` 0.3. Includes the format 0.4 additions prepared
  as 0.2.0 (below), which were never published.

## 0.2.0 — format 0.4 — prepared 2026-09-10, never published

Superseded by 0.3.0 before it reached the registry; everything below
ships there.

- `AntRecord::ContradictionCase { data: Box<ContradictionCase> }`,
  counted in the trailer as `contradictionCases`
  (`Counts::contradiction_cases`, default zero).
- `FORMAT_VERSION` / `SUPPORTED_FORMAT_VERSION` "0.4",
  `FORMAT_MINOR` 4.
- The reader ignores trailer count keys it does not know (tested), so
  a later kind's count never turns an older reader red.
- Conformance golden `contradiction_cases.ant` and the generator that
  produces it (`examples/gen_conformance.rs`); every golden regenerated
  at 0.4.
- Requires `ant-types` 0.2.

## 0.1.1 — 2026-08 — format 0.3

- README, docs, license, manifest; `SUPPORTED_FORMAT_VERSION`.

## 0.1.0 — format 0.3

- First publication: zstd-framed NDJSON container, manifest, trailer
  with SHA-256 and per-kind counts, version policy (same major reads),
  unknown kinds skipped-but-hashed, tombstones (0.2), typed property
  envelopes (0.3).
