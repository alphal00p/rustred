//! lane symfix (audit r2-sym F3 / F4): supervisor-side pieces of the kernel SECOND ROUTE. Pure (no process).
//! - the import's relation digest hashes EXACTLY the contract's canonical form of the relation object (so a producer
//!   computing `relation_sha256` = SHA-256 of `canon::canonical_line({"schema","key","rhs"})` agrees with the import);
//! - `composite_second_route` decides from the engine directory's manifest + SHA256SUMS alone (no child process).

use rustred_campaign::adapters::symcomp::{
    CompositeRoute, SYMROWS_FEATURES, composite_second_route,
};
use rustred_campaign_contract::canon::canonical_line;
use rustred_campaign_core::import::supplied_verify::{relation_canonical_text, relation_sha256};
use serde_json::json;

fn sha256(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

#[test]
fn the_relation_digest_hashes_the_contracts_canonical_form() {
    let cases: Vec<(Vec<i32>, Vec<(Vec<i32>, String)>)> = vec![
        (
            vec![2, 1, 1, 0, -1],
            vec![
                (vec![1, 1, 1, 0, -1], "-1/2".into()),
                (vec![1, 0, 1, 0, 0], "(d-4)/(2*d-3)".into()),
                (vec![1, 1, 1, -1, 0], "7".into()),
            ],
        ),
        (vec![1, 1], vec![(vec![1, 0], "1".into())]),
    ];
    for (key, rhs) in cases {
        let mut r = rhs.clone();
        r.sort_by(|a, b| a.0.cmp(&b.0));
        // the object exactly as lane symcert's contract writes it (proof-symrows-job.md 2), members in any order
        let v = json!({"schema": "rustred.campaign.symrows-relation.v1", "key": key,
            "rhs": r.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>()});
        let canon = canonical_line(v.to_string().as_bytes()).unwrap();
        let mine = relation_canonical_text(&key, &rhs).unwrap();
        assert_eq!(String::from_utf8(canon.clone()).unwrap(), mine);
        assert_eq!(sha256(&canon), relation_sha256(&key, &rhs).unwrap());
    }
}

#[test]
fn the_second_route_is_decided_from_the_manifest_and_sha256sums_only() {
    let d = std::env::temp_dir().join(format!("symfix-route-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let bin = b"not a real kernel: never executed";
    std::fs::write(d.join("rr_proof"), bin).unwrap();
    let same = |r: &CompositeRoute| matches!(r, CompositeRoute::SameBackend { .. });
    // no manifest
    assert!(same(&composite_second_route(&d, "rr_proof")));
    // a manifest without the feature (today's frozen engine manifests)
    std::fs::write(
        d.join("version.json"),
        json!({"program":"rr_node","features":["job","symmetry_rows"]}).to_string(),
    )
    .unwrap();
    assert!(same(&composite_second_route(&d, "rr_proof")));
    // the feature, but no SHA256SUMS entry
    std::fs::write(
        d.join("version.json"),
        json!({"program":"rr_node","features":["job", SYMROWS_FEATURES[0]]}).to_string(),
    )
    .unwrap();
    assert!(same(&composite_second_route(&d, "rr_proof")));
    // a wrong digest
    std::fs::write(
        d.join("SHA256SUMS"),
        format!("{}  rr_proof\n", "0".repeat(64)),
    )
    .unwrap();
    assert!(same(&composite_second_route(&d, "rr_proof")));
    // the listed digest of the file as it is: the kernel route, its sha256 = the import's allow-list entry
    std::fs::write(d.join("SHA256SUMS"), format!("{}  rr_proof\n", sha256(bin))).unwrap();
    assert_eq!(
        composite_second_route(&d, "rr_proof"),
        CompositeRoute::Kernel {
            program: d.join("rr_proof"),
            sha256: sha256(bin)
        }
    );
    let _ = std::fs::remove_dir_all(&d);
}
