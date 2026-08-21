//! Regenerate the OpenAntares conformance golden files from the
//! canonical Rust writer.
//!
//!   cargo run -p antares-format --example gen_conformance
//!
//! Deterministic by construction (fixed ids/timestamps, no clock or
//! RNG), so a regeneration only changes bytes when the format itself
//! changes — which is exactly what the conformance suite must catch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ant_types::{
    AuthorStamp, Belief, BeliefId, Edge, EdgeId, Evidence, EvidenceId, Observation, ObservationId,
    ProjectId, PropertyValue, SubjectType, TenantId, TypeName, UserId, Vertex, VertexId,
};
use antares_format::{
    AntRecord, AntWriter, Counts, Manifest, Tombstone, VectorRecord, FORMAT_VERSION,
};
use sha2::{Digest, Sha256};

fn manifest() -> Manifest {
    Manifest {
        format: "antares".into(),
        version: FORMAT_VERSION.into(),
        tenant_id: 1,
        project_id: 1,
        selection: Some(serde_json::json!({"kind": "whole_scope"})),
        created_at: Some("2026-08-10T00:00:00Z".parse().unwrap()),
        producer: Some("openantares-conformance/0.1".into()),
    }
}

/// Where regenerated goldens are written. Mirrors `tests/conformance.rs`
/// so the generator and the runner can never disagree about the
/// location: both default to this repo's copy and both honour
/// `ANT_CONFORMANCE_GOLDEN`.
fn out_dir() -> PathBuf {
    match std::env::var_os("ANT_CONFORMANCE_GOLDEN") {
        Some(dir) => PathBuf::from(dir),
        // crates/antares-format -> repo root -> openantares/conformance/golden
        None => {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../openantares/conformance/golden")
        }
    }
}

fn basic() -> Vec<u8> {
    let mut props = BTreeMap::new();
    props.insert("amount".into(), PropertyValue::Long(48000));
    props.insert("stage".into(), PropertyValue::Text("proposal".into()));
    props.insert(
        "meta".into(),
        PropertyValue::Json(serde_json::json!({"nested": [1, 2], "flag": true})),
    );
    // v0.3 SQL-parity types. A conformance vector is how another
    // implementation proves it reads the tagged envelopes, so the
    // golden carries one of each rather than describing them in prose.
    // The amount is deliberately past f64's ~15-16 significant digits:
    // an implementation that parses decimals as doubles fails here
    // instead of silently rounding a customer's money.
    props.insert(
        "exact_amount".into(),
        PropertyValue::Decimal(ant_types::Decimal::parse("12345678901234567.89").unwrap()),
    );
    props.insert(
        "closed_on".into(),
        PropertyValue::Date(chrono::NaiveDate::from_ymd_opt(2026, 8, 10).unwrap()),
    );
    props.insert(
        "review_at".into(),
        PropertyValue::Time(chrono::NaiveTime::from_hms_micro_opt(14, 30, 0, 500_000).unwrap()),
    );
    // A non-UTC offset, so a reader that normalizes to UTC is caught.
    props.insert(
        "signed_at".into(),
        PropertyValue::Timestamp(
            chrono::DateTime::parse_from_rfc3339("2026-08-10T09:00:00+02:00").unwrap(),
        ),
    );
    props.insert(
        "external_id".into(),
        PropertyValue::Uuid(uuid::Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap()),
    );
    props.insert(
        "seal".into(),
        PropertyValue::Bytes(vec![0x00, 0x01, 0xfe, 0xff]),
    );
    props.insert("headcount".into(), PropertyValue::Int32(1200));
    props.insert("region_code".into(), PropertyValue::Int16(-7));
    props.insert(
        "tags".into(),
        PropertyValue::Array(vec![
            PropertyValue::Text("enterprise".into()),
            PropertyValue::Text("renewal".into()),
        ]),
    );
    let deal = Vertex {
        id: VertexId("deal_1".into()),
        name: "Hooli Enterprise Proposal".into(),
        label: TypeName("Antares.Deal".into()),
        properties: props,
    };
    let company = Vertex {
        id: VertexId("acct_1".into()),
        name: "Hooli Corp".into(),
        label: TypeName("Antares.Company".into()),
        properties: BTreeMap::new(),
    };
    let edge = Edge {
        id: EdgeId("deal_1->acct_1:belongsTo".into()),
        src: VertexId("deal_1".into()),
        src_type: TypeName("Antares.Deal".into()),
        dst: VertexId("acct_1".into()),
        dst_type: TypeName("Antares.Company".into()),
        label: "belongsTo".into(),
        properties: BTreeMap::new(),
        valid_from: None,
        valid_to: None,
        observed_at: None,
        extracted_at: None,
        confidence: Some(1.0),
        evidenced_by: vec![EvidenceId("ev_1".into())],
    };
    let evidence = Evidence {
        id: EvidenceId("ev_1".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_uri: "antares://mail/9".into(),
        source_type: "email".into(),
        source_id: "mail-9".into(),
        content: "we moved the deal to proposal stage after the demo call".into(),
        char_start: None,
        char_end: None,
        byte_start: None,
        byte_end: None,
        observed_at: Some("2026-08-09T10:00:00Z".parse().unwrap()),
        extracted_at: Some("2026-08-09T10:00:01Z".parse().unwrap()),
        extractor_version: Some("conformance/1".into()),
        confidence: None,
        metadata: serde_json::Value::Null,
        author: None,
    };
    let obs = Observation {
        id: ObservationId("obs_1".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_event_id: Some("mail-9".into()),
        source_uri: Some("antares://mail/9".into()),
        subject_id: Some(VertexId("deal_1".into())),
        predicate: "stage_change".into(),
        object_id: None,
        object_value: Some(serde_json::json!("proposal")),
        observed_at: "2026-08-09T10:00:00Z".parse().unwrap(),
        extracted_at: "2026-08-09T10:00:01Z".parse().unwrap(),
        confidence: Some(0.9),
        evidence_ids: vec![EvidenceId("ev_1".into())],
        extractor_version: Some("conformance/1".into()),
        metadata: serde_json::Value::Null,
        author: None,
    };
    let belief = Belief {
        id: BeliefId("bel_1".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        subject_id: VertexId("deal_1".into()),
        predicate: "stage".into(),
        value_json: serde_json::json!("proposal"),
        confidence: Some(0.9),
        belief_version: 1,
        derived_from: vec![ObservationId("obs_1".into())],
        evidence_ids: vec![EvidenceId("ev_1".into())],
        valid_from: None,
        valid_to: None,
        observed_at: Some("2026-08-09T10:00:00Z".parse().unwrap()),
        updated_at: "2026-08-09T10:00:02Z".parse().unwrap(),
        decay_policy: None,
        metadata: serde_json::Value::Null,
        author: None,
        contributing_authors: Vec::new(),
    };
    let vector = VectorRecord {
        record_type: "Evidence".into(),
        record_id: "ev_1".into(),
        label: "Antares.Chunk".into(),
        field: "content".into(),
        vector: vec![0.25, -0.5, 0.125],
        text_preview: Some("we moved the deal".into()),
        evidence_ids: vec!["ev_1".into()],
    };

    let mut w = AntWriter::new(Vec::new(), manifest(), 0).expect("writer");
    w.write(AntRecord::Vertex { data: deal }).unwrap();
    w.write(AntRecord::Vertex { data: company }).unwrap();
    w.write(AntRecord::Edge { data: edge }).unwrap();
    w.write(AntRecord::Observation { data: obs }).unwrap();
    w.write(AntRecord::Evidence { data: evidence }).unwrap();
    w.write(AntRecord::Belief { data: belief }).unwrap();
    w.write(AntRecord::Vector { data: vector }).unwrap();
    w.finish().expect("finish")
}

/// Hand-built stream carrying an unknown record kind (`hologram`) that
/// v0.1 readers must skip while still verifying the trailer.
fn forward_compat() -> Vec<u8> {
    let m = serde_json::to_string(&AntRecord::Manifest(manifest())).unwrap();
    let v = serde_json::to_string(&AntRecord::Vertex {
        data: Vertex {
            id: VertexId("v_known".into()),
            name: "Known".into(),
            label: TypeName("Antares.Deal".into()),
            properties: BTreeMap::new(),
        },
    })
    .unwrap();
    let unknown = r#"{"kind":"hologram","data":{"future":true}}"#;
    let mut hasher = Sha256::new();
    for line in [m.as_str(), v.as_str(), unknown] {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let trailer = AntRecord::Trailer {
        counts: Counts {
            vertices: 1,
            ..Default::default()
        },
        sha256: format!("{:x}", hasher.finalize()),
    };
    let t = serde_json::to_string(&trailer).unwrap();
    let raw = format!("{m}\n{v}\n{unknown}\n{t}\n");
    zstd::stream::encode_all(raw.as_bytes(), 0).unwrap()
}

/// v0.2 file exercising the DELETE path: two live records, then a
/// tombstone for each plane. Other bindings need this to prove they
/// surface `vertex_tombstone` / `edge_tombstone` as records and count
/// them in the trailer — a binding that silently skips them as
/// "unknown kinds" still verifies, and would look correct while
/// dropping every deletion on the floor.
fn tombstones() -> Vec<u8> {
    let live = Vertex {
        id: VertexId("deal_live".into()),
        name: "Still Here".into(),
        label: TypeName("Antares.Deal".into()),
        properties: BTreeMap::new(),
    };
    let edge = Edge {
        id: EdgeId("deal_live->acct_1:belongsTo".into()),
        src: VertexId("deal_live".into()),
        src_type: TypeName("Antares.Deal".into()),
        dst: VertexId("acct_1".into()),
        dst_type: TypeName("Antares.Company".into()),
        label: "belongsTo".into(),
        properties: BTreeMap::new(),
        valid_from: None,
        valid_to: None,
        observed_at: None,
        extracted_at: None,
        confidence: None,
        evidenced_by: Vec::new(),
    };
    // With an author stamp: advisory provenance, never used to decide a
    // conflict. A binding must accept it and must also accept its
    // absence (the edge tombstone below omits it).
    let vtomb = Tombstone {
        id: "deal_gone".into(),
        deleted_at: "2026-08-10T09:15:00Z".parse().unwrap(),
        author: Some(AuthorStamp {
            user_id: UserId("u_42".into()),
            token_id: None,
            subject_type: SubjectType::User,
            authored_at: "2026-08-10T09:15:00Z".parse().unwrap(),
        }),
    };
    let etomb = Tombstone {
        id: "deal_gone->acct_1:belongsTo".into(),
        deleted_at: "2026-08-10T09:15:00Z".parse().unwrap(),
        author: None,
    };

    let mut w = AntWriter::new(Vec::new(), manifest(), 0).expect("writer");
    w.write(AntRecord::Vertex { data: live }).unwrap();
    w.write(AntRecord::Edge { data: edge }).unwrap();
    w.write(AntRecord::VertexTombstone { data: vtomb }).unwrap();
    w.write(AntRecord::EdgeTombstone { data: etomb }).unwrap();
    w.finish().expect("finish")
}

/// A NEGATIVE golden: a well-formed, correctly-hashed stream that
/// declares format v1.0. Every 0.x reader must REFUSE it.
///
/// This is the one case where refusing is the only safe answer: a major
/// bump means field meanings or the container framing changed, so a
/// reader that "did its best" would return plausible wrong answers
/// instead of an error. It is a shared fixture rather than a
/// per-runner synthetic because it is the rule most likely to be
/// implemented as `version == "0.2"`, which passes every positive test
/// while being wrong.
fn major_version() -> Vec<u8> {
    let mut m = manifest();
    m.version = "1.0".into();
    let m = serde_json::to_string(&AntRecord::Manifest(m)).unwrap();
    let v = serde_json::to_string(&AntRecord::Vertex {
        data: Vertex {
            id: VertexId("v_future".into()),
            name: "From A Later Major".into(),
            label: TypeName("Antares.Deal".into()),
            properties: BTreeMap::new(),
        },
    })
    .unwrap();
    let mut hasher = Sha256::new();
    for line in [m.as_str(), v.as_str()] {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let trailer = AntRecord::Trailer {
        counts: Counts {
            vertices: 1,
            ..Default::default()
        },
        sha256: format!("{:x}", hasher.finalize()),
    };
    let t = serde_json::to_string(&trailer).unwrap();
    // Deliberately valid in every other respect: the ONLY reason to
    // reject it is the major version. A reader that fails this for any
    // other reason is failing for the wrong reason.
    let raw = format!("{m}\n{v}\n{t}\n");
    zstd::stream::encode_all(raw.as_bytes(), 0).unwrap()
}

fn main() {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("mkdir golden");
    std::fs::write(dir.join("basic.ant"), basic()).expect("write basic.ant");
    std::fs::write(dir.join("forward_compat.ant"), forward_compat())
        .expect("write forward_compat.ant");
    std::fs::write(dir.join("tombstones.ant"), tombstones()).expect("write tombstones.ant");
    std::fs::write(dir.join("major_version.ant"), major_version())
        .expect("write major_version.ant");
    let expected = serde_json::json!({
        "basic.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 2, "edges": 1,
                        "observations": 1, "evidence": 1, "beliefs": 1, "vectors": 1,
                        "vertexTombstones": 0, "edgeTombstones": 0},
            "recordKinds": ["vertex", "vertex", "edge", "observation",
                             "evidence", "belief", "vector"],
            "firstVertexId": "deal_1"
        },
        "forward_compat.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 1, "edges": 0,
                        "observations": 0, "evidence": 0, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0},
            "recordKinds": ["vertex"],
            "skippedKinds": ["hologram"]
        },
        "tombstones.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 1, "edges": 1,
                        "observations": 0, "evidence": 0, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 1, "edgeTombstones": 1},
            "recordKinds": ["vertex", "edge", "vertex_tombstone", "edge_tombstone"],
            "firstVertexId": "deal_live"
        }
    });
    std::fs::write(
        dir.join("expected.json"),
        serde_json::to_string_pretty(&expected).unwrap(),
    )
    .expect("write expected.json");

    // Negatives live in their own file so a runner can keep iterating
    // expected.json as "files that must READ", with no special-casing.
    // The contract here is deliberately just "must be rejected": the
    // error TEXT differs per implementation and pinning it would make
    // the fixture untestable outside Rust.
    let negatives = serde_json::json!({
        "major_version.ant": {
            "mustReject": true,
            "why": "declares format v1.0; a 0.x reader must refuse rather than \
                    misread, because a major bump means field meanings or the \
                    container framing changed. Valid in every other respect, so \
                    rejecting it for any other reason is the wrong pass."
        }
    });
    std::fs::write(
        dir.join("expected_negatives.json"),
        serde_json::to_string_pretty(&negatives).unwrap(),
    )
    .expect("write expected_negatives.json");
    eprintln!("wrote goldens to {}", dir.display());
}
