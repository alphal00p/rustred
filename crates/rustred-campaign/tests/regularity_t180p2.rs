//! Lane gencheck round 4, REVERSED in round 7 by the user ruling of 2026-11-19 (WS/ops/LANE_RULES.md rule 21: d = 4
//! regularity is a REPORTED NUMBER and a tie-break only). The OUTSIDE REVIEW's witness
//! (astra_5L/review-20261119/T180p2-v5/regularity/WITNESS.json) as a TEST: the emitted replacement of T180p2 position 670
//! (v5 position 672, lane rulecost's r2/670 piece) is a true identity that turns first-match keys REGULAR at d = 4 into
//! SINGULAR ones. Under the old rule its acceptance refused it; now it is ACCEPTED and the regression is REPORTED exactly
//! (495 of 495 served keys regular -> singular; the review's 315 simple-pole daughters at its witness key).
//!
//! Real data (read only): RUSTRED_REG_T180P2 = lane rulecost's scratch directory (t180min/F.min.table.jsonl = the old
//! table, r2/670/run/pieces.jsonl = the replacement). The test SKIPS with a message when it is not set.
use rustred_campaign::emission::regcheck::{Options, acceptance, read_line_at};
use rustred_campaign_core::emission::body::{sha256_hex, strip_derivation};
use rustred_campaign_core::emission::regularity::{Relation, daughter_poles, regularity_at};
use std::path::PathBuf;

const ENV: &str = "RUSTRED_REG_T180P2";

#[test]
fn review_witness_670_to_672_is_accepted_and_its_regression_reported() {
    let Ok(root) = std::env::var(ENV) else {
        eprintln!("SKIP: {ENV} not set (lane rulecost's scratch directory)");
        return;
    };
    let root = PathBuf::from(root);
    let table = root.join("t180min/F.min.table.jsonl");
    let piece_file = root.join("r2/670/run/pieces.jsonl");
    // the very bytes the review measured
    let raw = std::fs::read(&piece_file).unwrap();
    assert_eq!(
        sha256_hex(&raw),
        "9289f96cdd1c16538c03c27ec2fc61af71a74cbf750c4a361665db0293a201fa",
        "the replacement is the review's new 672"
    );
    let old_line = read_line_at(&table, 670, None).unwrap();
    assert_eq!(
        sha256_hex(format!("{old_line}\n").as_bytes()),
        "c68cb0bed0e8e6b5536119d20614811a636e86237868f07601d67afc4ec5834a",
        "the old record is the review's 670"
    );
    let piece_line = String::from_utf8(raw).unwrap().lines().next().unwrap().to_string();
    // 1. the review's witness key: old regular, new singular (residue -1/3 at the witness daughter)
    let old = Relation::parse(&strip_derivation(&old_line).unwrap().0).unwrap();
    let new = Relation::parse(&strip_derivation(&piece_line).unwrap().0).unwrap();
    let parent: Vec<i64> = vec![1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let daughter: Vec<i64> = vec![2, 1, 2, 0, 2, 1, 1, 1, 2, 1, 1, 0, 0, -1, 0, 0, 0, 0, 0, 0];
    let o = regularity_at(&old, &parent).unwrap();
    assert_eq!(o.max_pole, 0, "old 670 is regular at the witness key: {o:?}");
    let (poles, undefined) = daughter_poles(&new, &parent).unwrap();
    assert!(!undefined);
    assert_eq!(poles.get(&daughter), Some(&1), "the witness daughter has a simple pole at d = 4");
    let simple = poles.values().filter(|&&p| p == 1).count();
    let singular = poles.values().filter(|&&p| p > 0).count();
    eprintln!("witness key: new relation has {singular} singular merged daughters ({simple} simple poles)");
    assert_eq!(simple, 315, "the review counts 315 merged daughters with a simple pole");
    // 2. THE MEASURE over the old record's first-match keys in the cone: ACCEPTED, the regression REPORTED
    let tmp = std::env::temp_dir().join(format!("reg670-{}", std::process::id()));
    let ot = rustred_campaign::emission::regcheck::OldTable::load(&table).unwrap();
    let a = acceptance(&ot, 670, &old_line, &[piece_line], &Options::default(), &tmp).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(a.accepted, "rule 21: d = 4 regularity refuses nothing");
    let v = a.verdict.expect("the acceptance carries its regularity verdict");
    eprintln!("{} | {:?}", serde_json::to_string(&v.counts).unwrap(), a.reason);
    assert_eq!((v.counts.keys, v.counts.regular_to_singular, v.counts.worse), (495, 495, 495), "the regression, reported exactly");
    assert!(!v.not_worse && !v.first_regressions.is_empty());
    assert!(a.reason.unwrap_or_default().starts_with("regularity_d4_reported"));
    assert_eq!(v.schema, rustred_campaign_core::emission::regularity::SCHEMA);
    assert!(v.role.contains("tie-break only"));
}
