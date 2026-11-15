//! Lane symcert REAL hand-over to the import (ignored; audit r2-sym F4, lane symfix's route (b)): the sealed certificate
//! streams of a real certification batch, re-verified (run directory under the roots, row 40 from its own files, the
//! sealed line re-checked against its pinned inputs, kernel pinned, family = the frame's), listed as the import spec's
//! `kernel_certificates` {files [{path, sha256}], kernels}. Driven by `SYMCERT_BIND` = JSON {"verdicts": path,
//! "roots": [dirs], "kernels": [sha256], "frame_table": path, "out": path}. No process is started here.

use rustred_campaign::closeloop::symcert::{
    ImportBinding, kernel_certificate_files, read_verdicts,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[test]
#[ignore = "real certificates: SYMCERT_BIND=<config.json>"]
fn symcert_bind_real() {
    let c: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("SYMCERT_BIND").unwrap()).unwrap(),
    )
    .unwrap();
    let st = |k: &str| c[k].as_str().unwrap().to_string();
    let h =
        rustred_campaign_core::import::lines::read_header("frame", Path::new(&st("frame_table")))
            .unwrap();
    let fam = h.raw["family_source"].as_str().unwrap();
    let kernels: Vec<String> = serde_json::from_value(c["kernels"].clone()).unwrap();
    let binding = ImportBinding {
        kernels: kernels.clone(),
        family_blake3: rustred_campaign_contract::blake3_hex(fam.as_bytes()),
        roots: serde_json::from_value::<Vec<PathBuf>>(c["roots"].clone()).unwrap(),
    };
    let verdicts = read_verdicts(Path::new(&st("verdicts"))).unwrap();
    let (files, no) = kernel_certificate_files(&verdicts, &binding);
    let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &no {
        *reasons.entry(n.reason).or_insert(0) += 1;
    }
    let certified = verdicts.iter().filter(|v| v.verdict == "certified").count();
    let out = json!({"verdicts": verdicts.len(), "certified": certified, "files": files.len(),
        "not_handed_on": reasons,
        "kernel_certificates": {"files": files.iter().map(|f| json!({"path": f.path, "sha256": f.sha256}))
            .collect::<Vec<_>>(), "kernels": kernels}});
    std::fs::write(st("out"), serde_json::to_vec_pretty(&out).unwrap()).unwrap();
    println!(
        "{}",
        json!({"verdicts": verdicts.len(), "certified": certified, "files": files.len(), "not_handed_on": reasons})
    );
    assert_eq!(files.len(), certified, "every certified run is handed on");
}
