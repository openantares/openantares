# Changelog — ant-types

Crate versions follow Cargo's semver rules for 0.x: the second number
moves for anything a downstream build can break on (a new variant in an
exhaustively matched enum, a new field on a plainly constructed struct),
the third for docs, metadata and additive-only changes. A `.ant` FORMAT
minor that adds a record kind is therefore a crate MINOR across the
publishable train (`ant-types`, `antares-format`, `openantares`), which
moves together so the registry stays coherent in dependency order.

## 0.5.1 — 2026-09-21 — format 0.7 — docs and metadata

- Crate-level docs list the `event_time` module (`EventTime`,
  `TimeBasis`, `UnknownTime`, format 0.6) and the `ontology` module
  (`OntologyRevision`, format 0.7), and say an observation's two times
  are an `EventTime`. The README covers the 0.6 time states.
- `SamplingFieldRule`'s docs linked a function that never existed; they
  now link `RelationSupport::check_coherent`, so a strict rustdoc build
  passes.
- `documentation` metadata points at docs.rs. No API, wire or format
  change.

## 0.5.0 — 2026-09-17 — format 0.7 — elected ontology revisions

- New `ontology` module with the immutable `OntologyRevision` envelope,
  typed reviewed manifest, semantic items, exact record/revision refs,
  retained positions, contributor attribution, approval binding,
  publisher stamp, and conditional ontology-head position.
- The stable semantic identity is `orv1:<manifestSha256>`; canonical
  hashes use `antares-canonical-json-v1`. Structural validation enforces
  explicit closure and the fixed `ontology/v1` / `ontology` chain.
- This is a crate-minor addition because `antares-format::AntRecord`
  gains an exhaustively matched variant; publication remains a separate
  release-prep decision.

## 0.4.1 — 2026-09-17 — format 0.6 — conditional revision chains

- New public `ConditionalRevision` (`chainId`, `expectedPreviousRevisionId`,
  `initializeFromExisting`) with `validate`, plus
  `CONDITIONAL_REVISION_METADATA_KEY`: the optimistic-concurrency
  condition an engine keeps inside an observation's `metadata` as a
  versioned envelope, so it travels through sync and `.ant` archives
  unchanged. `JsonSchema` shadow under `schemars`.
- `Observation::conditional_revision`, `set_conditional_revision`,
  `take_conditional_revision` and
  `metadata_uses_conditional_revision_envelope`: read, attach and strip
  that envelope, restoring the producer's original metadata exactly.
  Additive: no record arm, count or wire shape changed, and an archive
  without conditions is byte-identical.
## 0.4.0 — 2026-09-15 — format 0.6 — explicitly-unknown observation time

- New public `EventTime` (with `TimeBasis` and `UnknownTime`): a
  bitemporal time that is either `Known { at, basis }` or
  `Unknown { reason }`, aligned verbatim with the engine's process-mining
  vocabulary (PRODUCT-209). Manual serde: a `Known` with no basis is the
  bare RFC3339 string (v0.5-identical), a basis or an unknown reason
  takes an object form. `JsonSchema` shadow under `schemars`.
- BREAKING: `Observation::observed_at` and `Observation::extracted_at`
  are now `EventTime` (were `DateTime<Utc>`), so a dateless original is
  representable and nothing fabricates an instant for it (PRODUCT-231).
  A crate MINOR on the next release-prep pass, per the train rule above.

## 0.3.1 — 2026-09-14 — format 0.5, schema generated from the types

- `schemars` feature: `JsonSchema` derives on every record type, so the
  published `ant.schema.json` is generated from these types. Off by
  default, like `utoipa`.
- `SamplingMethod::ALL`, `SamplingMethod::wire_name`,
  `SamplingMethod::field_rule` and `SamplingFieldRule`: the presence rule
  per sampling method, which `Sampling::validate` now applies and the
  schema generator publishes as `if`/`then`. Validation behaviour is
  unchanged.

## 0.3.0 — 2026-09-14 — format 0.5

- New module `proposal`: `RelationshipProposal` — what a reconciliation
  loop proposed, how it was measured, and whether it was quarantined —
  with its parts (`ProposedRelation`, `RelationSupport`, `Sampling`,
  `Normalization`, `ProposalOrigin`, `SourceManifestRef`, `ProbeRef`,
  `ReviewerReceipt`), ids `RelationshipProposalId` and
  `ProposalRevisionId`, and the enums `ProposalStatus`, `SupportMethod`,
  `SamplingMethod`, `NormalizationOp`, `CastTarget`;
  `SUPPORT_CONTRACT_VERSION` pins the measurement contract. camelCase
  on the wire. `RelationshipProposal::references()` is the closure list
  and `validate()` the shape check, as for contradiction cases.
- Includes the format 0.4 additions prepared as 0.2.0 (below), which
  were never published.

## 0.2.0 — format 0.4 — prepared 2026-09-10, never published

Superseded by 0.3.0 before it reached the registry; everything below
ships there.

- New module `contradiction`: `ContradictionCase` and its parts
  (`ClaimRef`, `SourcePointer`, `EvidenceRef`, `MeasurementRef`,
  `ComparatorIdentity`, `Material`, `SourceDependency`,
  `VaultOccurrence`) and the three state enums `EpistemicState`,
  `BusinessImpact`, `WorkflowState`. camelCase on the wire.
  `ContradictionCase::references()` is the closure list;
  `validate()` requires two or more claims; `independent_witnesses()`
  excludes forwarded copies and derivations.
- `EvidenceId` derives `ToSchema` under the `utoipa` feature, like the
  other ids.
- docs.rs builds with all features.

## 0.1.1 — 2026-08

- README, docs, license and manifest work since 0.1.0, published as a
  new version because the registry is immutable. No code change.

## 0.1.0

- First publication: ids, graph (`Vertex`, `Edge`, typed
  `PropertyValue` with exact `Decimal`), observation, evidence, belief,
  schema, and the author stamp.
