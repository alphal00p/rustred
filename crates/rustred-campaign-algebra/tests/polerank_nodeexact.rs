//! Lane tadicjob (interfaces/tadic-job.md 3, the second route at node level): our own EXACT composition of a candidate
//! table (polerank::nodeexact) against EVERY residue of a node-level t-adic replay result; a corrupted residue is
//! caught. Fixtures: RUSTRED_CAMPAIGN_FIXTURES/tadicjob (SHA256SUMS; default the implementer scratch path); skips
//! without them or without the Symbolica licence. Test data only (L8).

mod inputs;
use rustred_campaign_algebra::polerank::nodeexact::{NodeTable, compare};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    let base = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures".into()
    });
    let d = PathBuf::from(base).join("tadicjob");
    d.join("SHA256SUMS").exists().then_some(d)
}

#[test]
fn exact_composition_equals_every_replayed_residue() {
    if !licensed() {
        inputs::missing(format_args!("SKIP: Symbolica licence absent"));
        return;
    }
    let Some(d) = dir() else {
        inputs::missing(format_args!("SKIP: fixtures tadicjob absent"));
        return;
    };
    let alg = SymbolicaAlgebra::new().unwrap();
    for (name, want) in [("TOY2kite", 314u64), ("L3a", 1706)] {
        let table = NodeTable::load(&d.join(format!("{name}.table.jsonl")), None).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(d.join(format!("{name}.result.json"))).unwrap())
                .unwrap();
        let (n, bad) = compare(&alg, &table, &result, "4").unwrap();
        assert_eq!(n, want, "{name}: residues compared");
        assert!(bad.is_empty(), "{name}: {bad:?}");
        // a corrupted residue is a mismatch
        let mut r2 = result.clone();
        let p: u128 = r2["requests"][0]["terms"][0]["per_prime"][0]["prime"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let cs = &mut r2["requests"][0]["terms"][0]["per_prime"][0]["coefficients"];
        if let Some(c) = cs.as_array_mut().and_then(|a| a.first_mut()) {
            let x: u128 = c.as_str().unwrap().parse().unwrap();
            *c = serde_json::json!(((x + 1) % p).to_string());
            let (_, bad) = compare(&alg, &table, &r2, "4").unwrap();
            assert_eq!(bad.len(), 1, "{name}: the corrupted residue");
        }
    }
}
