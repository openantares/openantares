//! `openantares` — validate and inspect `.ant` interchange files.
//!
//! This is a command-line tool, not a library: run `openantares` with
//! no arguments for usage, and see the repository README for the
//! format specification and the full toolchain. A thin CLI over
//! `antares-format`'s reader, with two read-only commands:
//!
//! ```text
//! openantares validate <file.ant> [file2.ant ...]
//! openantares info <file.ant>
//! ```
//!
//! Exit codes follow sysexits, matching the reference bindings'
//! contract: 0 on success, 64 on a usage error, 65 when a file is
//! present but violates the spec (EX_DATAERR), 66 when an input file
//! cannot be opened at all (EX_NOINPUT). `validate` keeps going after
//! a failing file — like the Python and JS binding CLIs — and exits
//! with the most severe code seen.

use std::fs::File;
use std::process::ExitCode;

use antares_format::{AntReader, AntRecord, Counts, Manifest, SUPPORTED_FORMAT_VERSION};

const EX_OK: u8 = 0;
const EX_USAGE: u8 = 64;
const EX_DATAERR: u8 = 65;
const EX_NOINPUT: u8 = 66;

const USAGE: &str =
    "usage:\n  openantares validate <file.ant> [file2.ant ...]\n  openantares info <file.ant>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("validate") if args.len() >= 3 => validate_cmd(&args[2..]),
        Some("info") if args.len() == 3 => info_cmd(&args[2]),
        _ => {
            eprintln!("{USAGE}");
            EX_USAGE
        }
    };
    ExitCode::from(code)
}

/// Fully read one file, enforcing every spec rule (trailer hash,
/// counts, version policy). Returns the verified summary.
fn read_all(path: &str) -> Result<(Manifest, Counts, usize, bool), (u8, String)> {
    let f = File::open(path).map_err(|e| (EX_NOINPUT, format!("cannot open: {e}")))?;
    let mut reader = AntReader::new(f).map_err(|e| (EX_DATAERR, e.to_string()))?;
    let mut records = 0usize;
    // Tallied here from the public record enum; the reader checks its
    // own tally against the trailer, so a verified read guarantees
    // this one matches the file's trailer too.
    let mut counts = Counts::default();
    loop {
        match reader.next_record() {
            Ok(Some(rec)) => {
                records += 1;
                match &rec {
                    AntRecord::SchemaType { .. } => counts.schema_types += 1,
                    AntRecord::Vertex { .. } => counts.vertices += 1,
                    AntRecord::Edge { .. } => counts.edges += 1,
                    AntRecord::Observation { .. } => counts.observations += 1,
                    AntRecord::Evidence { .. } => counts.evidence += 1,
                    AntRecord::Belief { .. } => counts.beliefs += 1,
                    AntRecord::Vector { .. } => counts.vectors += 1,
                    AntRecord::VertexTombstone { .. } => counts.vertex_tombstones += 1,
                    AntRecord::EdgeTombstone { .. } => counts.edge_tombstones += 1,
                    AntRecord::Manifest(_) | AntRecord::Trailer { .. } => {}
                }
            }
            Ok(None) => break,
            Err(e) => return Err((EX_DATAERR, e.to_string())),
        }
    }
    // `Ok(None)` is only returned after the trailer verified, but the
    // flag is the contract — assert it rather than assume it.
    assert!(reader.verified, "reader finished without verification");
    Ok((reader.manifest, counts, records, reader.minor_ahead))
}

fn validate_cmd(files: &[String]) -> u8 {
    let mut rc = EX_OK;
    for path in files {
        match read_all(path) {
            Ok((manifest, _counts, records, minor_ahead)) => {
                let ahead = if minor_ahead {
                    "  (file minor version is ahead of this reader; unknown record kinds were skipped)"
                } else {
                    ""
                };
                println!(
                    "{path}: OK  version={} records={records}{ahead}",
                    manifest.version
                );
            }
            Err((code, msg)) => {
                eprintln!("{path}: FAIL  {msg}");
                rc = rc.max(code);
            }
        }
    }
    rc
}

fn info_cmd(path: &str) -> u8 {
    match read_all(path) {
        Ok((manifest, counts, records, minor_ahead)) => {
            println!("file:            {path}");
            println!("format:          {}", manifest.format);
            println!(
                "format version:  {} (this build reads/writes {SUPPORTED_FORMAT_VERSION}.x{})",
                manifest.version,
                if minor_ahead {
                    "; file is minor-ahead"
                } else {
                    ""
                }
            );
            println!("tenant id:       {}", manifest.tenant_id);
            println!("project id:      {}", manifest.project_id);
            if let Some(p) = &manifest.producer {
                println!("producer:        {p}");
            }
            if let Some(t) = &manifest.created_at {
                println!("created at:      {t}");
            }
            if let Some(s) = &manifest.selection {
                println!("selection:       {s}");
            }
            println!("records:         {records}");
            print_counts(&counts);
            EX_OK
        }
        Err((code, msg)) => {
            eprintln!("{path}: FAIL  {msg}");
            code
        }
    }
}

fn print_counts(c: &Counts) {
    println!("counts:");
    println!("  schema types:      {}", c.schema_types);
    println!("  vertices:          {}", c.vertices);
    println!("  edges:             {}", c.edges);
    println!("  observations:      {}", c.observations);
    println!("  evidence:          {}", c.evidence);
    println!("  beliefs:           {}", c.beliefs);
    println!("  vectors:           {}", c.vectors);
    println!("  vertex tombstones: {}", c.vertex_tombstones);
    println!("  edge tombstones:   {}", c.edge_tombstones);
}
