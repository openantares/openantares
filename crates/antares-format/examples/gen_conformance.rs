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
    AuthorStamp, Belief, BeliefId, BusinessImpact, CaseRevisionId, ClaimKind, ClaimRef,
    ComparatorIdentity, ContradictionCase, ContradictionCaseId, Edge, EdgeId, EpistemicState,
    Evidence, EvidenceId, Material, MeasurementRef, Normalization, NormalizationOp, Observation,
    ObservationId, OntologyApprovalAttestation, OntologyApprovalBinding, OntologyAttribution,
    OntologyConditionalPosition, OntologyPositionDisposition, OntologyPublisherStamp,
    OntologyRecordKind, OntologyRecordRef, OntologyRetainedPosition, OntologyRevision,
    OntologyRevisionId, OntologyRevisionManifest, OntologySemanticItem, OntologySourceOwnerConsent,
    OntologyVaultPin, ProbeRef, ProjectId, PropertyDef, PropertyValue, ProposalOrigin,
    ProposalRevisionId, ProposalStatus, ProposedRelation, RelationSupport, RelationshipProposal,
    RelationshipProposalId, ReviewerReceipt, Sampling, SchemaType, SourceBlob, SourceDependency,
    SourceManifestRef, SourcePointer, SourceReference, SpgTypeKind, SubjectType, SupportMethod,
    TenantId, TokenId, TypeName, UserId, ValueType, VaultOccurrence, Vertex, VertexId,
    WorkflowState, ONTOLOGY_CHAIN_ID, ONTOLOGY_REVISION_DOMAIN,
};
use antares_format::{
    AntRecord, AntWriter, Counts, Manifest, OriginalChunk, Tombstone, VectorRecord, FORMAT_VERSION,
    ORIGINALS_FORMAT_VERSION,
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
        source_blob: None,
        derivation: None,
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
        observed_at: ant_types::EventTime::known("2026-08-09T10:00:00Z".parse().unwrap()),
        extracted_at: ant_types::EventTime::known("2026-08-09T10:00:01Z".parse().unwrap()),
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
    // Neither 0.x nor 1.x (stored originals): a reader of both majors
    // must refuse it.
    m.version = "2.0".into();
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

/// v0.4 golden: contradiction cases and their WHOLE closure. Three
/// cases, one of them in two revisions, every one a reference to
/// records that are in the same file:
///
/// * `case_hc_1` — a synthetic INCOMPATIBLE pair: a belief of 1200
///   employees against an observation of 900 for the same company,
///   with the two measurements, a supporting source and its forwarded
///   copy (ONE independent witness, not two), a review receipt and a
///   vault occurrence.
/// * `case_ms_1` — a NON-case: a model matched two different companies
///   as one subject. Epistemic `insufficiently_comparable`, settled on a
///   review receipt that says so.
/// * `case_wd_1` — a NON-case in two revisions: opened as incompatible,
///   then the lower figure was withdrawn by its author. Revision 2 is
///   `compatible` / `settled`, cites the withdrawal receipt, and names
///   revision 1 as its predecessor. Nothing was edited in place.
///
/// The two non-cases are the documented false-positive shapes that
/// motivated the state families; real corpus cases replace them in a
/// later release. Every id, name and content string here is synthetic.
fn contradiction_cases() -> Vec<u8> {
    let company = |id: &str, name: &str| Vertex {
        id: VertexId(id.into()),
        name: name.into(),
        label: TypeName("Antares.Company".into()),
        properties: BTreeMap::new(),
    };
    let evidence = |id: &str, source_type: &str, content: &str| Evidence {
        id: EvidenceId(id.into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_uri: format!("antares://{source_type}/{id}"),
        source_type: source_type.into(),
        source_id: id.into(),
        content: content.into(),
        source_blob: None,
        derivation: None,
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
    let observation = |id: &str, subject: &str, value: i64, ev: &str| Observation {
        id: ObservationId(id.into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_event_id: None,
        source_uri: Some(format!("antares://filing/{ev}")),
        subject_id: Some(VertexId(subject.into())),
        predicate: "headcount".into(),
        object_id: None,
        object_value: Some(serde_json::json!(value)),
        observed_at: ant_types::EventTime::known("2026-08-09T10:00:00Z".parse().unwrap()),
        extracted_at: ant_types::EventTime::known("2026-08-09T10:00:01Z".parse().unwrap()),
        confidence: Some(0.9),
        evidence_ids: vec![EvidenceId(ev.into())],
        extractor_version: Some("conformance/1".into()),
        metadata: serde_json::Value::Null,
        author: None,
    };
    let belief = Belief {
        id: BeliefId("bel_hc_1".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        subject_id: VertexId("co_1".into()),
        predicate: "headcount".into(),
        value_json: serde_json::json!(1200),
        confidence: Some(0.9),
        belief_version: 1,
        derived_from: vec![ObservationId("obs_hc_1".into())],
        evidence_ids: vec![EvidenceId("ev_hc_1".into())],
        valid_from: None,
        valid_to: None,
        observed_at: Some("2026-08-09T10:00:00Z".parse().unwrap()),
        updated_at: "2026-08-09T10:00:02Z".parse().unwrap(),
        decay_policy: None,
        metadata: serde_json::Value::Null,
        author: None,
        contributing_authors: Vec::new(),
    };
    let claim = |kind: ClaimKind, id: &str, version: Option<u64>| ClaimRef {
        kind,
        id: id.into(),
        version,
        pointer: None,
    };
    let comparator = |rule: Option<&str>, model: Option<&str>| ComparatorIdentity {
        comparator: "numeric_tolerance".into(),
        comparator_version: "1.2".into(),
        rule_id: rule.map(str::to_string),
        rule_version: rule.map(|_| "3".to_string()),
        model: model.map(str::to_string),
        model_version: model.map(|_| "2026.08".to_string()),
        snapshot_id: Some("snap_2026_08_10".into()),
    };
    let case = |id: &str,
                case_id: &str,
                prev: Option<&str>,
                claims: Vec<ClaimRef>,
                comparator: ComparatorIdentity,
                epistemic: EpistemicState,
                impact: BusinessImpact,
                workflow: WorkflowState,
                receipts: Vec<&str>,
                revised_at: &str| ContradictionCase {
        id: CaseRevisionId(id.into()),
        case_id: ContradictionCaseId(case_id.into()),
        previous_revision_id: prev.map(|p| CaseRevisionId(p.into())),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        family: "same_subject_numeric".into(),
        claims,
        evidence: vec![],
        measurements: vec![],
        comparator,
        supporting: vec![],
        refuting: vec![],
        vault_occurrences: vec![],
        epistemic,
        impact,
        workflow,
        proposal_id: None,
        review_receipts: receipts.into_iter().map(|r| EvidenceId(r.into())).collect(),
        revised_at: revised_at.parse().unwrap(),
        author: None,
        metadata: serde_json::Value::Null,
    };

    // The incompatible pair, fully dressed.
    let mut hc = case(
        "case_hc_1@1",
        "case_hc_1",
        None,
        vec![
            claim(ClaimKind::Belief, "bel_hc_1", Some(1)),
            claim(ClaimKind::Observation, "obs_hc_2", None),
        ],
        comparator(Some("headcount_agreement"), None),
        EpistemicState::Incompatible,
        BusinessImpact::Harmful,
        WorkflowState::AwaitingReview,
        vec!["ev_review_1"],
        "2026-08-10T09:00:00Z",
    );
    hc.measurements = vec![
        MeasurementRef {
            name: "amount_a".into(),
            value: serde_json::json!(1200),
            unit: Some("employees".into()),
            evidence_id: Some(EvidenceId("ev_hc_1".into())),
            pointer: Some(SourcePointer {
                char_start: Some(10),
                char_end: Some(14),
                ..Default::default()
            }),
        },
        MeasurementRef {
            name: "amount_b".into(),
            value: serde_json::json!(900),
            unit: Some("employees".into()),
            evidence_id: Some(EvidenceId("ev_hc_2".into())),
            pointer: Some(SourcePointer {
                char_start: Some(6),
                char_end: Some(9),
                ..Default::default()
            }),
        },
        MeasurementRef {
            name: "tolerance".into(),
            value: serde_json::json!(0.05),
            unit: None,
            evidence_id: None,
            pointer: None,
        },
    ];
    // The source, and a forwarded copy of it: one witness, not two.
    hc.supporting = vec![
        Material {
            evidence_id: EvidenceId("ev_hc_2".into()),
            pointer: None,
            dependency: SourceDependency::Independent,
        },
        Material {
            evidence_id: EvidenceId("ev_hc_3".into()),
            pointer: None,
            dependency: SourceDependency::ForwardedCopy {
                of: EvidenceId("ev_hc_2".into()),
            },
        },
    ];
    hc.vault_occurrences = vec![VaultOccurrence {
        vault_id: "vault_main".into(),
        item_id: "hooli/headcount".into(),
        revision: Some("r7".into()),
        pointer: None,
        conditions: vec!["fiscal_year=2026".into()],
    }];
    hc.proposal_id = Some("prop_hc_1".into());

    // Non-case one: the model invented a shared subject.
    let mut ms = case(
        "case_ms_1@1",
        "case_ms_1",
        None,
        vec![
            claim(ClaimKind::Observation, "obs_hc_1", None),
            claim(ClaimKind::Observation, "obs_pp_1", None),
        ],
        comparator(None, Some("matcher-lm")),
        EpistemicState::InsufficientlyComparable,
        BusinessImpact::Unassessed,
        WorkflowState::Settled,
        vec!["ev_review_2"],
        "2026-08-10T09:05:00Z",
    );
    ms.refuting = vec![Material {
        evidence_id: EvidenceId("ev_review_2".into()),
        pointer: None,
        dependency: SourceDependency::Independent,
    }];
    ms.metadata = serde_json::json!({"nonCase": "model-invented shared subject"});

    // Non-case two: opened, then the lower figure was withdrawn.
    let wd1 = case(
        "case_wd_1@1",
        "case_wd_1",
        None,
        vec![
            claim(ClaimKind::Observation, "obs_hc_2", None),
            claim(ClaimKind::Observation, "obs_hc_4", None),
        ],
        comparator(Some("headcount_agreement"), None),
        EpistemicState::Incompatible,
        BusinessImpact::AlignmentOnly,
        WorkflowState::Open,
        vec![],
        "2026-08-10T09:10:00Z",
    );
    let mut wd2 = case(
        "case_wd_1@2",
        "case_wd_1",
        Some("case_wd_1@1"),
        vec![
            claim(ClaimKind::Observation, "obs_hc_2", None),
            claim(ClaimKind::Observation, "obs_hc_4", None),
        ],
        comparator(Some("headcount_agreement"), None),
        EpistemicState::Compatible,
        BusinessImpact::AlignmentOnly,
        WorkflowState::Settled,
        vec!["ev_withdrawal"],
        "2026-08-12T15:00:00Z",
    );
    wd2.metadata = serde_json::json!({"nonCase": "withdrawal receipt"});

    let mut w = AntWriter::new(Vec::new(), manifest(), 0).expect("writer");
    w.write(AntRecord::Vertex {
        data: company("co_1", "Hooli Corp"),
    })
    .unwrap();
    w.write(AntRecord::Vertex {
        data: company("co_2", "Pied Piper Inc"),
    })
    .unwrap();
    for (id, source_type, content) in [
        (
            "ev_hc_1",
            "filing",
            "headcount 1200 per the 2026 annual filing",
        ),
        (
            "ev_hc_2",
            "press",
            "about 900 employees, per the March release",
        ),
        (
            "ev_hc_3",
            "mail",
            "fwd: about 900 employees, per the March release",
        ),
        (
            "ev_hc_4",
            "filing",
            "corrected headcount 1150 in the restated filing",
        ),
        ("ev_pp_1", "filing", "headcount 40 per the seed-round deck"),
        (
            "ev_withdrawal",
            "receipt",
            "the March figure was withdrawn by its author on 2026-08-12",
        ),
        (
            "ev_review_1",
            "receipt",
            "reviewed by the data desk: incompatible, impact unassessed pending finance",
        ),
        (
            "ev_review_2",
            "receipt",
            "reviewed: Hooli Corp and Pied Piper Inc are different companies; not comparable",
        ),
    ] {
        w.write(AntRecord::Evidence {
            data: evidence(id, source_type, content),
        })
        .unwrap();
    }
    for (id, subject, value, ev) in [
        ("obs_hc_1", "co_1", 1200, "ev_hc_1"),
        ("obs_hc_2", "co_1", 900, "ev_hc_2"),
        ("obs_hc_4", "co_1", 1150, "ev_hc_4"),
        ("obs_pp_1", "co_2", 40, "ev_pp_1"),
    ] {
        w.write(AntRecord::Observation {
            data: observation(id, subject, value, ev),
        })
        .unwrap();
    }
    w.write(AntRecord::Belief { data: belief }).unwrap();
    // A previous revision precedes its successor.
    for c in [hc, ms, wd1, wd2] {
        w.write(AntRecord::ContradictionCase { data: Box::new(c) })
            .unwrap();
    }
    w.finish().expect("finish")
}

/// v0.5 golden: relationship proposals and their whole closure. Three
/// revisions of two proposals over one synthetic shop schema, each
/// citing records that are in the same file:
///
/// * `prop_ord_cust@1` — SUPPORTED by its own measurement: 890 of 900
///   orders carrying a customer reference match a customer row, against
///   a declared minimum of 0.95, with the target key unique.
/// * `prop_color_variant@1` — a QUARANTINED HYPOTHESIS in the shape the
///   loop keeps finding: a self-join a model suggested, measured at
///   0 of 1,914, held back with the reason and the probe that measured
///   it. This is the record a grader reads instead of nothing.
/// * `prop_ord_cust@2` — the first proposal PROMOTED BY A REVIEWER, a
///   new revision naming its predecessor, carrying who decided, when,
///   why, and the receipt record — which is in the file, because a
///   promotion whose receipt is missing is not a promotion.
///
/// Every id, name, column and statement here is synthetic.
fn relationship_proposals() -> Vec<u8> {
    let evidence = |id: &str, source_type: &str, content: &str| Evidence {
        id: EvidenceId(id.into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_uri: format!("antares://{source_type}/{id}"),
        source_type: source_type.into(),
        source_id: id.into(),
        content: content.into(),
        source_blob: None,
        derivation: None,
        char_start: None,
        char_end: None,
        byte_start: None,
        byte_end: None,
        observed_at: Some("2026-09-12T08:00:00Z".parse().unwrap()),
        extracted_at: Some("2026-09-12T08:00:01Z".parse().unwrap()),
        extractor_version: Some("conformance/1".into()),
        confidence: None,
        metadata: serde_json::Value::Null,
        author: None,
    };
    let manifest_ref = || SourceManifestRef {
        connection: "shop".into(),
        plan_hash: "9f2b7c1d4e6a8035".into(),
        catalog_hash: Some("3c5e9017ab42d6f8".into()),
        policy_version: Some(3),
        policy_hash: Some("1467488ba7a68011".into()),
    };
    let origin = |model: Option<&str>| ProposalOrigin {
        run_id: "run_2026_09_12_01".into(),
        recon_version: "2.1".into(),
        source_manifest: manifest_ref(),
        model: model.map(str::to_string),
        model_version: model.map(|_| "2026.08".to_string()),
    };
    let support = |source_rows: u64,
                   source_non_null: u64,
                   matched: u64,
                   target_rows: u64,
                   target_distinct: u64,
                   fingerprint: &str| RelationSupport {
        contract_version: 1,
        method: SupportMethod::JoinMatchScan,
        method_version: 1,
        source_rows,
        source_non_null,
        matched_rows: matched,
        target_rows,
        target_non_null: target_distinct,
        target_distinct,
        sampling: Sampling::full_scan(),
        fingerprint: fingerprint.into(),
        min_support: 0.95,
    };

    // The supported one: orders reference customers by a trimmed,
    // lower-cased code.
    let ord_cust_relation = || ProposedRelation {
        subject_type: "Shop.Order".into(),
        predicate: "placedBy".into(),
        target_type: "Shop.Customer".into(),
        source_relation: "shop.orders".into(),
        source_key_columns: vec!["customer_ref".into()],
        target_relation: "shop.customers".into(),
        target_key_columns: vec!["code".into()],
        normalization: Some(Normalization {
            source: NormalizationOp::TrimLower,
            target: Some(NormalizationOp::TrimLower),
        }),
    };
    let ord_cust_probe = || ProbeRef {
        name: "matched_rows".into(),
        statement: "SELECT count(*) FROM shop.orders o JOIN shop.customers c \
                    ON lower(btrim(o.customer_ref)) = lower(btrim(c.code))"
            .into(),
        dialect: "postgres".into(),
        ran_at: Some("2026-09-12T08:00:02Z".parse().unwrap()),
        evidence_id: Some(EvidenceId("ev_probe_ord_cust".into())),
    };

    let supported = RelationshipProposal {
        id: ProposalRevisionId("prop_ord_cust@1".into()),
        proposal_id: RelationshipProposalId("prop_ord_cust".into()),
        previous_revision_id: None,
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        origin: origin(None),
        relation: ord_cust_relation(),
        support: support(1000, 900, 890, 50, 50, "sha256:1a2b3c4d5e6f7081"),
        status: ProposalStatus::Supported,
        findings: vec![EvidenceId("ev_find_ord_cust".into())],
        probes: vec![ord_cust_probe()],
        proposed_at: "2026-09-12T08:00:03Z".parse().unwrap(),
        author: None,
        metadata: serde_json::Value::Null,
    };

    // The quarantined hypothesis: 0 of 1,914. Well-formed, measured,
    // and held back — a finding, not an absence.
    let quarantined = RelationshipProposal {
        id: ProposalRevisionId("prop_color_variant@1".into()),
        proposal_id: RelationshipProposalId("prop_color_variant".into()),
        previous_revision_id: None,
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        origin: origin(Some("recon-suggester")),
        relation: ProposedRelation {
            subject_type: "Shop.Product".into(),
            predicate: "colorVariantOf".into(),
            target_type: "Shop.Product".into(),
            source_relation: "shop.products".into(),
            source_key_columns: vec!["color_variant_of".into()],
            target_relation: "shop.products".into(),
            target_key_columns: vec!["sku".into()],
            normalization: Some(Normalization {
                source: NormalizationOp::TrimLower,
                target: Some(NormalizationOp::TrimLower),
            }),
        },
        support: support(1914, 1914, 0, 1914, 1914, "sha256:90ab12cd34ef5678"),
        status: ProposalStatus::QuarantinedHypothesis {
            reason: "0 of 1914 source rows match a target row; the column holds a free-text \
                     colour name, not a product key"
                .into(),
        },
        findings: vec![EvidenceId("ev_find_color_variant".into())],
        probes: vec![ProbeRef {
            name: "matched_rows".into(),
            statement: "SELECT count(*) FROM shop.products p JOIN shop.products t \
                        ON lower(btrim(p.color_variant_of)) = lower(btrim(t.sku))"
                .into(),
            dialect: "postgres".into(),
            ran_at: Some("2026-09-12T08:00:04Z".parse().unwrap()),
            evidence_id: Some(EvidenceId("ev_probe_color_variant".into())),
        }],
        proposed_at: "2026-09-12T08:00:05Z".parse().unwrap(),
        author: None,
        metadata: serde_json::Value::Null,
    };

    // A reviewer promotes the first one, on the record. A new revision
    // naming its predecessor; nothing is edited in place.
    let promoted = RelationshipProposal {
        id: ProposalRevisionId("prop_ord_cust@2".into()),
        proposal_id: RelationshipProposalId("prop_ord_cust".into()),
        previous_revision_id: Some(ProposalRevisionId("prop_ord_cust@1".into())),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        origin: origin(None),
        relation: ord_cust_relation(),
        support: support(1000, 900, 890, 50, 50, "sha256:1a2b3c4d5e6f7081"),
        status: ProposalStatus::PromotedByReviewer {
            receipt: ReviewerReceipt {
                reviewer: "user:reviewer_1".into(),
                decided_at: "2026-09-12T09:30:00Z".parse().unwrap(),
                reason: "the customer code is the documented external key; the ten unmatched \
                         rows are orders placed before the code was issued"
                    .into(),
                receipt: EvidenceId("ev_review_ord_cust".into()),
            },
        },
        findings: vec![EvidenceId("ev_find_ord_cust".into())],
        probes: vec![ord_cust_probe()],
        proposed_at: "2026-09-12T09:30:01Z".parse().unwrap(),
        author: None,
        metadata: serde_json::Value::Null,
    };

    let mut w = AntWriter::new(Vec::new(), manifest(), 0).expect("writer");
    // Every referenced record before the proposal that cites it.
    for (id, source_type, content) in [
        (
            "ev_find_ord_cust",
            "catalog",
            "shop.orders.customer_ref is a text column whose values look like shop.customers.code",
        ),
        (
            "ev_probe_ord_cust",
            "probe",
            "matched_rows=890 source_non_null=900 target_distinct=50",
        ),
        (
            "ev_find_color_variant",
            "catalog",
            "shop.products.color_variant_of is a text column suggested as a self-reference",
        ),
        (
            "ev_probe_color_variant",
            "probe",
            "matched_rows=0 source_non_null=1914 target_distinct=1914",
        ),
        (
            "ev_review_ord_cust",
            "review",
            "reviewed the measurement and the column documentation; promoted for publication",
        ),
    ] {
        w.write(AntRecord::Evidence {
            data: evidence(id, source_type, content),
        })
        .unwrap();
    }
    for p in [supported, quarantined, promoted] {
        w.write(AntRecord::RelationshipProposal { data: Box::new(p) })
            .unwrap();
    }
    w.finish().expect("finish")
}

/// v0.6 golden: explicitly-unknown observation time.
///
/// Two observations pin the whole additive contract in one file:
///
///   * `obs_undated` — a genuinely dateless original. Its EVENT time is
///     `Unknown{no_source_time}` (never a fabricated instant), while its
///     PROVENANCE time is `Known` WITH a basis (`source_record_time`)
///     drawn from the extraction receipt. This is the record that could
///     not exist before v0.6.
///   * `obs_dated` — an ordinary dated observation whose `observed_at`
///     and `extracted_at` are bare RFC3339 strings, byte-identical to
///     v0.5. Pinning it here is the proof that the bump is additive: the
///     common case did not move.
fn unknown_time() -> Vec<u8> {
    let undated = Observation {
        id: ObservationId("obs_undated".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_event_id: Some("filing-7".into()),
        source_uri: Some("antares://filing/7".into()),
        subject_id: Some(VertexId("deal_1".into())),
        predicate: "headcount".into(),
        object_id: None,
        object_value: Some(serde_json::json!(1200)),
        // Event time is genuinely unknown; nothing fabricates an instant.
        observed_at: ant_types::EventTime::unknown(ant_types::UnknownTime::NoSourceTime),
        // Provenance time is known and carries its basis from the receipt.
        extracted_at: ant_types::EventTime::known_with(
            "2026-08-09T10:00:01Z".parse().unwrap(),
            ant_types::TimeBasis::SourceRecordTime,
        ),
        confidence: Some(0.9),
        evidence_ids: vec![],
        extractor_version: Some("conformance/1".into()),
        metadata: serde_json::Value::Null,
        author: None,
    };
    let dated = Observation {
        id: ObservationId("obs_dated".into()),
        tenant_id: TenantId(1),
        project_id: ProjectId(1),
        source_event_id: Some("mail-9".into()),
        source_uri: Some("antares://mail/9".into()),
        subject_id: Some(VertexId("deal_1".into())),
        predicate: "stage_change".into(),
        object_id: None,
        object_value: Some(serde_json::json!("proposal")),
        observed_at: ant_types::EventTime::known("2026-08-09T10:00:00Z".parse().unwrap()),
        extracted_at: ant_types::EventTime::known("2026-08-09T10:00:01Z".parse().unwrap()),
        confidence: Some(0.9),
        evidence_ids: vec![],
        extractor_version: Some("conformance/1".into()),
        metadata: serde_json::Value::Null,
        author: None,
    };

    let mut w = AntWriter::new(Vec::new(), manifest(), 0).expect("writer");
    w.write(AntRecord::Observation { data: undated }).unwrap();
    w.write(AntRecord::Observation { data: dated }).unwrap();
    w.finish().expect("finish")
}

fn ontology_sha256(value: &impl serde::Serialize) -> String {
    let mut value = serde_json::to_value(value).expect("ontology fixture serializes");
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"antares-canonical-json-v1\0");
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// v0.7 golden: one complete, self-contained immutable ontology election.
/// Its evidence precedes the revision so the archive demonstrates native
/// closure instead of depending on placeholder ancestors or a destination
/// store that a third-party conformance runner does not have.
fn ontology_revisions() -> (Vec<u8>, OntologyRevision) {
    let mut evidence = Evidence::quick(
        "ontology-support-1",
        TenantId(7),
        ProjectId(7),
        "review",
        "ontology-genesis",
        "reviewed declaration: Test.Deal.amount is a Long",
    );
    evidence.author = Some(AuthorStamp {
        user_id: UserId("sme-a".into()),
        token_id: Some(TokenId("token-sme-a-fixture".into())),
        subject_type: SubjectType::User,
        authored_at: "2026-09-17T15:20:00Z".parse().unwrap(),
    });
    let evidence_ref = OntologyRecordRef {
        kind: OntologyRecordKind::Evidence,
        id: evidence.id.0.clone(),
        content_sha256: ontology_sha256(&evidence),
    };
    let schema = SchemaType {
        kind: SpgTypeKind::EntityType,
        name: TypeName("Test.Deal".into()),
        name_zh: None,
        properties: vec![PropertyDef {
            name: "amount".into(),
            name_zh: None,
            value_type: ValueType::Long,
            index: None,
        }],
        relations: Vec::new(),
    };
    let semantic_item = OntologySemanticItem::SchemaType {
        key: "Test.Deal".into(),
        revision: "review:ontology-genesis:1".into(),
        content_sha256: ontology_sha256(&schema),
        content: schema,
        support: vec![evidence_ref.clone()],
    };
    let attestation = OntologyApprovalAttestation {
        attestation_version: 1,
        attester_principal: "machine:main-server".into(),
        proposal_id: "proposal:ontology-genesis".into(),
        election_subject_sha256: "0".repeat(64),
        source_vault: "root".into(),
        source_vault_revision: 0,
        target_vault: "team:operations".into(),
        target_vault_revision: 0,
        authority_policy_revision: "decision-authority-2026-09-17".into(),
        authority_policy_sha256: "c365c65db86a8aaa165d982353a10a1e0ddb88b2f995b9fdc489cf19e00f5d38"
            .into(),
        designation: serde_json::json!({"kind": "reviewer"}),
        matched_by: serde_json::json!({
            "kind": "group",
            "group": "/directory/entra/ops-managers"
        }),
        source_owner_consent: OntologySourceOwnerConsent {
            consent_version: 1,
            consent_id: "source-owner-consent:ontology-genesis".into(),
            owner_principal: "user:ontology-source-owner".into(),
            election_subject_sha256: "0".repeat(64),
            source: OntologyVaultPin {
                vault_id: "root".into(),
                vault_revision: 0,
                ontology_revision: None,
            },
            target: OntologyVaultPin {
                vault_id: "team:operations".into(),
                vault_revision: 0,
                ontology_revision: None,
            },
            published_records: vec![evidence_ref.clone()],
            published_revision_refs: Vec::new(),
            consent: serde_json::json!({
                "consentId": "source-owner-consent:ontology-genesis",
                "sourceOwner": "user:ontology-source-owner",
                "electionSubjectSha256": "0".repeat(64),
                "source": {"vaultId": "root", "vaultRevision": 0},
                "target": {"vaultId": "team:operations", "vaultRevision": 0},
                "publishedRecords": [evidence_ref.clone()],
                "publishedRevisionRefs": [],
                "at": "2026-09-17T15:20:00Z",
                "valid": true
            }),
        },
        approval: serde_json::json!({
            "reviewId": "review:ontology-genesis",
            "revision": 1,
            "materialFingerprint": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "reviewer": "operations-owner",
            "grantId": "ontology-publication-conformance",
            "authorized": {},
            "binding": {"executor": "machine:main-server"},
            "identity": {},
            "policyRevision": {
                "version": "decision-authority-2026-09-17",
                "sha256": "c365c65db86a8aaa165d982353a10a1e0ddb88b2f995b9fdc489cf19e00f5d38"
            },
            "designation": {"kind": "reviewer"},
            "at": "2026-09-17T15:25:00Z",
            "valid": true
        }),
    };
    let mut reviewed_manifest = OntologyRevisionManifest {
        contract_version: 1,
        election_subject_sha256: "0".repeat(64),
        source: OntologyVaultPin {
            vault_id: "root".into(),
            vault_revision: 0,
            ontology_revision: None,
        },
        target: OntologyVaultPin {
            vault_id: "team:operations".into(),
            vault_revision: 0,
            ontology_revision: None,
        },
        common_base: None,
        dependencies: Vec::new(),
        semantic_items: vec![semantic_item],
        published_records: vec![evidence_ref.clone()],
        published_revision_refs: Vec::new(),
        accepted_claims: Vec::new(),
        retained_positions: vec![OntologyRetainedPosition {
            disposition: OntologyPositionDisposition::Accepted,
            records: vec![evidence_ref.clone()],
        }],
        attribution: vec![OntologyAttribution {
            principal: "user:sme-a".into(),
            evidence: evidence_ref,
        }],
        reverses: None,
        approval: OntologyApprovalBinding {
            content_sha256: "0".repeat(64),
            attestation,
        },
    };
    let election_subject_sha256 = ontology_sha256(&reviewed_manifest.election_subject());
    reviewed_manifest.election_subject_sha256 = election_subject_sha256.clone();
    reviewed_manifest
        .approval
        .attestation
        .election_subject_sha256 = election_subject_sha256;
    reviewed_manifest
        .approval
        .attestation
        .source_owner_consent
        .election_subject_sha256 = reviewed_manifest.election_subject_sha256.clone();
    reviewed_manifest
        .approval
        .attestation
        .source_owner_consent
        .consent["electionSubjectSha256"] =
        serde_json::Value::String(reviewed_manifest.election_subject_sha256.clone());
    reviewed_manifest.approval.content_sha256 =
        ontology_sha256(&reviewed_manifest.approval.attestation);
    let manifest_sha256 = ontology_sha256(&reviewed_manifest);
    let revision_id = OntologyRevisionId(format!("orv1:{manifest_sha256}"));
    let request_id = "ontology-publication:conformance:genesis".to_string();
    let request_sha256 = ontology_sha256(&serde_json::json!({
        "requestId": request_id,
        "projectId": 7,
        "canonicalEncoding": "antares-canonical-json-v1",
        "manifestSha256": manifest_sha256,
        "manifest": &reviewed_manifest
    }));
    let revision = OntologyRevision {
        id: revision_id.clone(),
        tenant_id: TenantId(7),
        project_id: ProjectId(7),
        manifest_sha256,
        manifest: reviewed_manifest,
        publisher: OntologyPublisherStamp {
            principal: "machine:main-server".into(),
            token_id: "token-main-server-fixture".into(),
            subject_type: "service".into(),
        },
        request_id,
        request_sha256,
        committed_at: "2026-09-17T15:30:00Z".parse().unwrap(),
        conditional: OntologyConditionalPosition {
            revision_domain: ONTOLOGY_REVISION_DOMAIN.into(),
            chain_id: ONTOLOGY_CHAIN_ID.into(),
            revision_id,
            previous_revision_id: None,
            initialized_from_existing: false,
        },
    };
    revision
        .validate_shape()
        .expect("ontology conformance revision is structurally valid");

    let mut file_manifest = manifest();
    file_manifest.tenant_id = 7;
    file_manifest.project_id = 7;
    file_manifest.selection = Some(serde_json::json!({"kind": "ontology_closure"}));
    let mut writer = AntWriter::new(Vec::new(), file_manifest, 0).expect("writer");
    writer
        .write(AntRecord::Evidence { data: evidence })
        .unwrap();
    writer
        .write(AntRecord::OntologyRevision {
            data: Box::new(revision.clone()),
        })
        .unwrap();
    (writer.finish().expect("finish"), revision)
}

/// The stored original of the v1.0 golden: 150 deterministic bytes in
/// 64-byte chunks (64 + 64 + 22). Real writers use larger chunks (the
/// engine writes 1 MiB); the rules are the same at any size.
const ORIGINAL_CHUNK: usize = 64;

/// Opaque coverage facts every reader must return exactly and count as
/// Rust serializes them: integers past 2^53 and at u64::MAX, and doubles in
/// every layout serde_json writes (plain, fraction, leading zeros, exponent
/// with its sign, extremes).
fn coverage_exact() -> serde_json::Value {
    serde_json::json!({
        "n": 9_007_199_254_740_993u64,
        "m": u64::MAX,
        "i": i64::MIN,
        "z": -0.0f64,
        "f": 1.0f64,
        "spread": [0.1f64, 1e-7, 1e21, 123_456.789, 5e-324, 1.797_693_134_862_315_7e308,
                   -2.5e-5, 1e16, 1e15, 0.001_234, 1.234e33, 100.0],
        // Parsed lossily by serde_json without `float_roundtrip` (it reads
        // 51.24817837550541); JS and Python keep it. Revision 26 differential.
        "differential": 51.248_178_375_505_404f64

    })
}

/// A valid archive with exactly one data line edited as raw text (the first
/// line containing `from`), its trailer hash recomputed: for fixtures no
/// typed writer can express (a `-0` literal, a number out of range).
fn edit_one_line(archive: &[u8], from: &str, to: &str) -> Vec<u8> {
    let raw = zstd::stream::decode_all(archive).unwrap();
    let text = String::from_utf8(raw).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let at = lines
        .iter()
        .position(|l| l.contains(from))
        .unwrap_or_else(|| panic!("no line contains {from}"));
    lines[at] = lines[at].replacen(from, to, 1);
    let (trailer, body) = lines.split_last_mut().unwrap();
    let mut hasher = Sha256::new();
    for line in body.iter() {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let mut t: serde_json::Value = serde_json::from_str(trailer).unwrap();
    t["sha256"] = serde_json::json!(format!("{:x}", hasher.finalize()));
    *trailer = serde_json::to_string(&t).unwrap();
    let out = lines.join("\n") + "\n";
    zstd::stream::encode_all(out.as_bytes(), 0).unwrap()
}

/// A JSON object nesting exactly `levels` objects, itself the first.
fn nested(levels: usize) -> serde_json::Value {
    (1..levels).fold(
        serde_json::json!({"d": 0}),
        |inner, _| serde_json::json!({"d": inner}),
    )
}

fn original_bytes() -> Vec<u8> {
    (0..150u32).map(|i| ((i * 7 + 3) % 256) as u8).collect()
}

/// A cleaned-text derivative of `ev_original` (format v1.0): blob-free
/// Evidence whose typed derivation binds that exact original.
fn derivative(id: &str, index: u64, text: &str, raw: &[u8]) -> Evidence {
    derivative_in("normalization-golden-0001", id, index, text, raw)
}

/// [`derivative`], in a named job.
fn derivative_in(job: &str, id: &str, index: u64, text: &str, raw: &[u8]) -> Evidence {
    let mut e = Evidence::quick(
        id,
        TenantId(1),
        ProjectId(1),
        "text/plain",
        "ev_original",
        text,
    );
    e.derivation = Some(Box::new(ant_types::Derivation {
        contract: ant_types::NORMALIZED_TEXT_CONTRACT.into(),
        primary_evidence_id: "ev_original".into(),
        asset_id: "asset_original_0001".into(),
        sha256: format!("{:x}", Sha256::digest(raw)),
        byte_length: raw.len() as u64,
        normalizer: ant_types::Normalizer {
            name: "html5ever-visible".into(),
            version: "product-3-html-original-v1".into(),
            configuration_sha256: format!("{:x}", Sha256::digest(b"golden configuration")),
        },
        job_id: job.into(),
        segment: ant_types::DerivationSegment {
            index,
            locator: format!("html:line:{}:block:{index}", index.wrapping_add(1)),
            coverage: serde_json::json!({"units": [index]}),
        },
        text_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        text_byte_length: text.len() as u64,
    }));
    e
}

fn original_evidence(id: &str, asset: &str, raw: &[u8]) -> Evidence {
    let mut e = Evidence::quick(id, TenantId(1), ProjectId(1), "manual_file", id, "");
    e.source_blob = Some(SourceBlob {
        asset_id: asset.into(),
        byte_length: raw.len() as u64,
        sha256: format!("{:x}", Sha256::digest(raw)),
        media_type: "application/pdf".into(),
        file_name: format!("{id}.pdf"),
    });
    e
}

fn chunks_of(evidence: &str, asset: &str, raw: &[u8]) -> Vec<OriginalChunk> {
    raw.chunks(ORIGINAL_CHUNK)
        .enumerate()
        .map(|(i, c)| OriginalChunk::new(evidence, asset, i as u64, (i * ORIGINAL_CHUNK) as u64, c))
        .collect()
}

/// How a negative golden departs from the valid originals stream.
#[derive(Clone, Copy)]
enum Break {
    None,
    MissingChunk,
    Reordered,
    ChunkDigest,
    WholeDigest,
    Interrupted,
    InV0,
    UnboundSource,
    DerivativeOrphan,
    DerivativeUnbound,
    DerivativeText,
    DerivativeOrder,
    /// Positive: slots past 2^53 and i64::MAX.
    WideSlots,
    /// One negative per structural derivation rule, each breaking only it.
    DerivContract,
    DerivPrimaryControl,
    DerivAssetId,
    DerivSha,
    DerivNormalizer,
    DerivConfiguration,
    DerivJobId,
    DerivLocator,
    DerivCoverageType,
    DerivCoverageSize,
    DerivTextSha,
    DerivWithOriginal,
    /// Coverage accounting is Rust's serialization: a positive golden
    /// whose coverage holds exact wide integers and a spread of doubles; a
    /// positive at 1500 x 1e-6 (7.5 KiB in Rust, larger in naive JS/Python
    /// spellings); a negative at 2500 x 1.0 (10 KiB in Rust, 5 KiB naive).
    CoverageExact,
    CoverageFloatsUnder,
    CoverageFloatsOver,
    /// Positive: the first source reference's opaque `source` holds exact
    /// wide integers and doubles, returned exactly and counted as Rust does.
    SourceExact,
    /// One negative per SourceBlob / SourceReference structural rule.
    BlobAssetId,
    BlobSha,
    BlobMediaType,
    BlobFileName,
    RefId,
    RefSourceType,
    RefSourceSize,
    RefEvidenceControl,
    RefAssetId,
    /// Opaque nesting: coverage and source at exactly the 64-level limit
    /// (positive), and one level past it (negative).
    CoverageDepthLimit,
    DerivCoverageDepth,
    SourceDepthLimit,
    RefSourceDepth,
}

/// The v1.0 golden, or one of its negatives. Every negative has a valid
/// trailer: the ONLY reason to reject it is the original rule it breaks.
fn originals(b: Break) -> Vec<u8> {
    let mut m = manifest();
    m.version = match b {
        Break::InV0 => FORMAT_VERSION.into(),
        _ => ORIGINALS_FORMAT_VERSION.into(),
    };
    let raw = original_bytes();
    let (ev, asset) = ("ev_original", "asset_original_0001");
    let mut chunks = chunks_of(ev, asset, &raw);
    match b {
        Break::MissingChunk => {
            chunks.remove(1);
        }
        Break::Reordered => chunks.swap(1, 2),
        Break::ChunkDigest => chunks[1].sha256 = format!("{:x}", Sha256::digest(b"not it")),
        Break::WholeDigest => {
            // A consistent chunk (its own digest matches) that is not the
            // original the evidence declares.
            let mut bad = raw[ORIGINAL_CHUNK..2 * ORIGINAL_CHUNK].to_vec();
            bad[0] ^= 0xff;
            chunks[1] = OriginalChunk::new(ev, asset, 1, ORIGINAL_CHUNK as u64, &bad);
        }
        _ => {}
    }
    let mut w = AntWriter::new(Vec::new(), m, 0).unwrap();
    let mut primary = original_evidence(ev, asset, &raw);
    {
        let blob = primary.source_blob.as_mut().unwrap();
        match b {
            Break::BlobAssetId => blob.asset_id = "short".into(),
            Break::BlobSha => blob.sha256 = blob.sha256.to_uppercase(),
            Break::BlobMediaType => blob.media_type = "application/pdf; name=\u{e9}".into(),
            Break::BlobFileName => blob.file_name = "report\n.pdf".into(),
            _ => {}
        }
    }
    w.write(AntRecord::Evidence { data: primary }).unwrap();
    let plain = Evidence::quick(
        "ev_plain",
        TenantId(1),
        ProjectId(1),
        "note",
        "n1",
        "cleaned text lives in ordinary evidence",
    );
    for (i, c) in chunks.into_iter().enumerate() {
        if matches!(b, Break::Interrupted) && i == 1 {
            w.write(AntRecord::Evidence {
                data: plain.clone(),
            })
            .unwrap();
        }
        w.write(AntRecord::OriginalChunk { data: c }).unwrap();
    }
    // Where the bytes came from: two source references, in referenceId
    // order, bound to exactly this original.
    let sha = format!("{:x}", Sha256::digest(&raw));
    for (i, reference_id, source) in [
        (
            "ref-drive-0001",
            serde_json::json!({"driveId": "drive-1", "itemId": "item-9"}),
        ),
        ("ref-picker-0001", serde_json::json!({"origin": "picker"})),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, r)| (i, r.0, r.1))
    {
        let bound_sha = if matches!(b, Break::UnboundSource) {
            format!("{:x}", Sha256::digest(b"other bytes"))
        } else {
            sha.clone()
        };
        let mut reference = SourceReference {
            evidence_id: ev.into(),
            asset_id: asset.into(),
            sha256: bound_sha,
            byte_length: raw.len() as u64,
            reference_id: reference_id.into(),
            source,
            recorded_at: chrono::DateTime::from_timestamp(1_790_000_000, 0).unwrap(),
            author: None,
        };
        if i == 0 {
            match b {
                Break::SourceExact => reference.source = coverage_exact(),
                Break::RefId => reference.reference_id = "ref drive 0001".into(),
                Break::RefSourceType => reference.source = serde_json::json!([0]),
                Break::RefSourceSize => {
                    reference.source = serde_json::json!({"x": "y".repeat(17_000)})
                }
                Break::RefEvidenceControl => reference.evidence_id = format!("{ev}\u{1}"),
                Break::RefAssetId => reference.asset_id = "short".into(),
                Break::SourceDepthLimit => {
                    reference.source = nested(SourceReference::SOURCE_MAX_DEPTH)
                }
                Break::RefSourceDepth => {
                    reference.source = nested(SourceReference::SOURCE_MAX_DEPTH + 1)
                }
                _ => {}
            }
        }
        w.write(AntRecord::OriginalSource { data: reference })
            .unwrap();
    }
    // Its cleaned text: two derivatives of one job, in (jobId, index)
    // order. Slots may be sparse (index 1 is not here).
    let mut first = derivative("ev_original_text_0", 0, "First cleaned block.", &raw);
    let mut second = derivative("ev_original_text_2", 2, "Third cleaned block.", &raw);
    match b {
        Break::DerivativeUnbound => {
            first.derivation.as_mut().unwrap().sha256 = format!("{:x}", Sha256::digest(b"other"))
        }
        Break::DerivativeText => first.content = "Not the named text.".into(),
        Break::DerivativeOrder => std::mem::swap(&mut first, &mut second),
        Break::DerivContract => {
            let d = first.derivation.as_mut().unwrap();
            d.contract = "antares.other-text/v1".into();
        }
        Break::DerivPrimaryControl => {
            let d = first.derivation.as_mut().unwrap();
            d.primary_evidence_id = "ev_original\u{1}".into();
        }
        Break::DerivAssetId => {
            let d = first.derivation.as_mut().unwrap();
            d.asset_id = "short".into();
        }
        Break::DerivSha => {
            let d = first.derivation.as_mut().unwrap();
            d.sha256 = d.sha256.to_uppercase();
        }
        Break::DerivNormalizer => {
            let d = first.derivation.as_mut().unwrap();
            d.normalizer.name = "html5ever visible".into();
        }
        Break::DerivConfiguration => {
            let d = first.derivation.as_mut().unwrap();
            d.normalizer.configuration_sha256 = d.normalizer.configuration_sha256.to_uppercase();
        }
        Break::DerivJobId => {
            let d = first.derivation.as_mut().unwrap();
            d.job_id = "normalization golden".into();
        }
        Break::DerivLocator => {
            let d = first.derivation.as_mut().unwrap();
            d.segment.locator = "x".repeat(ant_types::Derivation::LOCATOR_MAX_BYTES + 1);
        }
        Break::DerivCoverageType => {
            let d = first.derivation.as_mut().unwrap();
            d.segment.coverage = serde_json::json!([0]);
        }
        Break::DerivCoverageSize => {
            let d = first.derivation.as_mut().unwrap();
            d.segment.coverage = serde_json::json!({"units": "x".repeat(9000)});
        }
        Break::DerivTextSha => {
            let d = first.derivation.as_mut().unwrap();
            d.text_sha256 = d.text_sha256.to_uppercase();
        }
        Break::CoverageExact => {
            first.derivation.as_mut().unwrap().segment.coverage = coverage_exact()
        }
        Break::CoverageFloatsUnder => {
            first.derivation.as_mut().unwrap().segment.coverage =
                serde_json::json!({"a": vec![1e-6f64; 1500]})
        }
        Break::CoverageFloatsOver => {
            first.derivation.as_mut().unwrap().segment.coverage =
                serde_json::json!({"a": vec![1.0f64; 2500]})
        }
        Break::DerivWithOriginal => {
            first.source_blob =
                original_evidence("ev_original", "asset_original_0001", &raw).source_blob
        }
        Break::CoverageDepthLimit => {
            first.derivation.as_mut().unwrap().segment.coverage =
                nested(ant_types::Derivation::COVERAGE_MAX_DEPTH)
        }
        Break::DerivCoverageDepth => {
            first.derivation.as_mut().unwrap().segment.coverage =
                nested(ant_types::Derivation::COVERAGE_MAX_DEPTH + 1)
        }
        _ => {}
    }
    if !matches!(b, Break::DerivativeOrphan | Break::Interrupted) {
        w.write(AntRecord::Evidence {
            data: first.clone(),
        })
        .unwrap();
        w.write(AntRecord::Evidence { data: second }).unwrap();
    }
    // Slots are full-range u64: 2^53 and 2^53+1 are distinct (a double
    // cannot tell them apart), and i64::MAX+1 and u64::MAX sort above every
    // smaller slot.
    let wide: &[(&str, u64)] = match b {
        Break::WideSlots => &[
            ("normalization-golden-0001", 9_007_199_254_740_992),
            ("normalization-golden-0001", 9_007_199_254_740_993),
            ("normalization-golden-0001", 9_223_372_036_854_775_808),
            ("normalization-golden-0001", u64::MAX),
        ],
        _ => &[],
    };
    for (n, (job, index)) in wide.iter().enumerate() {
        let d = derivative_in(
            job,
            &format!("ev_original_wide_{n}"),
            *index,
            &format!("Wide block {n}."),
            &raw,
        );
        w.write(AntRecord::Evidence { data: d }).unwrap();
    }
    if !matches!(b, Break::Interrupted) {
        // An empty original: zero chunks, and the empty digest.
        w.write(AntRecord::Evidence {
            data: original_evidence("ev_empty", "asset_empty_00001", b""),
        })
        .unwrap();
        w.write(AntRecord::Evidence { data: plain }).unwrap();
    }
    if matches!(b, Break::DerivativeOrphan) {
        // Cleaned text that does not follow its primary's original.
        w.write(AntRecord::Evidence { data: first }).unwrap();
    }
    w.finish().unwrap()
}

fn main() {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("mkdir golden");
    std::fs::write(dir.join("basic.ant"), basic()).expect("write basic.ant");
    std::fs::write(dir.join("unknown_time.ant"), unknown_time()).expect("write unknown_time.ant");
    std::fs::write(dir.join("forward_compat.ant"), forward_compat())
        .expect("write forward_compat.ant");
    std::fs::write(dir.join("tombstones.ant"), tombstones()).expect("write tombstones.ant");
    std::fs::write(dir.join("major_version.ant"), major_version())
        .expect("write major_version.ant");
    std::fs::write(dir.join("contradiction_cases.ant"), contradiction_cases())
        .expect("write contradiction_cases.ant");
    std::fs::write(
        dir.join("relationship_proposals.ant"),
        relationship_proposals(),
    )
    .expect("write relationship_proposals.ant");
    let (ontology_bytes, ontology_revision) = ontology_revisions();
    std::fs::write(dir.join("ontology_revisions.ant"), ontology_bytes)
        .expect("write ontology_revisions.ant");
    std::fs::write(dir.join("originals.ant"), originals(Break::None)).expect("write originals.ant");
    // Raw-literal fixtures on the first derivative (slot 0, coverage
    // {"units":[0]}): a typed u64 slot may not be `-0` (a double to Rust);
    // opaque coverage may (it is -0.0 data); no number may be out of range.
    let base = originals(Break::None);
    for (name, from, to) in [
        (
            "derivative_index_negative_zero.ant",
            r#""segment":{"index":0,"#,
            r#""segment":{"index":-0,"#,
        ),
        (
            "derivative_coverage_negative_zero.ant",
            r#""coverage":{"units":[0]}"#,
            r#""coverage":{"units":[-0]}"#,
        ),
        (
            "derivative_coverage_non_finite.ant",
            r#""coverage":{"units":[0]}"#,
            r#""coverage":{"units":[1e999]}"#,
        ),
        // A valid finite double the shared parser refuses as out of range;
        // correctly rounded it is f64::MAX (revision 29).
        (
            "source_reference_max_double.ant",
            r#""source":{"driveId":"drive-1","itemId":"item-9"}"#,
            r#""source":{"driveId":"drive-1","itemId":"item-9","max":17976931348623158e292}"#,
        ),
    ] {
        std::fs::write(dir.join(name), edit_one_line(&base, from, to))
            .unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
    for (name, b) in [
        ("original_missing_chunk.ant", Break::MissingChunk),
        ("original_reordered.ant", Break::Reordered),
        ("original_chunk_digest.ant", Break::ChunkDigest),
        ("original_whole_digest.ant", Break::WholeDigest),
        ("original_interrupted.ant", Break::Interrupted),
        ("original_in_v0.ant", Break::InV0),
        ("original_source_unbound.ant", Break::UnboundSource),
        ("derivative_orphan.ant", Break::DerivativeOrphan),
        ("derivative_unbound.ant", Break::DerivativeUnbound),
        ("derivative_text_mismatch.ant", Break::DerivativeText),
        ("derivative_out_of_order.ant", Break::DerivativeOrder),
        ("derivative_wide_slots.ant", Break::WideSlots),
        ("derivative_coverage_exact.ant", Break::CoverageExact),
        (
            "derivative_coverage_floats_under.ant",
            Break::CoverageFloatsUnder,
        ),
        (
            "derivative_coverage_floats_over.ant",
            Break::CoverageFloatsOver,
        ),
        ("source_reference_exact.ant", Break::SourceExact),
        ("derivative_contract.ant", Break::DerivContract),
        ("derivative_primary_control.ant", Break::DerivPrimaryControl),
        ("derivative_asset_id.ant", Break::DerivAssetId),
        ("derivative_sha256.ant", Break::DerivSha),
        ("derivative_normalizer.ant", Break::DerivNormalizer),
        ("derivative_configuration.ant", Break::DerivConfiguration),
        ("derivative_job_id.ant", Break::DerivJobId),
        ("derivative_locator.ant", Break::DerivLocator),
        ("derivative_coverage_type.ant", Break::DerivCoverageType),
        ("derivative_coverage_size.ant", Break::DerivCoverageSize),
        ("derivative_text_sha256.ant", Break::DerivTextSha),
        ("derivative_with_original.ant", Break::DerivWithOriginal),
        ("original_blob_asset_id.ant", Break::BlobAssetId),
        ("original_blob_sha256.ant", Break::BlobSha),
        ("original_blob_media_type.ant", Break::BlobMediaType),
        ("original_blob_file_name.ant", Break::BlobFileName),
        ("source_reference_id.ant", Break::RefId),
        ("source_reference_source_type.ant", Break::RefSourceType),
        ("source_reference_source_size.ant", Break::RefSourceSize),
        (
            "source_reference_evidence_control.ant",
            Break::RefEvidenceControl,
        ),
        ("source_reference_asset_id.ant", Break::RefAssetId),
        (
            "derivative_coverage_depth_limit.ant",
            Break::CoverageDepthLimit,
        ),
        ("derivative_coverage_depth.ant", Break::DerivCoverageDepth),
        ("source_reference_depth_limit.ant", Break::SourceDepthLimit),
        ("source_reference_source_depth.ant", Break::RefSourceDepth),
    ] {
        std::fs::write(dir.join(name), originals(b))
            .unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
    let original_sha = format!("{:x}", Sha256::digest(original_bytes()));
    let empty_sha = format!("{:x}", Sha256::digest(b""));
    let mut expected = serde_json::json!({
        "basic.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 2, "edges": 1,
                        "observations": 1, "evidence": 1, "beliefs": 1, "vectors": 1,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 0},
            "recordKinds": ["vertex", "vertex", "edge", "observation",
                             "evidence", "belief", "vector"],
            "firstVertexId": "deal_1"
        },
        "unknown_time.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 0, "edges": 0,
                        "observations": 2, "evidence": 0, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 0},
            "recordKinds": ["observation", "observation"],
            // In file order (undated first, dated second). A binding that
            // could not read the v0.6 wire form would classify these
            // wrong or fail to read them at all.
            "observedTimeStates": ["unknown", "known"],
            // The undated one's PROVENANCE time carries its basis; the
            // dated one's is a bare string, so no basis. `null` marks the
            // bare-string form the check must still accept.
            "extractedTimeBases": ["source_record_time", null],
            "unknownReasons": ["no_source_time"]
        },
        "forward_compat.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 1, "edges": 0,
                        "observations": 0, "evidence": 0, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 0},
            "recordKinds": ["vertex"],
            "skippedKinds": ["hologram"]
        },
        "tombstones.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 1, "edges": 1,
                        "observations": 0, "evidence": 0, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 1, "edgeTombstones": 1, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 0},
            "recordKinds": ["vertex", "edge", "vertex_tombstone", "edge_tombstone"],
            "firstVertexId": "deal_live"
        },
        "contradiction_cases.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 2, "edges": 0,
                        "observations": 4, "evidence": 8, "beliefs": 1, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 4,
                        "relationshipProposals": 0, "ontologyRevisions": 0},
            "recordKinds": ["vertex", "vertex",
                             "evidence", "evidence", "evidence", "evidence",
                             "evidence", "evidence", "evidence", "evidence",
                             "observation", "observation", "observation", "observation",
                             "belief",
                             "contradiction_case", "contradiction_case",
                             "contradiction_case", "contradiction_case"],
            "firstVertexId": "co_1",
            // In file order. A binding that reads the kind proves it
            // by reporting the state families, not just a count.
            "epistemicStates": ["incompatible", "insufficiently_comparable",
                                 "incompatible", "compatible"],
            "workflowStates": ["awaiting_review", "settled", "open", "settled"]
        },
        "relationship_proposals.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 0, "edges": 0,
                        "observations": 0, "evidence": 5, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 3, "ontologyRevisions": 0},
            "recordKinds": ["evidence", "evidence", "evidence", "evidence", "evidence",
                             "relationship_proposal", "relationship_proposal",
                             "relationship_proposal"],
            // In file order. A binding that reads the kind proves it by
            // reporting the statuses — and the ratios, because the
            // quarantined one's 0/1914 is the whole point of carrying it.
            "proposalStatuses": ["supported", "quarantined_hypothesis",
                                  "promoted_by_reviewer"],
            "proposalMatched": [890, 0, 890],
            "proposalNonNull": [900, 1914, 900]
        },
        "ontology_revisions.ant": {
            "version": FORMAT_VERSION,
            "tenantId": 7,
            "projectId": 7,
            "counts": {"schemaTypes": 0, "vertices": 0, "edges": 0,
                        "observations": 0, "evidence": 1, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 1},
            "recordKinds": ["evidence", "ontology_revision"],
            "ontologyRevisionIds": [ontology_revision.id.0],
            "ontologyTargetVaults": ["team:operations"],
            "ontologyPreviousRevisionIds": [null],
            "ontologySemanticKinds": [["schema_type"]],
            "ontologyConditionalDomains": ["ontology/v1"],
            "ontologyConditionalChains": ["ontology"],
            "ontologyPublisherPrincipals": ["machine:main-server"],
            "ontologyApprovalAttesters": ["machine:main-server"],
            "ontologyRetainedDispositions": [["accepted"]],
            "ontologyAttributionPrincipals": [["user:sme-a"]]
        },
        "originals.ant": {
            "version": ORIGINALS_FORMAT_VERSION,
            "tenantId": 1,
            "projectId": 1,
            "counts": {"schemaTypes": 0, "vertices": 0, "edges": 0,
                        "observations": 0, "evidence": 5, "beliefs": 0, "vectors": 0,
                        "vertexTombstones": 0, "edgeTombstones": 0, "contradictionCases": 0,
                        "relationshipProposals": 0, "ontologyRevisions": 0,
                        "originalChunks": 3, "originalSources": 2},
            "recordKinds": ["evidence", "original_chunk", "original_chunk", "original_chunk",
                             "original_source", "original_source", "evidence", "evidence",
                             "evidence", "evidence"],
            // Cleaned-text derivatives, in file order: (id, primary, job, slot).
            "derivatives": [
                {"evidenceId": "ev_original_text_0", "primaryEvidenceId": "ev_original",
                 "jobId": "normalization-golden-0001", "index": 0},
                {"evidenceId": "ev_original_text_2", "primaryEvidenceId": "ev_original",
                 "jobId": "normalization-golden-0001", "index": 2}
            ],
            // The source references, in file order.
            "sourceReferenceIds": ["ref-drive-0001", "ref-picker-0001"],
            // Reassembled from the chunks, in file order. The empty
            // original has zero chunks and the empty digest.
            "originals": [
                {"evidenceId": "ev_original", "byteLength": 150, "sha256": original_sha,
                 "chunks": 3},
                {"evidenceId": "ev_empty", "byteLength": 0, "sha256": empty_sha, "chunks": 0}
            ]
        }
    });
    // The wide-slot golden is originals.ant plus four more derivatives of
    // ev_original, after the first two.
    let mut wide = expected["originals.ant"].clone();
    wide["counts"]["evidence"] = serde_json::json!(9);
    let kinds = wide["recordKinds"].as_array_mut().unwrap();
    for _ in 0..4 {
        kinds.insert(8, serde_json::json!("evidence"));
    }
    let derivatives = wide["derivatives"].as_array_mut().unwrap();
    for (n, (job, index)) in [
        ("normalization-golden-0001", 9_007_199_254_740_992u64),
        ("normalization-golden-0001", 9_007_199_254_740_993),
        ("normalization-golden-0001", 9_223_372_036_854_775_808),
        ("normalization-golden-0001", u64::MAX),
    ]
    .into_iter()
    .enumerate()
    {
        derivatives.push(serde_json::json!({
            "evidenceId": format!("ev_original_wide_{n}"),
            "primaryEvidenceId": "ev_original",
            "jobId": job,
            "index": index,
        }));
    }
    expected["derivative_wide_slots.ant"] = wide;
    // The source-exact golden: originals.ant with the first reference's
    // source replaced; returned exactly and counted as Rust does.
    {
        let mut e = expected["originals.ant"].clone();
        e["sourceBytes"] = serde_json::json!([
            serde_json::to_vec(&coverage_exact()).unwrap().len(),
            serde_json::to_vec(&serde_json::json!({"origin": "picker"}))
                .unwrap()
                .len()
        ]);
        e["source"] = coverage_exact();
        expected["source_reference_exact.ant"] = e;
    }
    // A source nested exactly at the limit reads, returned exactly.
    {
        let mut e = expected["originals.ant"].clone();
        let first = nested(SourceReference::SOURCE_MAX_DEPTH);
        e["sourceBytes"] = serde_json::json!([
            serde_json::to_vec(&first).unwrap().len(),
            serde_json::to_vec(&serde_json::json!({"origin": "picker"}))
                .unwrap()
                .len()
        ]);
        e["source"] = first;
        expected["source_reference_depth_limit.ant"] = e;
    }
    // f64::MAX written as the token the shared parser refuses: every reader
    // returns it exactly and counts it as Rust writes it.
    {
        let mut e = expected["originals.ant"].clone();
        let first = serde_json::json!({"driveId": "drive-1", "itemId": "item-9", "max": f64::MAX});
        e["sourceBytes"] = serde_json::json!([
            serde_json::to_vec(&first).unwrap().len(),
            serde_json::to_vec(&serde_json::json!({"origin": "picker"}))
                .unwrap()
                .len()
        ]);
        e["source"] = first;
        expected["source_reference_max_double.ant"] = e;
    }
    // Opaque -0 is data: accepted, counted as Rust writes it (-0.0).
    {
        let mut e = expected["originals.ant"].clone();
        e["coverageBytes"] = serde_json::json!([
            serde_json::to_vec(&serde_json::json!({"units": [-0.0f64]}))
                .unwrap()
                .len(),
            serde_json::to_vec(&serde_json::json!({"units": [2]}))
                .unwrap()
                .len()
        ]);
        e["coverage"] = serde_json::json!({"units": [-0.0f64]});
        expected["derivative_coverage_negative_zero.ant"] = e;
    }
    // Coverage goldens: originals.ant with the first derivative's coverage
    // replaced. `coverageBytes` is each derivative's coverage size as Rust
    // serializes it; `coverage` is the first one's value, to be returned
    // exactly.
    for (name, first_coverage) in [
        ("derivative_coverage_exact.ant", coverage_exact()),
        (
            "derivative_coverage_floats_under.ant",
            serde_json::json!({"a": vec![1e-6f64; 1500]}),
        ),
        (
            "derivative_coverage_depth_limit.ant",
            nested(ant_types::Derivation::COVERAGE_MAX_DEPTH),
        ),
    ] {
        let mut e = expected["originals.ant"].clone();
        let second_coverage = serde_json::json!({"units": [2]});
        e["coverageBytes"] = serde_json::json!([
            serde_json::to_vec(&first_coverage).unwrap().len(),
            serde_json::to_vec(&second_coverage).unwrap().len()
        ]);
        e["coverage"] = first_coverage;
        expected[name] = e;
    }
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
            "why": "declares format v2.0; a reader of 0.x and 1.x must refuse rather \
                    than misread, because a major bump means field meanings or the \
                    container framing changed. Valid in every other respect, so \
                    rejecting it for any other reason is the wrong pass."
        },
        "original_missing_chunk.ant": {
            "mustReject": true,
            "why": "v1.0: chunk 1 of ev_original is absent; its original cannot be whole."
        },
        "original_reordered.ant": {
            "mustReject": true,
            "why": "v1.0: chunks 1 and 2 of ev_original are swapped; chunks are contiguous \
                    and in order."
        },
        "original_chunk_digest.ant": {
            "mustReject": true,
            "why": "v1.0: chunk 1 of ev_original does not match its own sha256."
        },
        "original_whole_digest.ant": {
            "mustReject": true,
            "why": "v1.0: every chunk matches its own sha256, but together they are not \
                    the original ev_original's sourceBlob.sha256 declares."
        },
        "original_interrupted.ant": {
            "mustReject": true,
            "why": "v1.0: another record appears between ev_original's chunks; an \
                    original's chunks follow its evidence with nothing in between."
        },
        "original_source_unbound.ant": {
            "mustReject": true,
            "why": "v1.0: a source reference after ev_original names different bytes (another \
                    sha256); a reference binds to exactly the original it follows."
        },
        "derivative_orphan.ant": {
            "mustReject": true,
            "why": "v1.0: a cleaned-text derivative of ev_original appears after unrelated \
                    records; a derivative follows its primary's original and source references."
        },
        "derivative_unbound.ant": {
            "mustReject": true,
            "why": "v1.0: a derivative after ev_original names different bytes (another sha256); \
                    a derivation binds exactly the original it follows."
        },
        "derivative_text_mismatch.ant": {
            "mustReject": true,
            "why": "v1.0: a derivative's content is not the text its derivation names \
                    (textSha256/textByteLength)."
        },
        "derivative_out_of_order.ant": {
            "mustReject": true,
            "why": "v1.0: ev_original's derivatives are not in strictly increasing \
                    (jobId, index) order."
        },
        "derivative_contract.ant": {
            "mustReject": true,
            "refusedFor": "derivation.contract must be",
            "why": "v1.0: a derivative of ev_original names a contract other than antares.normalized-text/v1; the Rust Derivation rule refuses it."
        },
        "derivative_primary_control.ant": {
            "mustReject": true,
            "refusedFor": "primaryEvidenceId must be non-empty with no control characters",
            "why": "v1.0: a derivative of ev_original names a primaryEvidenceId with a control character; the Rust Derivation rule refuses it."
        },
        "derivative_asset_id.ant": {
            "mustReject": true,
            "refusedFor": "assetId/sha256 are malformed",
            "why": "v1.0: a derivative of ev_original names an assetId shorter than 16 characters; the Rust Derivation rule refuses it."
        },
        "derivative_sha256.ant": {
            "mustReject": true,
            "refusedFor": "assetId/sha256 are malformed",
            "why": "v1.0: a derivative of ev_original names a sha256 that is not lowercase hex; the Rust Derivation rule refuses it."
        },
        "derivative_normalizer.ant": {
            "mustReject": true,
            "refusedFor": "normalizer.name/version must be",
            "why": "v1.0: a derivative of ev_original names a normalizer outside [A-Za-z0-9_.:-]; the Rust Derivation rule refuses it."
        },
        "derivative_configuration.ant": {
            "mustReject": true,
            "refusedFor": "configurationSha256 must be 64 lowercase hex",
            "why": "v1.0: a derivative of ev_original names a normalizer configurationSha256 that is not lowercase hex; the Rust Derivation rule refuses it."
        },
        "derivative_job_id.ant": {
            "mustReject": true,
            "refusedFor": "jobId must be 1..=128 characters",
            "why": "v1.0: a derivative of ev_original names a jobId outside [A-Za-z0-9_.:-]; the Rust Derivation rule refuses it."
        },
        "derivative_locator.ant": {
            "mustReject": true,
            "refusedFor": "segment.locator must be",
            "why": "v1.0: a derivative of ev_original names a locator one byte over 2 KiB; the Rust Derivation rule refuses it."
        },
        "derivative_coverage_type.ant": {
            "mustReject": true,
            "refusedFor": "coverage must be a JSON object",
            "why": "v1.0: a derivative of ev_original carries coverage that is not a JSON object; the Rust Derivation rule refuses it."
        },
        "derivative_coverage_size.ant": {
            "mustReject": true,
            "refusedFor": "coverage exceeds",
            "why": "v1.0: a derivative of ev_original carries coverage over 8 KiB serialized; the Rust Derivation rule refuses it."
        },
        "derivative_coverage_depth.ant": {
            "mustReject": true,
            "refusedFor": "coverage nests deeper than 64 levels",
            "why": "v1.0: a derivative of ev_original carries coverage nesting 65 objects deep, one past the 64-level bound that keeps it readable inside every envelope; the Rust Derivation rule refuses it."
        },
        "derivative_text_sha256.ant": {
            "mustReject": true,
            "refusedFor": "textSha256 must be 64 lowercase hex",
            "why": "v1.0: a derivative of ev_original names a textSha256 that is not lowercase hex; the Rust Derivation rule refuses it."
        },
        "derivative_index_negative_zero.ant": {
            "mustReject": true,
            "refusedFor": "malformed `evidence` record",
            "why": "v1.0: a derivative's typed u64 segment.index is the literal -0, which the Rust reader decodes as a double, not a u64."
        },
        "derivative_coverage_non_finite.ant": {
            "mustReject": true,
            "refusedFor": "number out of range",
            "why": "v1.0: a derivative's opaque coverage holds 1e999, which no double holds; the Rust reader refuses it (serde_json: number out of range) and no reader may turn it into an infinity."
        },
        "derivative_coverage_floats_over.ant": {
            "mustReject": true,
            "refusedFor": "coverage exceeds",
            "why": "v1.0: a derivative's coverage is 2500 copies of 1.0: 10 KiB as the Rust reader serializes it (1.0 is written 1.0), over the 8 KiB bound; a reader counting its own shorter spelling would accept it."
        },
        "derivative_with_original.ant": {
            "mustReject": true,
            "refusedFor": "carries both a derivation and an original",
            "why": "v1.0: a derivative of ev_original also carries a sourceBlob; a derivative carries no original."
        },
        "original_blob_asset_id.ant": {
            "mustReject": true,
            "refusedFor": "sourceBlob.assetId must be 16..=128 characters",
            "why": "v1.0: ev_original's sourceBlob names an assetId shorter than 16 characters; the Rust rule refuses it."
        },
        "original_blob_sha256.ant": {
            "mustReject": true,
            "refusedFor": "sourceBlob.sha256 must be 64 lowercase hex",
            "why": "v1.0: ev_original's sourceBlob names a sha256 that is not lowercase hex; the Rust rule refuses it."
        },
        "original_blob_media_type.ant": {
            "mustReject": true,
            "refusedFor": "sourceBlob.mediaType must be 1..=255 printable ASCII",
            "why": "v1.0: ev_original's sourceBlob declares a mediaType that is not printable ASCII; the Rust rule refuses it."
        },
        "original_blob_file_name.ant": {
            "mustReject": true,
            "refusedFor": "sourceBlob.fileName must be 1..=1024 bytes with no control",
            "why": "v1.0: ev_original's sourceBlob declares a fileName with a control character; the Rust rule refuses it."
        },
        "source_reference_id.ant": {
            "mustReject": true,
            "refusedFor": "reference.referenceId must be 1..=128 characters",
            "why": "v1.0: ev_original's first source reference names a referenceId outside [A-Za-z0-9_.:-]; the Rust rule refuses it."
        },
        "source_reference_source_type.ant": {
            "mustReject": true,
            "refusedFor": "reference.source must be a JSON object",
            "why": "v1.0: ev_original's first source reference carries a source that is not a JSON object; the Rust rule refuses it."
        },
        "source_reference_source_size.ant": {
            "mustReject": true,
            "refusedFor": "reference.source exceeds",
            "why": "v1.0: ev_original's first source reference carries a source over 16 KiB serialized; the Rust rule refuses it."
        },
        "source_reference_source_depth.ant": {
            "mustReject": true,
            "refusedFor": "reference.source nests deeper than 64 levels",
            "why": "v1.0: ev_original's first source reference carries a source nesting 65 objects deep, one past the 64-level bound; the Rust rule refuses it."
        },
        "source_reference_evidence_control.ant": {
            "mustReject": true,
            "refusedFor": "reference.evidenceId must be non-empty with no control",
            "why": "v1.0: ev_original's first source reference names an evidenceId with a control character; the Rust rule refuses it."
        },
        "source_reference_asset_id.ant": {
            "mustReject": true,
            "refusedFor": "reference.assetId/sha256 are malformed",
            "why": "v1.0: ev_original's first source reference names an assetId shorter than 16 characters; the Rust rule refuses it."
        },
        "original_in_v0.ant": {
            "mustReject": true,
            "why": "declares v0.7 but carries a sourceBlob and original_chunk records, \
                    which only v1.0 may: a 0.x reader would drop them silently."
        }
    });
    std::fs::write(
        dir.join("expected_negatives.json"),
        serde_json::to_string_pretty(&negatives).unwrap(),
    )
    .expect("write expected_negatives.json");
    eprintln!("wrote goldens to {}", dir.display());
}
