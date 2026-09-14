//! The canonical Rust reader against the OpenAntares conformance
//! goldens (openantares/conformance/golden/*). If the format drifts,
//! this fails before the Python/JS runners ever see it. Regenerate
//! goldens deliberately with:
//!   cargo run -p antares-format --example gen_conformance

use std::path::PathBuf;

use antares_format::{AntReader, AntRecord};

/// Directory holding the conformance goldens.
///
/// Defaults to this repo's own copy. `ANT_CONFORMANCE_GOLDEN` overrides
/// it so the same runner works when the goldens live somewhere else —
/// e.g. a checkout layout that keeps them in a different directory.
/// Without the override a layout change turns into a "missing golden"
/// panic that reads like a broken test rather than a moved file.
fn golden_dir() -> PathBuf {
    match std::env::var_os("ANT_CONFORMANCE_GOLDEN") {
        Some(dir) => PathBuf::from(dir),
        None => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openantares/conformance/golden")
        }
    }
}

fn golden(name: &str) -> Vec<u8> {
    let dir = golden_dir();
    let path = dir.join(name);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}\n\
             The conformance goldens were not found. Set ANT_CONFORMANCE_GOLDEN to \
             the directory holding them (e.g. the openantares/ant checkout) if they \
             do not live at {}.",
            path.display(),
            dir.display()
        )
    })
}

#[test]
fn basic_golden_verifies_with_expected_counts() {
    let bytes = golden("basic.ant");
    let mut r = AntReader::new(&bytes[..]).expect("golden must open");
    assert_eq!(r.manifest.tenant_id, 1);
    assert_eq!(r.manifest.project_id, 1);
    let mut kinds = Vec::new();
    while let Some(rec) = r.next_record().expect("golden must read clean") {
        kinds.push(match rec {
            AntRecord::SchemaType { .. } => "schema_type",
            AntRecord::Vertex { .. } => "vertex",
            AntRecord::Edge { .. } => "edge",
            AntRecord::Observation { .. } => "observation",
            AntRecord::Evidence { .. } => "evidence",
            AntRecord::Belief { .. } => "belief",
            AntRecord::Vector { .. } => "vector",
            AntRecord::VertexTombstone { .. } => "vertex_tombstone",
            AntRecord::EdgeTombstone { .. } => "edge_tombstone",
            AntRecord::ContradictionCase { .. } => "contradiction_case",
            AntRecord::RelationshipProposal { .. } => "relationship_proposal",
            AntRecord::Manifest(_) | AntRecord::Trailer { .. } => unreachable!(),
        });
    }
    assert!(r.verified, "trailer sha256 + counts must verify");
    assert_eq!(
        kinds,
        vec![
            "vertex",
            "vertex",
            "edge",
            "observation",
            "evidence",
            "belief",
            "vector"
        ],
        "golden record sequence drifted — regenerate deliberately or fix the format"
    );
}

#[test]
fn forward_compat_golden_skips_unknown_kind_and_verifies() {
    let bytes = golden("forward_compat.ant");
    let mut r = AntReader::new(&bytes[..]).unwrap();
    let mut n = 0;
    while let Some(rec) = r.next_record().unwrap() {
        assert!(matches!(rec, AntRecord::Vertex { .. }));
        n += 1;
    }
    assert!(r.verified);
    assert_eq!(n, 1, "the hologram record must be skipped, the vertex kept");
}

// ---------------------------------------------------------------------
// Version compatibility policy. Same major reads, different major is
// refused with a reason, unknown kinds inside a same-major file skip.
// ---------------------------------------------------------------------

use antares_format::{Counts, FormatVersion, Manifest, FORMAT_VERSION};
use sha2::{Digest, Sha256};

/// Hand-build a stream at an arbitrary version, optionally carrying a
/// record kind this build does not know.
fn stream_at(version: &str, extra_kind: Option<&str>) -> Vec<u8> {
    let manifest = Manifest {
        format: "antares".into(),
        version: version.into(),
        tenant_id: 1,
        project_id: 1,
        selection: None,
        created_at: None,
        producer: Some("version-policy-test".into()),
    };
    let m = serde_json::to_string(&AntRecord::Manifest(manifest)).unwrap();
    let v = serde_json::to_string(&AntRecord::Vertex {
        data: ant_types::Vertex {
            id: ant_types::VertexId("v1".into()),
            name: "V1".into(),
            label: ant_types::TypeName("Antares.Deal".into()),
            properties: Default::default(),
        },
    })
    .unwrap();

    let mut lines: Vec<String> = vec![m, v];
    if let Some(kind) = extra_kind {
        lines.push(format!(r#"{{"kind":"{kind}","data":{{"future":true}}}}"#));
    }
    let mut hasher = Sha256::new();
    for l in &lines {
        hasher.update(l.as_bytes());
        hasher.update(b"\n");
    }
    let trailer = AntRecord::Trailer {
        counts: Counts {
            vertices: 1,
            ..Default::default()
        },
        sha256: format!("{:x}", hasher.finalize()),
    };
    let mut raw = lines.join("\n");
    raw.push('\n');
    raw.push_str(&serde_json::to_string(&trailer).unwrap());
    raw.push('\n');
    zstd::stream::encode_all(raw.as_bytes(), 0).unwrap()
}

fn read_all(bytes: &[u8]) -> Result<(bool, bool, usize), antares_format::AntError> {
    let mut r = AntReader::new(bytes)?;
    let mut n = 0;
    while r.next_record()?.is_some() {
        n += 1;
    }
    Ok((r.verified, r.minor_ahead, n))
}

#[test]
fn version_parses_major_minor() {
    assert_eq!(
        FormatVersion::parse("0.1"),
        Some(FormatVersion { major: 0, minor: 1 })
    );
    assert_eq!(
        FormatVersion::parse("0.2"),
        Some(FormatVersion { major: 0, minor: 2 })
    );
    assert_eq!(
        FormatVersion::parse("1"),
        Some(FormatVersion { major: 1, minor: 0 }),
        "a bare major means .0"
    );
    assert_eq!(FormatVersion::parse("nonsense"), None);
    assert_eq!(FormatVersion::CURRENT.to_string(), FORMAT_VERSION);
}

/// The whole point of the policy: an additive minor bump must not lock
/// out an older reader.
#[test]
fn same_major_newer_minor_is_readable() {
    // Must name a version strictly AHEAD of this build (now 0.5), or
    // the test stops exercising the forward-compat path it exists for.
    let (verified, ahead, n) = read_all(&stream_at("0.6", None)).expect("v0.6 must be readable");
    assert!(verified, "trailer still verifies across a minor bump");
    assert_eq!(n, 1);
    assert!(ahead, "the reader must know the file is ahead of it");
}

#[test]
fn same_major_newer_minor_with_unknown_kind_skips_cleanly() {
    let (verified, ahead, n) =
        read_all(&stream_at("0.9", Some("tombstone_from_the_future"))).expect("must be readable");
    assert!(
        verified,
        "an unknown kind is hashed and skipped, so integrity holds"
    );
    assert_eq!(n, 1, "only the known vertex is surfaced");
    assert!(ahead);
}

#[test]
fn older_minor_is_readable_and_not_flagged_ahead() {
    let (verified, ahead, n) = read_all(&stream_at("0.0", None)).expect("v0.0 must be readable");
    assert!(verified);
    assert_eq!(n, 1);
    assert!(!ahead, "an older file is not ahead of this reader");
}

/// A major bump means an old reader would MISREAD the file, so refusing
/// is the only safe answer — and the message has to say that.
#[test]
fn different_major_is_refused_with_a_reason() {
    let err = read_all(&stream_at("1.0", None)).expect_err("v1.0 must be refused");
    let msg = format!("{err}");
    assert!(msg.contains("v1.0"), "must name the file's version: {msg}");
    assert!(
        msg.contains(FORMAT_VERSION),
        "must name the reader's version: {msg}"
    );
    assert!(
        msg.contains("Major versions are not compatible"),
        "must say WHY, not just 'mismatch': {msg}"
    );
    assert!(
        msg.contains("Upgrade the reader") && msg.contains("re-export"),
        "must tell the operator what to do about it: {msg}"
    );
}

#[test]
fn a_malformed_version_is_refused_with_a_reason() {
    let err = read_all(&stream_at("banana", None)).expect_err("must be refused");
    let msg = format!("{err}");
    assert!(
        msg.contains("MAJOR.MINOR"),
        "must say what shape was expected: {msg}"
    );
}

// ---------------------------------------------------------------------
// v0.2 goldens: the delete plane, and the major-version negative.
// ---------------------------------------------------------------------

/// A binding that treats `vertex_tombstone` / `edge_tombstone` as
/// unknown kinds still VERIFIES the file (unknown kinds are hashed and
/// skipped), so it would look correct while dropping every deletion.
/// This golden is what distinguishes the two.
#[test]
fn tombstones_golden_surfaces_both_planes_and_counts_them() {
    let bytes = golden("tombstones.ant");
    let mut r = AntReader::new(&bytes[..]).expect("golden must open");
    let mut kinds = Vec::new();
    let mut tomb_ids = Vec::new();
    while let Some(rec) = r.next_record().expect("golden must read clean") {
        match rec {
            AntRecord::Vertex { .. } => kinds.push("vertex"),
            AntRecord::Edge { .. } => kinds.push("edge"),
            AntRecord::VertexTombstone { data } => {
                tomb_ids.push(data.id.clone());
                // The author stamp is advisory provenance and must
                // survive the round trip, but it is never consulted to
                // decide a conflict.
                assert!(
                    data.author.is_some(),
                    "the vertex tombstone golden carries an author stamp"
                );
                kinds.push("vertex_tombstone");
            }
            AntRecord::EdgeTombstone { data } => {
                tomb_ids.push(data.id.clone());
                assert!(
                    data.author.is_none(),
                    "the edge tombstone golden omits the author — both shapes must parse"
                );
                kinds.push("edge_tombstone");
            }
            other => panic!("unexpected record in tombstones.ant: {other:?}"),
        }
    }
    assert!(r.verified, "trailer sha256 + counts must verify");
    assert_eq!(
        kinds,
        vec!["vertex", "edge", "vertex_tombstone", "edge_tombstone"],
    );
    assert_eq!(
        tomb_ids,
        vec!["deal_gone", "deal_gone->acct_1:belongsTo"],
        "tombstone ids must arrive intact — they are what the importer deletes by"
    );
}

/// The negative golden. Valid in every respect except its major
/// version, so a reader that rejects it for any OTHER reason is passing
/// for the wrong reason — hence the message assertions.
#[test]
fn major_version_golden_is_refused() {
    let bytes = golden("major_version.ant");
    let err = read_all(&bytes).expect_err("a v1.0 file must be refused by a 0.x reader");
    let msg = format!("{err}");
    assert!(msg.contains("v1.0"), "must name the file's version: {msg}");
    assert!(
        msg.contains("Major versions are not compatible"),
        "must be refused for the VERSION, not for some other defect: {msg}"
    );
}

/// The v0.3 golden must carry the SQL-parity property types INTACT.
///
/// A conformance vector nobody checks the contents of proves only that
/// the file parses. This is the check another implementation is meant
/// to mirror: read `basic.ant`, and every typed value must come back as
/// its type with its exact value — most of all the decimal, which is
/// past `f64`'s reach and will silently round in any implementation
/// that parses it as a double.
#[test]
fn basic_golden_carries_the_v0_3_typed_properties() {
    use ant_types::PropertyValue as P;

    let bytes = golden("basic.ant");
    let mut r = AntReader::new(&bytes[..]).expect("golden must open");
    let mut deal = None;
    while let Some(rec) = r.next_record().expect("golden must read clean") {
        if let AntRecord::Vertex { data } = rec {
            if data.id.0 == "deal_1" {
                deal = Some(data);
            }
        }
    }
    let p = deal.expect("deal_1 in the golden").properties;

    match p.get("exact_amount") {
        Some(P::Decimal(d)) => assert_eq!(
            d.to_string(),
            "12345678901234567.89",
            "the decimal lost digits — this is what parsing money as f64 looks like"
        ),
        other => panic!("exact_amount is {other:?}, not a Decimal"),
    }
    match p.get("signed_at") {
        Some(P::Timestamp(t)) => assert_eq!(
            t.to_rfc3339(),
            "2026-08-10T09:00:00+02:00",
            "the offset was rewritten; TIMESTAMPTZ must keep it"
        ),
        other => panic!("signed_at is {other:?}, not a Timestamp"),
    }
    assert!(matches!(p.get("closed_on"), Some(P::Date(_))));
    assert!(matches!(p.get("review_at"), Some(P::Time(_))));
    assert!(matches!(p.get("external_id"), Some(P::Uuid(_))));
    assert_eq!(p.get("seal"), Some(&P::Bytes(vec![0x00, 0x01, 0xfe, 0xff])));
    assert_eq!(p.get("headcount"), Some(&P::Int32(1200)));
    assert_eq!(p.get("region_code"), Some(&P::Int16(-7)));
    assert_eq!(
        p.get("tags"),
        Some(&P::Array(vec![
            P::Text("enterprise".into()),
            P::Text("renewal".into()),
        ]))
    );

    // And the v0.2 shapes are untouched, which is what makes this a
    // MINOR bump rather than a breaking one.
    assert_eq!(p.get("amount"), Some(&P::Long(48000)));
    assert_eq!(p.get("stage"), Some(&P::Text("proposal".into())));
    assert!(matches!(p.get("meta"), Some(P::Json(_))));
}

// ---------------------------------------------------------------------
// v0.4 golden: contradiction cases.
// ---------------------------------------------------------------------

/// A binding that treats `contradiction_case` as an unknown kind still
/// VERIFIES this file (unknown kinds are hashed and skipped) while
/// surfacing none of the cases — so the check is on CONTENTS: the four
/// revisions, their three state families, the revision chain, the
/// references, and the witness count that the dependency field exists
/// for.
#[test]
fn contradiction_cases_golden_carries_the_cases_and_their_closure() {
    use ant_types::{BusinessImpact, EpistemicState, WorkflowState};

    let bytes = golden("contradiction_cases.ant");
    let mut r = AntReader::new(&bytes[..]).expect("golden must open");
    let mut kinds = Vec::new();
    let mut cases = Vec::new();
    let mut belief_versions = Vec::new();
    let mut evidence_ids = Vec::new();
    let mut observation_ids = Vec::new();
    while let Some(rec) = r.next_record().expect("golden must read clean") {
        match rec {
            AntRecord::Vertex { .. } => kinds.push("vertex"),
            AntRecord::Evidence { data } => {
                evidence_ids.push(data.id.0);
                kinds.push("evidence");
            }
            AntRecord::Observation { data } => {
                observation_ids.push(data.id.0);
                kinds.push("observation");
            }
            AntRecord::Belief { data } => {
                belief_versions.push((data.id.0, data.belief_version));
                kinds.push("belief");
            }
            AntRecord::ContradictionCase { data } => {
                cases.push(*data);
                kinds.push("contradiction_case");
            }
            other => panic!("unexpected record in contradiction_cases.ant: {other:?}"),
        }
    }
    assert!(r.verified, "trailer sha256 + counts must verify");
    assert_eq!(
        kinds.iter().filter(|k| **k == "contradiction_case").count(),
        4
    );
    assert_eq!(
        cases.iter().map(|c| c.id.0.as_str()).collect::<Vec<_>>(),
        vec!["case_hc_1@1", "case_ms_1@1", "case_wd_1@1", "case_wd_1@2"],
        "four revisions of three cases, a predecessor before its successor"
    );

    // Three independent state families, read back as their own values.
    let states: Vec<(EpistemicState, BusinessImpact, WorkflowState)> = cases
        .iter()
        .map(|c| (c.epistemic, c.impact, c.workflow))
        .collect();
    assert_eq!(
        states,
        vec![
            (
                EpistemicState::Incompatible,
                BusinessImpact::Harmful,
                WorkflowState::AwaitingReview
            ),
            (
                EpistemicState::InsufficientlyComparable,
                BusinessImpact::Unassessed,
                WorkflowState::Settled
            ),
            (
                EpistemicState::Incompatible,
                BusinessImpact::AlignmentOnly,
                WorkflowState::Open
            ),
            (
                EpistemicState::Compatible,
                BusinessImpact::AlignmentOnly,
                WorkflowState::Settled
            ),
        ]
    );

    // The revision chain is a reference, never an edit.
    let wd2 = &cases[3];
    assert_eq!(wd2.case_id.0, "case_wd_1");
    assert_eq!(
        wd2.previous_revision_id.as_ref().map(|p| p.0.as_str()),
        Some("case_wd_1@1")
    );
    assert_eq!(cases[2].case_id, wd2.case_id, "same case, two revisions");

    // The forwarded copy is not a second witness.
    let hc = &cases[0];
    assert_eq!(hc.supporting.len(), 2);
    assert_eq!(hc.independent_witnesses(), 1);
    assert_eq!(hc.measurements.len(), 3);
    assert_eq!(hc.vault_occurrences.len(), 1);

    // Closure, checked against the file itself: every id every case
    // references is present. (An importer's closure verifier applies
    // the same rule to any archive; this is the format-level statement.)
    for c in &cases {
        let refs = c.references();
        for (id, version) in &refs.beliefs {
            assert!(
                belief_versions
                    .iter()
                    .any(|(bid, v)| bid == id && version.is_none_or(|x| x == *v)),
                "case {} cites belief {id}@{version:?}, absent from the golden",
                c.id.0
            );
        }
        for o in &refs.observations {
            assert!(
                observation_ids.contains(o),
                "case {} cites observation {o}",
                c.id.0
            );
        }
        for e in &refs.evidence {
            assert!(
                evidence_ids.contains(e),
                "case {} cites evidence {e}",
                c.id.0
            );
        }
        if let Some(prev) = &refs.previous_revision {
            assert!(
                cases.iter().any(|p| &p.id.0 == prev),
                "case {} cites {prev}",
                c.id.0
            );
        }
    }
}

// ---------------------------------------------------------------------
// v0.5 golden: relationship proposals.
// ---------------------------------------------------------------------

/// The kind exists so a grader reading an ARCHIVE sees what the recon
/// loop proposed, measured and quarantined — which it could not while
/// those lived in receipts local to one server. So the check is on
/// CONTENTS: the three revisions, their statuses, the measurement
/// behind each, the probe that took it, and the closure of everything
/// they cite.
#[test]
fn relationship_proposals_golden_carries_the_proposals_and_their_closure() {
    use ant_types::ProposalStatus;

    let bytes = golden("relationship_proposals.ant");
    let mut r = AntReader::new(&bytes[..]).expect("golden must open");
    let mut proposals = Vec::new();
    let mut evidence_ids = Vec::new();
    while let Some(rec) = r.next_record().expect("golden must read clean") {
        match rec {
            AntRecord::Vertex { .. } => {}
            AntRecord::Evidence { data } => evidence_ids.push(data.id.0),
            AntRecord::RelationshipProposal { data } => proposals.push(*data),
            other => panic!("unexpected record in relationship_proposals.ant: {other:?}"),
        }
    }
    assert!(r.verified, "trailer sha256 + counts must verify");
    assert_eq!(
        proposals
            .iter()
            .map(|p| p.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["prop_ord_cust@1", "prop_color_variant@1", "prop_ord_cust@2"],
        "a predecessor precedes its successor"
    );
    assert_eq!(
        proposals.iter().map(|p| p.status.tag()).collect::<Vec<_>>(),
        vec![
            "supported",
            "quarantined_hypothesis",
            "promoted_by_reviewer"
        ],
    );

    // The quarantined hypothesis: the live shape, 0 of 1,914, with the
    // probe that measured it. This is what a grader reads instead of
    // "zero relationship proposals".
    let q = &proposals[1];
    assert_eq!(q.relation.predicate, "colorVariantOf");
    assert_eq!(q.support.matched_rows, 0);
    assert_eq!(q.support.source_non_null, 1914);
    assert_eq!(q.ratio(), Some(0.0));
    assert_eq!(q.probes.len(), 1);
    assert!(
        q.probes[0].statement.contains("count(*)"),
        "the probe travels with the proposal: {}",
        q.probes[0].statement
    );
    match &q.status {
        ProposalStatus::QuarantinedHypothesis { reason } => {
            assert!(reason.contains("1914"), "{reason}")
        }
        other => panic!("{other:?}"),
    }

    // Promotion is an act with a receipt, and the receipt is a record
    // in the file — which is what closure buys.
    let promoted = &proposals[2];
    assert_eq!(promoted.proposal_id.0, "prop_ord_cust");
    assert_eq!(
        promoted.previous_revision_id.as_ref().map(|p| p.0.as_str()),
        Some("prop_ord_cust@1")
    );
    match &promoted.status {
        ProposalStatus::PromotedByReviewer { receipt } => {
            assert!(!receipt.reviewer.is_empty());
            assert!(evidence_ids.contains(&receipt.receipt.0));
        }
        other => panic!("a promotion carries its receipt: {other:?}"),
    }

    // Closure, checked against the file itself.
    for p in &proposals {
        p.validate()
            .expect("every golden proposal is structurally valid");
        let refs = p.references();
        for e in &refs.evidence {
            assert!(
                evidence_ids.contains(e),
                "proposal {} cites evidence {e}, absent from the golden",
                p.id.0
            );
        }
        if let Some(prev) = &refs.previous_revision {
            assert!(
                proposals.iter().any(|q| &q.id.0 == prev),
                "proposal {} cites {prev}",
                p.id.0
            );
        }
    }
}

/// The additive promise, from the other side: a trailer written before
/// v0.5 has no `relationshipProposals` key, and it must read as zero
/// rather than as an error — otherwise every additive kind is a
/// breaking change. A v0.5 trailer carries the key, and a reader that
/// does not know it ignores it (serde drops unknown fields).
#[test]
fn an_older_trailer_reads_the_new_count_as_zero_and_a_newer_one_is_ignored() {
    let v04: Counts = serde_json::from_str(
        r#"{"schemaTypes":0,"vertices":1,"edges":0,"observations":0,"evidence":0,
            "beliefs":0,"vectors":0,"vertexTombstones":0,"edgeTombstones":0,
            "contradictionCases":2}"#,
    )
    .expect("a v0.4 trailer still parses");
    assert_eq!(v04.contradiction_cases, 2);
    assert_eq!(
        v04.relationship_proposals, 0,
        "absent means zero, never an error"
    );

    let v05 = serde_json::to_value(Counts {
        relationship_proposals: 3,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(v05["relationshipProposals"], 3);

    // A key from a version after this one is ignored, which is the same
    // rule pointed forward.
    let ahead: Counts = serde_json::from_str(
        r#"{"schemaTypes":0,"vertices":0,"edges":0,"observations":0,"evidence":0,
            "beliefs":0,"vectors":0,"relationshipProposals":1,"holograms":9}"#,
    )
    .expect("a later trailer key is not an error");
    assert_eq!(ahead.relationship_proposals, 1);
}
