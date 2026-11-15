//! Lane symcert REAL import-side binding (ignored; audit r2-sym F4): a real import with a supplied relation source, the
//! sealed kernel certificates of those relations bound by `bind_certificates` (pointer verdicts -> re-verified sealed
//! runs, pinned kernel, the import's family, relation digest from the import's own record), and the import check
//! before / after: the certified keys move to per-key `symmetry_relation_kernel_certified`; every other row unchanged.
//! Driven by `SYMCERT_BIND` = JSON {"import_spec": path, "base": dir, "verdicts": path, "roots": [dirs],
//! "kernels": [sha256], "frame_table": path, "out": path}. No process is started here.

use rustred_campaign::closeloop::symcert::{
    ImportBinding, bind_certificates, import_records, read_verdicts,
};
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, check, load};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[test]
#[ignore = "real import: SYMCERT_BIND=<config.json>"]
fn symcert_bind_real() {
    let c: Value = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("SYMCERT_BIND").unwrap()).unwrap(),
    )
    .unwrap();
    let st = |k: &str| c[k].as_str().unwrap().to_string();
    let spec =
        TableSetSpec::from_json(&std::fs::read_to_string(st("import_spec")).unwrap()).unwrap();
    let set = load(&spec, Path::new(&st("base"))).unwrap();
    let h =
        rustred_campaign_core::import::lines::read_header("frame", Path::new(&st("frame_table")))
            .unwrap();
    let fam = h.raw["family_source"].as_str().unwrap();
    let binding = ImportBinding {
        kernels: serde_json::from_value(c["kernels"].clone()).unwrap(),
        family_blake3: rustred_campaign_contract::blake3_hex(fam.as_bytes()),
        roots: serde_json::from_value::<Vec<PathBuf>>(c["roots"].clone()).unwrap(),
    };
    let verdicts = read_verdicts(Path::new(&st("verdicts"))).unwrap();
    let records = import_records(&set);
    let (certs, no) = bind_certificates(&verdicts, &records, &binding);
    let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &no {
        *reasons.entry(n.reason).or_insert(0) += 1;
    }
    let opts = CheckOptions::default();
    let before = check(&set, &Evidence::new(&set), &opts).unwrap();
    let mut ev = Evidence::new(&set);
    ev.symmetry_certificates = Some(certs.clone());
    let after = check(&set, &ev, &opts).unwrap();
    let mut moved: BTreeMap<String, usize> = BTreeMap::new();
    let mut other_changed = 0usize;
    let bmap: BTreeMap<&Vec<i32>, _> = before.rows.iter().map(|r| (&r.key, r)).collect();
    for r in &after.rows {
        let b = bmap[&r.key];
        let was = (b.status, b.evidence, b.basis);
        let now = (r.status, r.evidence, r.basis);
        if was != now {
            if r.basis
                .is_some_and(|x| x.starts_with("symmetry_relation_kernel"))
            {
                *moved
                    .entry(format!(
                        "{:?}/{:?} -> {:?}/{:?}",
                        b.evidence, b.basis, r.evidence, r.basis
                    ))
                    .or_insert(0) += 1;
            } else {
                other_changed += 1;
            }
        }
    }
    let out = json!({"verdicts": verdicts.len(), "bound": certs.len(), "no_lift": reasons,
        "rows": after.rows.len(), "moved": moved, "other_rows_changed": other_changed});
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    std::fs::write(st("out"), serde_json::to_vec_pretty(&out).unwrap()).unwrap();
    assert_eq!(
        other_changed, 0,
        "a certificate changed a row it does not bind"
    );
}
