//! The ETA estimator on synthetic histories: remaining items by level x quantile cost per level / live
//! parallelism; pooled fallback; null with a reason when there is too little data or no parallelism.
mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{Event, Ledger, LedgerConfig};
use rustred_campaign::progress::eta;
use rustred_campaign::state::{EvidenceLevel, Rung};

/// `done[r]` = CPU costs (ms) of finished items at rung r; `open[r]` = open items at rung r.
fn history(tag: &str, done: &[(u8, Vec<u64>)], open: &[(u8, usize)]) -> (Ledger, Cleanup) {
    let d = scratch(tag);
    let c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    let mut k = 0;
    for (r, costs) in done {
        for &ms in costs {
            l.append_all(item_life("T162", k, *r, EvidenceLevel::IdentityTested, ms))
                .unwrap();
            k += 1;
        }
    }
    for (r, n) in open {
        for _ in 0..*n {
            l.append(Event::ItemCreated {
                item: item_id("T162", k),
                node: "T162".into(),
                kind: "face".into(),
                rung: Rung(*r),
                parent: None,
                inputs: vec![],
                label: "x".into(),
                weight: 1,
            })
            .unwrap();
            k += 1;
        }
    }
    (l, c)
}

#[test]
fn too_little_data_gives_null_with_a_reason() {
    let (l, _c) = history("eta0", &[(0, vec![1000, 2000])], &[(0, 10)]);
    let e = eta(l.state(), Some(4));
    assert!(e["wall_seconds"].is_null() && e["cpu_seconds"].is_null());
    assert!(
        e["reason"].as_str().unwrap().contains("too little data"),
        "{e}"
    );
    assert_eq!(e["remaining_items"], 10);
}

#[test]
fn quartile_range_per_level_divided_by_parallelism() {
    // L0: 8 finished jobs 1..8 s -> p25 2 s, p50 4 s, p75 6 s; 20 open L0 items; 4 live jobs
    let (l, _c) = history(
        "eta1",
        &[(0, (1..=8).map(|s| s * 1000).collect())],
        &[(0, 20)],
    );
    let e = eta(l.state(), Some(4));
    assert_eq!(e["cpu_seconds"]["low"], 40.0);
    assert_eq!(e["cpu_seconds"]["mid"], 80.0);
    assert_eq!(e["cpu_seconds"]["high"], 120.0);
    assert_eq!(e["wall_seconds"]["low"], 10.0);
    assert_eq!(e["wall_seconds"]["mid"], 20.0);
    assert_eq!(e["wall_seconds"]["high"], 30.0);
    assert_eq!(e["parallelism"], 4);
    assert!(e["reason"].is_null());
    assert_eq!(e["by_level"]["L0"]["p50_s"], 4.0);
}

#[test]
fn sparse_level_uses_pooled_samples_and_says_so() {
    // L0 has 6 samples of 10 s; L2 has 1 sample of 100 s (< 5): L2's open items use the pooled quantiles
    let (l, _c) = history(
        "eta2",
        &[(0, vec![10_000; 6]), (2, vec![100_000])],
        &[(0, 2), (2, 3)],
    );
    let e = eta(l.state(), Some(1));
    assert_eq!(e["pooled_levels"], serde_json::json!(["L2"]));
    // pooled sorted: [10 x6, 100]: p25 = 10, p50 = 10, p75 = 10 (nearest rank 6 of 7) -> 5 items x 10 s
    assert_eq!(e["cpu_seconds"]["mid"], 50.0);
    assert_eq!(e["cpu_seconds"]["high"], 50.0);
}

#[test]
fn no_parallelism_and_nothing_left() {
    let (l, _c) = history("eta3", &[(0, vec![1000; 5])], &[(0, 3)]);
    let e = eta(l.state(), None);
    assert!(e["wall_seconds"].is_null());
    assert_eq!(e["cpu_seconds"]["mid"], 3.0);
    assert!(
        e["reason"]
            .as_str()
            .unwrap()
            .contains("no live parallelism")
    );
    let (l, _c) = history("eta4", &[(0, vec![1000; 5])], &[]);
    let e = eta(l.state(), Some(2));
    assert_eq!(e["remaining_items"], 0);
    assert_eq!(e["reason"], "nothing left");
}
