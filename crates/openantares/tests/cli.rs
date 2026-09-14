//! CLI acceptance: exit contract (0 / 64 usage / 65 data / 66
//! no-input) and output shape, driven against the conformance goldens
//! — the same corpus the bindings' CLIs are proven on.

use std::path::PathBuf;
use std::process::{Command, Output};

/// `ANT_CONFORMANCE_GOLDEN` overrides the default in-repo location,
/// same as antares-format's conformance runner, so the test works in
/// checkouts that keep the goldens elsewhere.
fn golden(name: &str) -> PathBuf {
    let dir = match std::env::var_os("ANT_CONFORMANCE_GOLDEN") {
        Some(d) => PathBuf::from(d),
        None => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openantares/conformance/golden")
        }
    };
    dir.join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openantares"))
        .args(args)
        .output()
        .expect("spawn openantares")
}

#[test]
fn no_args_is_usage_64() {
    // In an empty directory, so that "did it write anything?" is a
    // question with an exact answer: a bare invocation must leave the
    // working directory as it found it (PRODUCT-232).
    let dir = tempdir().join("bare-run");
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openantares"))
        .current_dir(&dir)
        .output()
        .expect("spawn openantares");
    assert_eq!(out.status.code(), Some(64), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stderr).contains("usage:"));
    assert!(
        text(&out.stdout).is_empty(),
        "usage for misuse goes to stderr"
    );
    let left: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "a bare run must create nothing: {left:?}");
}

#[test]
fn unknown_command_is_usage_64() {
    let out = run(&["frobnicate", "x.ant"]);
    assert_eq!(out.status.code(), Some(64), "stderr: {}", text(&out.stderr));
}

#[test]
fn validate_ok_is_0_with_ok_line() {
    let p = golden("basic.ant");
    let out = run(&["validate", p.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {} stderr: {}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let stdout = text(&out.stdout);
    assert!(stdout.contains(": OK"), "stdout: {stdout}");
    assert!(stdout.contains("version=0.5"), "stdout: {stdout}");
}

#[test]
fn validate_major_version_file_is_65() {
    // major_version.ant is the negative golden: a different MAJOR must
    // be refused (spec §2), which is EX_DATAERR for the CLI.
    let p = golden("major_version.ant");
    let out = run(&["validate", p.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(65),
        "stdout: {} stderr: {}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert!(text(&out.stderr).contains("FAIL"));
}

#[test]
fn validate_truncated_stream_is_65() {
    // Chop the tail off a valid golden: the zstd frame (or trailer)
    // is now broken and validation must fail with EX_DATAERR.
    let bytes = std::fs::read(golden("basic.ant")).unwrap();
    let dir = tempdir();
    let p = dir.join("truncated.ant");
    std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
    let out = run(&["validate", p.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(65), "stderr: {}", text(&out.stderr));
}

#[test]
fn validate_missing_file_is_66() {
    let out = run(&["validate", "/nonexistent/definitely-not-here.ant"]);
    assert_eq!(
        out.status.code(),
        Some(66),
        "stdout: {} stderr: {}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert!(text(&out.stderr).contains("FAIL"));
}

#[test]
fn validate_keeps_going_and_reports_most_severe() {
    // ok file + data-error file + missing file in one run: every file
    // gets a line, and the exit is the most severe (66).
    let ok = golden("basic.ant");
    let bad = golden("major_version.ant");
    let out = run(&[
        "validate",
        ok.to_str().unwrap(),
        bad.to_str().unwrap(),
        "/nonexistent/definitely-not-here.ant",
    ]);
    assert_eq!(out.status.code(), Some(66), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stdout).contains(": OK"), "ok file still reported");
    assert_eq!(
        text(&out.stderr).matches("FAIL").count(),
        2,
        "both failing files reported: {}",
        text(&out.stderr)
    );
}

#[test]
fn info_reports_manifest_and_counts_matching_expected_json() {
    let p = golden("basic.ant");
    let out = run(&["info", p.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", text(&out.stderr));
    let stdout = text(&out.stdout);
    // Pinned against conformance/golden/expected.json for basic.ant.
    for needle in [
        "format:          antares",
        "tenant id:       1",
        "project id:      1",
        "records:         7",
        "vertices:          2",
        "edges:             1",
        "observations:      1",
        "evidence:          1",
        "beliefs:           1",
        "vectors:           1",
        "vertex tombstones: 0",
        "contradiction cases: 0",
        "relationship proposals: 0",
    ] {
        assert!(
            needle_in(&stdout, needle),
            "missing `{needle}` in:\n{stdout}"
        );
    }
    assert!(stdout.contains("0.5"), "format version shown: {stdout}");
}

#[test]
fn info_takes_exactly_one_file() {
    let p = golden("basic.ant");
    let out = run(&["info", p.to_str().unwrap(), p.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(64), "stderr: {}", text(&out.stderr));
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn needle_in(hay: &str, needle: &str) -> bool {
    hay.contains(needle)
}

fn tempdir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("openantares-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
