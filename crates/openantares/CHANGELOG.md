# Changelog — openantares (CLI)

Moves with `antares-format`; see `ant-types/CHANGELOG.md` for the
train rule.

## 0.3.1 — 2026-09-14 — format 0.5, schema generated from the types

- No CLI change. Builds against the `Unreleased` crates above.

## 0.3.0 — 2026-09-14 — format 0.5

- `info` tallies and prints `relationship proposals`.
- Requires `antares-format` 0.3. Includes the format 0.4 additions
  prepared as 0.2.0 (below), which were never published.

## 0.2.0 — format 0.4 — prepared 2026-09-10, never published

Superseded by 0.3.0 before it reached the registry; everything below
ships there.

- `info` tallies and prints `contradiction cases`.
- A bare invocation prints usage on stderr, exits 64 and writes
  nothing, asserted at the process boundary.
- Requires `antares-format` 0.2.

## 0.1.1 — 2026-08

- README, docs, license, manifest. No code change.

## 0.1.0

- First publication: `validate <file>...` and `info <file>` over the
  canonical reader; exit codes 0 / 64 usage / 65 data / 66 no input.
