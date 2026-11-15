//! lane symfix: REAL-DATA driver (ignored; run by hand as its own unit): load a node's import spec, optionally with one
//! extra `supplied_relations` source (a shipped table + its verification file, pinned here), and report how the import
//! verified every supplied relation AT LOAD (interfaces/import.md 21): routes, counts, typed reasons, load time.
//! Env: SYMFIX_REAL_SPEC (spec path), SYMFIX_REAL_BASE (default "/"), SYMFIX_REAL_ADD_TABLE + SYMFIX_REAL_ADD_VERIFICATION
//! (optional extra source), SYMFIX_REAL_OUT (summary JSON path), SYMFIX_REAL_EXPECT_ALL (=1: every supplied relation of a
//! source with a verification file must be verified).

use rustred_campaign_core::import::supplied_verify::SuppliedRoute;
use rustred_campaign_core::import::{TableSetSpec, load};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn sha256_file(p: &str) -> String {
    let b = std::fs::read(p).unwrap();
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&b);
    h.finish_hex()
}

#[test]
#[ignore = "real data; run by hand: SYMFIX_REAL_SPEC=<spec> (module documentation)"]
fn real_spec_supplied_relations_are_verified_at_load() {
    let spec_path = std::env::var("SYMFIX_REAL_SPEC").expect("SYMFIX_REAL_SPEC");
    let base = PathBuf::from(std::env::var("SYMFIX_REAL_BASE").unwrap_or_else(|_| "/".into()));
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&spec_path).unwrap()).unwrap();
    if let (Ok(t), Ok(ver)) = (
        std::env::var("SYMFIX_REAL_ADD_TABLE"),
        std::env::var("SYMFIX_REAL_ADD_VERIFICATION"),
    ) {
        let srcs = v["sources"].as_array_mut().unwrap();
        let at = srcs
            .iter()
            .position(|s| s["id"] == "default-recipe")
            .unwrap_or(srcs.len());
        srcs.insert(
            at,
            json!({"id": "symfix:real", "kind": "supplied_relations", "class": "symmetry-composite",
                "path": t, "sha256": sha256_file(&t),
                "verification": {"path": ver, "sha256": sha256_file(&ver)}}),
        );
    }
    let spec = TableSetSpec::from_json(&v.to_string()).unwrap();
    let t0 = std::time::Instant::now();
    let set = load(&spec, &base).unwrap();
    let load_s = t0.elapsed().as_secs_f64();
    let mut out = json!({"spec": spec_path, "load_s": load_s, "sources": []});
    let mut all_ok = true;
    for s in set
        .report
        .sources
        .iter()
        .filter(|s| s.kind == "supplied_relations")
    {
        let info = set.supplied_info.get(&s.id).cloned().unwrap_or_default();
        let mut routes: BTreeMap<String, u64> = BTreeMap::new();
        let mut reasons: BTreeMap<String, u64> = BTreeMap::new();
        let mut samples: Vec<Value> = vec![];
        for (k, i) in &info {
            let w = match &i.route {
                SuppliedRoute::KernelCertified { .. } => "kernel_certified".to_string(),
                SuppliedRoute::GroupRederived => "group_rederived".to_string(),
                SuppliedRoute::SameBackend => "same_backend".to_string(),
                SuppliedRoute::Diagnostic => "diagnostic".to_string(),
                SuppliedRoute::Unverified { reason } => {
                    let c = reason.split(':').next().unwrap_or("").to_string();
                    *reasons.entry(c).or_default() += 1;
                    if samples.len() < 5 {
                        samples.push(json!({"key": k, "reason": reason}));
                    }
                    "unverified".to_string()
                }
            };
            *routes.entry(w).or_default() += 1;
        }
        let has_ver = spec
            .sources
            .iter()
            .any(|x| x.id == s.id && x.verification.is_some());
        if has_ver && routes.get("unverified").copied().unwrap_or(0) > 0 {
            all_ok = false;
        }
        let row = json!({"id": s.id, "relations": s.relations, "relations_verified": s.relations_verified,
            "relations_same_backend": s.relations_same_backend,
            "relations_kernel_certified": s.relations_kernel_certified,
            "relations_not_verified": s.relations_not_verified.len(), "routes": routes, "reasons": reasons,
            "samples": samples});
        println!("{row}");
        out["sources"].as_array_mut().unwrap().push(row);
    }
    println!("load {load_s:.1} s");
    if let Ok(p) = std::env::var("SYMFIX_REAL_OUT") {
        std::fs::write(p, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }
    if std::env::var("SYMFIX_REAL_EXPECT_ALL").as_deref() == Ok("1") {
        assert!(
            all_ok,
            "a supplied relation with a verification file was not verified: {out:#}"
        );
    }
}
