# openantares

Command-line tool for validating and inspecting Open Antares (`.ant`)
interchange files.

```sh
openantares validate world.ant more.ant   # full spec validation
openantares info world.ant                # manifest + per-section counts
```

`validate` reads each file to the end, enforcing every container rule
— framing, manifest, version policy, and the trailer's integrity hash
and per-kind counts. A file that carries stored originals (format
1.0) is checked through to the bytes: every chunk against its digest
and each whole original against its evidence's `source_blob`. It
keeps going after a failing file and exits with the most severe code
seen. `info` prints the manifest (format
version, tenant/project ids, producer, selection) and the per-section
record counts; both commands are read-only.

Exit codes follow sysexits, matching the format's reference bindings:

| code | meaning |
|------|-------------------------------------------|
| 0    | success |
| 64   | usage error |
| 65   | a file is present but violates the spec |
| 66   | an input file cannot be opened |

Built on [`antares-format`](https://crates.io/crates/antares-format),
the canonical Rust implementation of the container. The format
specification, JSON Schema, reference bindings, and conformance suite
live in the repository.

## License

Apache-2.0.
