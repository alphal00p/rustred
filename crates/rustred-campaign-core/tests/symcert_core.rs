//! Lane symcert (interfaces/proof-symrows-job.md 6-7): the import's evidence hook for KERNEL CERTIFICATES of supplied
//! symmetry relations (certified -> per-key `symmetry_relation_kernel_certified`; refused -> `imported`
//! `symmetry_relation_kernel_refused`; none -> unchanged `symmetry_relation`; no other row changes), and the readers of
//! the three producers' files on the real records (counts only; comparison data, skipped when absent).
//! Test data: T178's 4 supplied symmetry relations (WS SCRATCHPAD, read-only) and the lanes' scratch records.

use rustred_campaign_core::import;
use rustred_campaign_core::symcert::{SymmetryCertificates, producers};
use std::path::{Path, PathBuf};

use import::lines::parse_key_text;
use import::{CheckOptions, Evidence, Level, RowStatus, TableSetSpec, check, load};
use serde_json::{Value, json};

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}
fn scratch() -> PathBuf {
    PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/implementer-scratch")
}

#[test]
fn kernel_certificates_set_the_level_of_supplied_symmetry_relations_and_nothing_else() {
    let ws = ws();
    let fam = ws.join("handoff/sym-investigation-for-ben/families/T178.sym");
    let sym = ws.join("SCRATCHPAD/t178-close/data/symrules.table.jsonl");
    let piece = ws.join("SCRATCHPAD/t178/regenB/R0/try1/node/F.table.jsonl");
    if !fam.is_file() || !sym.is_file() || !piece.is_file() {
        println!("SKIP: T178 family / relations / frame table missing");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("symcert-core-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let text = std::fs::read_to_string(&sym).unwrap();
    // (record sha256, key) of every supplied relation line
    let rels: Vec<(String, Vec<i32>)> = text
        .lines()
        .filter_map(|l| {
            let v: Value = serde_json::from_str(l).ok()?;
            (v["type"] == "rule").then(|| {
                (
                    producers::sha256_hex(l.as_bytes()),
                    parse_key_text(v["target"].as_str().unwrap(), 20).unwrap(),
                )
            })
        })
        .collect();
    assert_eq!(rels.len(), 4);
    let start = tmp.join("start.jsonl");
    std::fs::write(
        &start,
        rels.iter()
            .map(|(_, k)| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"R0","required_evidence":"per-key",
        "start":{"path": start.display().to_string(),"format":"keys_jsonl"},
        "sources":[
            {"id":"symmetry","kind":"supplied_relations","class":"symmetry","path": sym.display().to_string(),
             "verify":{"family":{"path": fam.display().to_string(),
                 "sha256":"eb34c9dedbe35b752f6b085f83ca4a6b63ff8fc9a5a7c46f18ee46d9d76cc608"}}},
            {"id":"R0","kind":"engine_table","path": piece.display().to_string(),"records":[0]},
            {"id":"credits","kind":"symmetry_credit","family":{"path": fam.display().to_string()}}]});
    let set = load(
        &TableSetSpec::from_json(&spec.to_string()).unwrap(),
        Path::new("/"),
    )
    .unwrap();
    // the import's record digests are the ones the readers compute (the binding of a certificate)
    for (sha, k) in &rels {
        assert_eq!(
            set.supplied_info["symmetry"][k].record_sha256, *sha,
            "{k:?}"
        );
    }
    let base = {
        let ev = Evidence::new(&set);
        check(&set, &ev, &CheckOptions::default()).unwrap()
    };
    let mut certs = SymmetryCertificates::default();
    certs.certify(&rels[0].0, rels[0].1.clone(), "c0".into());
    certs.certify(&rels[1].0, rels[1].1.clone(), "c1".into());
    certs.refuse(
        &rels[2].0,
        rels[2].1.clone(),
        "EXACT_NOT_IN_SPAN_AT_KEY".into(),
    );
    // a certificate of ANOTHER record at the same key never applies
    certs.certify(&"0".repeat(64), rels[3].1.clone(), "c3".into());
    // a refusal wins over a certificate of the same pair
    certs.certify(&rels[2].0, rels[2].1.clone(), "late".into());
    let mut ev = Evidence::new(&set);
    ev.symmetry_certificates = Some(certs);
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    let row = |res: &import::CheckResult, k: &[i32]| {
        let r = res.rows.iter().find(|r| r.key == k).unwrap();
        (r.status, r.evidence, r.basis)
    };
    for (i, (_, k)) in rels.iter().enumerate() {
        let want = match i {
            0 | 1 => (
                RowStatus::Served,
                Some(Level::PerKey),
                Some("symmetry_relation_kernel_certified"),
            ),
            2 => (
                RowStatus::Served,
                Some(Level::Imported),
                Some("symmetry_relation_kernel_refused"),
            ),
            _ => (
                RowStatus::Served,
                Some(Level::PerKey),
                Some("symmetry_relation"),
            ),
        };
        assert_eq!(row(&res, k), want, "relation {i} {k:?}");
    }
    // no other row changed (status, level, basis), apart from the three keys above
    for r in &base.rows {
        if rels[..3].iter().any(|(_, k)| *k == r.key) {
            continue;
        }
        assert_eq!(
            row(&res, &r.key),
            (r.status, r.evidence, r.basis),
            "{:?}",
            r.key
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn readers_on_the_producers_records() {
    let s = scratch();
    let f8 = s.join("tworow/t178f8/f8b.jsonl");
    if let Ok(t) = std::fs::read_to_string(&f8) {
        let e = producers::tworow(&t).unwrap();
        assert_eq!(e.len(), 79, "tworow F8 served keys");
        let terms: usize = e.iter().map(|x| x.rhs.len()).sum();
        println!(
            "tworow F8: {} keys, {terms} rhs terms (best per key)",
            e.len()
        );
        assert_eq!(terms, 37_228, "the lane's total of the best relations");
    } else {
        println!("SKIP tworow: {} missing", f8.display());
    }
    let r1 = s.join("repairflow/runs/t162v3-r1b");
    if let (Ok(t), Ok(v)) = (
        std::fs::read_to_string(r1.join("composite.table.jsonl")),
        std::fs::read_to_string(r1.join("composite.verification.jsonl")),
    ) {
        let c = producers::composites(&t, &v).unwrap();
        assert_eq!(c.len(), 6_297);
        assert_eq!(c.iter().filter(|x| !x.plain).count(), 117);
    } else {
        println!("SKIP composites T162");
    }
    let acc = s.join("symjob/real/accepted");
    if let (Ok(t), Ok(v)) = (
        std::fs::read_to_string(acc.join("symcomp.f424a5b69c733d41.table.jsonl")),
        std::fs::read_to_string(acc.join("symcomp.b0d774069205d52a.verification.jsonl")),
    ) {
        let c = producers::composites(&t, &v).unwrap();
        assert_eq!(c.iter().filter(|x| !x.plain).count(), 44);
    } else {
        println!("SKIP composites T178");
    }
}
