//! Demo: write a small `.ant` stream and read it back, printing the
//! compressed vs raw sizes.

use ant_types::{Evidence, EvidenceId, ProjectId, TenantId};
use antares_format::*;
fn main() {
    let manifest = Manifest {
        format: "antares".into(),
        version: FORMAT_VERSION.into(),
        tenant_id: 1,
        project_id: 1,
        selection: None,
        created_at: None,
        producer: None,
    };
    let mut w = AntWriter::new(Vec::new(), manifest, 0).unwrap();
    let mut raw_size = 0usize;
    for i in 0..200 {
        let e = Evidence {
            id: EvidenceId(format!("ev{i}")),
            tenant_id: TenantId(1),
            project_id: ProjectId(1),
            source_uri: format!("antares://mail/{i}"),
            source_type: "email".into(),
            source_id: format!("m{i}"),
            content: format!(
                "Dear team, following up on the proposal discussion from our call. \
                The client asked about implementation timelines and pricing tiers. Item {i}."
            ),
            source_blob: None,
            derivation: None,
            char_start: None,
            char_end: None,
            byte_start: None,
            byte_end: None,
            observed_at: None,
            extracted_at: None,
            extractor_version: None,
            confidence: None,
            metadata: serde_json::Value::Null,
            author: None,
        };
        raw_size += serde_json::to_string(&e).unwrap().len();
        w.write(AntRecord::Evidence { data: e }).unwrap();
    }
    let bytes = w.finish().unwrap();
    std::fs::write("/tmp/demo.ant", &bytes).unwrap();
    println!(
        "logical JSON size: {raw_size} bytes | .ant file: {} bytes | ratio {:.1}x",
        bytes.len(),
        raw_size as f64 / bytes.len() as f64
    );
}
