# Changelog — ant-types

Crate versions follow Cargo's semver rules for 0.x: the second number
moves for anything a downstream build can break on (a new variant in an
exhaustively matched enum, a new field on a plainly constructed struct),
the third for docs, metadata and additive-only changes. A `.ant` FORMAT
minor that adds a record kind is therefore a crate MINOR across the
publishable train (`ant-types`, `antares-format`, `openantares`), which
moves together so the registry stays coherent in dependency order.

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
