//! Streamed manifests retain exact wire bytes and propagate sink errors.
use antares_format::{AntReader, AntRecord, AntWriter, Manifest, FORMAT_VERSION};
use serde::ser::{SerializeSeq, Serializer};
use serde::Serialize;

struct Ids;
impl Serialize for Ids {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(None)?;
        for id in 0..10000 {
            seq.serialize_element(&format!("v{id:05}"))?;
        }
        seq.end()
    }
}

#[test]
fn disk_style_selection_is_identical_to_a_materialized_manifest() {
    let selection = serde_json::json!({"vaults": {"team": {"vertices": (0..10000).map(|id|format!("v{id:05}")).collect::<Vec<_>>()}}});
    let manifest = Manifest {
        format: "antares".into(),
        version: FORMAT_VERSION.into(),
        tenant_id: 1,
        project_id: 7,
        selection: Some(selection),
        created_at: Some("2026-09-14T00:00:00Z".parse().unwrap()),
        producer: Some("test".into()),
    };
    #[derive(Serialize)]
    struct Vertices {
        vertices: Ids,
    }
    #[derive(Serialize)]
    struct Team {
        team: Vertices,
    }
    #[derive(Serialize)]
    struct Selection {
        vaults: Team,
    }
    let streaming = Selection {
        vaults: Team {
            team: Vertices { vertices: Ids },
        },
    };
    let record = AntRecord::Evidence {
        data: ant_types::Evidence::quick(
            "e",
            ant_types::TenantId(1),
            ant_types::ProjectId(7),
            "test",
            "e",
            "the evidence",
        ),
    };
    let mut a = AntWriter::new(Vec::new(), manifest.clone(), 0).unwrap();
    a.write(record.clone()).unwrap();
    let a = a.finish().unwrap();
    let mut b = AntWriter::new_with_selection(Vec::new(), &manifest, &streaming, 0).unwrap();
    b.write(record.clone()).unwrap();
    let b = b.finish().unwrap();
    assert!(
        a == b,
        "streamed manifest changed compressed bytes: {} vs {} bytes",
        a.len(),
        b.len()
    );
    let mut r = AntReader::new(&b[..]).unwrap();
    assert_eq!(r.manifest, manifest);
    assert_eq!(r.next_record().unwrap(), Some(record));
    assert_eq!(r.next_record().unwrap(), None);
    assert!(r.verified, "incremental counts/hash did not verify");
}

#[test]
fn sink_failure_is_returned_instead_of_finishing_an_archive() {
    struct Fails;
    impl std::io::Write for Fails {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let manifest = Manifest {
        format: "antares".into(),
        version: FORMAT_VERSION.into(),
        tenant_id: 1,
        project_id: 1,
        selection: None,
        created_at: None,
        producer: None,
    };
    let result = AntWriter::new(Fails, manifest, 0).and_then(|w| w.finish());
    assert!(
        result.is_err(),
        "a failing destination was reported as a finished archive"
    );
}
