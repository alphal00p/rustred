//! Lane import2: VERIFIED symmetry in the import (interfaces/import.md section 13). The chart's group is derived from
//! the family file and proved exactly (replay_base::symrows), supplied group data is checked against it; SUPPLIED
//! SYMMETRY RELATIONS are verified by re-deriving them from the verified group at the key (exact span of the expanded
//! symmetry rows; rational arithmetic through `CheckAlgebra`, here the core's certificate backend `Independent`; the
//! Symbolica backend runs the same function in crate rustred-campaign-algebra). Test data: the handoff family files
//! (read-only), T178's 4 supplied relations, T162's I(1^9; n9 = -1) = I(1^9) + I(D4 = 0); planted wrong relations.

use rustred_campaign_core::import;
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{DeriveOptions, verify_derived};

use std::path::{Path, PathBuf};

use import::lines::parse_key_text;
use import::symmetry::{RelationCheck, presented_family_text, verify_relation_with};
use import::{CheckOptions, Evidence, Level, RowStatus, TableSetSpec, check, load};
use serde_json::{Value, json};

/// lane closeloop4 (audit-closeloop M1 / m4): the spec text with its evidence PINNED (each file's sha256, a
/// SHA256SUMS manifest beside a directory) and the family of its tables named (spec evidence is read only so).
#[allow(dead_code)]
fn pin_ev(spec: &serde_json::Value) -> String {
    use std::io::BufRead;
    let mut v = spec.clone();
    let fam = v["sources"].as_array().and_then(|a| {
        a.iter().find_map(|s| {
            let p = s["path"].as_str()?;
            let f = std::fs::File::open(p).ok()?;
            let mut l = String::new();
            std::io::BufReader::new(f).read_line(&mut l).ok()?;
            let h: serde_json::Value = serde_json::from_str(l.trim()).ok()?;
            h["family"].as_str().map(str::to_string)
        })
    });
    let sha = |p: &std::path::Path| rustred_campaign_core::import::lines::file_sha256(p).unwrap();
    if let Some(ev) = v.get_mut("evidence").and_then(|e| e.as_object_mut()) {
        for k in ["facts", "proofs", "solved"] {
            if let Some(a) = ev.get_mut(k).and_then(|x| x.as_array_mut()) {
                for e in a {
                    let p = std::path::PathBuf::from(e["path"].as_str().unwrap_or(""));
                    if p.is_file() && e.get("sha256").is_none() {
                        e["sha256"] = serde_json::json!(sha(&p));
                    }
                    if p.is_dir() && e.get("manifest").is_none() {
                        let mut names: Vec<String> = std::fs::read_dir(&p)
                            .unwrap()
                            .flatten()
                            .map(|x| x.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.ends_with(".json") || n.ends_with(".jsonl"))
                            .collect();
                        names.sort();
                        let body: String = names
                            .iter()
                            .map(|n| format!("{}  {n}\n", sha(&p.join(n))))
                            .collect();
                        let m = std::env::temp_dir().join(format!(
                            "pin-ev-{}-{}.SHA256SUMS",
                            std::process::id(),
                            rustred_campaign_core::closure::sha256::sha256_tag(
                                p.display().to_string().as_bytes()
                            )
                        ));
                        std::fs::write(&m, body).unwrap();
                        e["manifest"] =
                            serde_json::json!({"path": m.display().to_string(), "sha256": sha(&m)});
                    }
                    if e.get("family").is_none()
                        && let Some(f) = &fam
                    {
                        e["family"] = serde_json::json!(f);
                    }
                }
            }
        }
    }
    v.to_string()
}

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}
fn ho() -> PathBuf {
    ws().join("handoff/sym-investigation-for-ben")
}

fn key(t: &str, n: usize) -> Vec<i32> {
    parse_key_text(t, n).unwrap()
}

#[test]
fn presented_family_text_reorders_props_and_relabels() {
    let src = "I(\n  name(X),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1-k2)^2,0),\n  gram(q,q,1)\n)";
    let t = presented_family_text(src, &[1, 0, 2]).unwrap();
    assert_eq!(
        t,
        "I(\n  name(X),\n  prop(D1,(k2)^2,1),\n  prop(D2,(k1)^2,1),\n  prop(D3,(k1-k2)^2,0),\n  gram(q,q,1)\n)"
    );
    assert!(presented_family_text(src, &[0, 0, 2]).is_err());
    assert!(presented_family_text(src, &[0, 1]).is_err());
}

/// T162: the one-row relation of the helper's input (section 6) re-derived from the verified group; planted variants
/// (a wrong coefficient, a missing term) are NOT in the span.
#[test]
fn t162_symmetry_relation_n9_verified_and_planted_ones_refused() {
    let p = ho().join("t162-bank/family/T162-presented-P3.sym");
    let Ok(src) = std::fs::read_to_string(&p) else {
        println!("SKIP: {} missing", p.display());
        return;
    };
    let fam = Family::parse(&src).unwrap();
    let n = fam.n();
    let chart: Vec<bool> = (0..n).map(|i| i < 9).collect();
    let vg = verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap();
    let target = key("I(1,1,1,1,1,1,1,1,1,-1,0,0,0,0,0,0,0,0,0,0)", n);
    let good = vec![
        (
            key("I(1,1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0)", n),
            "1".to_string(),
        ),
        (
            key("I(1,1,1,0,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0)", n),
            "1".to_string(),
        ),
    ];
    let c = verify_relation_with(&Independent, &vg, &target, &good, &[], 1 << 16);
    println!(
        "T162 I(1^9; n9=-1) = I(1^9) + I(D4=0): {c:?} (group: {} elements)",
        vg.group().elements.len()
    );
    assert!(c.verified(), "{c:?}");
    let mut wrong = good.clone();
    wrong[1].1 = "2".into();
    let c = verify_relation_with(&Independent, &vg, &target, &wrong, &[], 1 << 16);
    assert!(matches!(c, RelationCheck::NotInSpan { .. }), "{c:?}");
    let c = verify_relation_with(&Independent, &vg, &target, &good[..1], &[], 1 << 16);
    assert!(matches!(c, RelationCheck::NotInSpan { .. }), "{c:?}");
}

/// T178: the import's spec with the 4 supplied symmetry relations (3 one-term, 1 multi-term: I(1^10; D12 = -1) =
/// 3/2 corner + 1/4 of four subsector corners) and `verify` (the group derived from the family file): every relation
/// is VERIFIED, the keys are served at level `per-key` (basis `symmetry_relation`); a planted wrong relation is NOT
/// verified and its key does NOT serve (open, `recipe_unverified`); credits from the verified group are strict.
#[test]
fn t178_supplied_symmetry_relations_verified_by_rederivation() {
    let ws = ws();
    let fam = ho().join("families/T178.sym");
    let sym = ws.join("SCRATCHPAD/t178-close/data/symrules.table.jsonl");
    let piece = ws.join("SCRATCHPAD/t178/regenB/R0/try1/node/F.table.jsonl");
    if !fam.is_file() || !sym.is_file() || !piece.is_file() {
        println!("SKIP: T178 family / relations / frame table missing");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("import-sym-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // start: the 4 relation targets
    let targets: Vec<Vec<i32>> = std::fs::read_to_string(&sym)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "rule")
        .map(|v| key(v["target"].as_str().unwrap(), 20))
        .collect();
    assert_eq!(targets.len(), 4);
    let start = tmp.join("start.jsonl");
    std::fs::write(
        &start,
        targets
            .iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let spec_for = |rel: &Path| {
        json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"R0","required_evidence":"per-key",
            "start":{"path": start.display().to_string(),"format":"keys_jsonl"},
            "sources":[
                {"id":"symmetry","kind":"supplied_relations","class":"symmetry","path": rel.display().to_string(),
                 "verify":{"family":{"path": fam.display().to_string(),
                     "sha256":"eb34c9dedbe35b752f6b085f83ca4a6b63ff8fc9a5a7c46f18ee46d9d76cc608"}}},
                {"id":"R0","kind":"engine_table","path": piece.display().to_string(),"records":[0]},
                {"id":"credits","kind":"symmetry_credit","family":{"path": fam.display().to_string()}}]})
    };
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&pin_ev(&spec_for(&sym))).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let rep = set
        .report
        .sources
        .iter()
        .find(|s| s.id == "symmetry")
        .unwrap();
    println!(
        "T178 supplied relations: verified {} not verified {:?}; group {:?} ({:.2}s)",
        rep.relations_verified,
        rep.relations_not_verified,
        rep.group,
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(rep.relations_verified, 4);
    assert!(rep.relations_not_verified.is_empty());
    let cred = set
        .report
        .sources
        .iter()
        .find(|s| s.id == "credits")
        .unwrap();
    assert!(
        cred.group
            .as_ref()
            .is_some_and(|g| g.provenance == "Derived" && g.elements == 7)
    );
    let ev = Evidence::new(&set);
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    for k in &targets {
        let r = res.rows.iter().find(|r| &r.key == k).unwrap();
        assert_eq!(
            (r.status, r.evidence, r.basis),
            (
                RowStatus::Served,
                Some(Level::PerKey),
                Some("symmetry_relation")
            ),
            "{k:?}"
        );
    }
    // a verified group never makes the closure diagnostic
    assert!(
        !res.summary
            .relaxed_checks
            .iter()
            .any(|c| c.contains("credits"))
    );
    // planted: the multi-term relation with 3/2 -> 5/4
    let bad = tmp.join("symrules.bad.jsonl");
    let txt = std::fs::read_to_string(&sym)
        .unwrap()
        .replace("\"3/2\"", "\"5/4\"");
    std::fs::write(&bad, txt).unwrap();
    let spec = TableSetSpec::from_json(&pin_ev(&spec_for(&bad))).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let rep = set
        .report
        .sources
        .iter()
        .find(|s| s.id == "symmetry")
        .unwrap();
    assert_eq!(rep.relations_verified, 3);
    assert_eq!(rep.relations_not_verified.len(), 1);
    let ev = Evidence::new(&set);
    let res = check(&set, &ev, &CheckOptions::default()).unwrap();
    let k = key("I(1,1,1,1,1,1,1,1,1,1,0,-1,0,0,0,0,0,0,0,0)", 20);
    let r = res.rows.iter().find(|r| r.key == k).unwrap();
    assert_ne!(
        r.status,
        RowStatus::Served,
        "a relation that is not verified must not serve"
    );
    println!("planted relation: {:?} {:?}", r.open_status, r.open_reason);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Lane import3: PER-SLOT numerator purity of a verified credit element. The T266 group is derived from the jointest9
/// table header's presented family (header line only). The independent checker credits I(1,1,1,2,1^10,-8,0^5) by the
/// element mapping it to I(2,1^13,-8,0^5) (its group element 7: numerator slot 14 pure, the others not). Our policy
/// refused it (element-level purity); with per-slot purity the same image is produced exactly while the element stays
/// out of `pure_numerator_maps`, and a key with a numerator on a non-pure slot is still refused.
#[test]
fn t266_per_slot_numerator_purity_reproduces_the_checkers_image() {
    use import::symmetry::{GroupFormat, GroupSpec, load_verified_group};
    use rustred_campaign_core::order::NodeComparator;
    use rustred_campaign_core::symmetry::CreditPolicy;
    let table = ws().join("SCRATCHPAD/t266/jointest9/F.table.jsonl");
    if !table.is_file() {
        println!("SKIP: {} missing", table.display());
        return;
    }
    let h = import::lines::read_header("jointest9", &table).unwrap();
    let frame = rustred_campaign_core::key::NodeFrame::from_header(&h).unwrap();
    let cmp = NodeComparator::from_header(&h.raw).unwrap();
    let gs = GroupSpec {
        family: None,
        group: None,
        group_format: GroupFormat::Derive,
        max_candidates: None,
        max_terms: None,
    };
    let (vg, rep) = load_verified_group(
        &gs,
        Path::new("/"),
        frame.mask(),
        cmp.presentation().new_to_old(),
        h.raw.get("family_source").and_then(Value::as_str),
    )
    .unwrap();
    let pol = CreditPolicy::from_verified(&vg, frame.mask()).unwrap();
    let k = key("I(1,1,1,2,1,1,1,1,1,1,1,1,1,1,-8,0,0,0,0,0)", 20);
    let want = key("I(2,1,1,1,1,1,1,1,1,1,1,1,1,1,-8,0,0,0,0,0)", 20);
    let hits: Vec<usize> = (0..pol.group().len())
        .filter(|&e| pol.partial_image(e, &k, frame.mask()).as_deref() == Some(&want[..]))
        .collect();
    println!(
        "T266 group {} elements ({:?}); fully pure numerator maps {:?}; elements imaging the key onto the checker's image {hits:?}",
        pol.group().len(),
        rep.provenance,
        pol.pure_numerator_maps()
    );
    assert!(
        !hits.is_empty(),
        "per-slot purity gives the checker's image"
    );
    assert!(hits.iter().all(|e| !pol.pure_numerator_maps().contains(e)));
    // a numerator on a slot that element does not map purely is still refused
    let e = hits[0];
    let bad: Vec<usize> = (14..20)
        .filter(|&s| {
            let mut kk = k.clone();
            kk[14] = 0;
            kk[s] = -1;
            pol.partial_image(e, &kk, frame.mask()).is_none()
        })
        .collect();
    println!("element {e}: numerator slots without a one-term image: {bad:?}");
    assert!(!bad.is_empty());
}
