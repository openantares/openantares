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
    RelationshipProposalId, ReviewerReceipt, Sampling, SchemaType, SourceDependency,
    SourceManifestRef, SourcePointer, SpgTypeKind, SubjectType, SupportMethod, TenantId, TokenId,
    TypeName, UserId, ValueType, VaultOccurrence, Vertex, VertexId, WorkflowState,
    ONTOLOGY_CHAIN_ID, ONTOLOGY_REVISION_DOMAIN,
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
    let expected = serde_json::json!({
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
