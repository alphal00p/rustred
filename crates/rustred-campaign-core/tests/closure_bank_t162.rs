//! T162 bank as engine-loadable per-origin tables (coord-bank-tables.md; coordinator addendum 4): forward closure of
//! box B (55,704 keys) under first match over the 56 `qualified` tables in DISPATCH-PRIORITY order, witness check ON.
//! (a) Tier K, witness policy `block` (reported): every key served by a witness-absent bank record is blocked and
//! "requires regularity fact"; (b) the lift ASSUMED granted (test-only fact source: the lift fact and regularity
//! `Regular` everywhere, coefficients unknown, so the reach equals the Tier K superset): reached keys, used rules,
//! edges, descent. NEW reference numbers (bank only, Tier K superset; the coordinator's coefficient-level 65,018 keys
//! need bank + batch-05 and zero tests of 1.97 M positive-numerator and 9,053 zero-coefficient top terms: not this
//! test). `#[ignore]`: `light_build.sh <lane> test -p rustred-campaign-core --release --test closure_bank_t162 --
//! --ignored --nocapture`.

mod closure_common;
mod inputs;

use std::path::PathBuf;

use closure_common::k3_common;
use rustred_campaign_core::closure::{
    ClosureConfig, CoefFact, FactSource, HandDowns, KeyCodec, KeySetBuilder, NoFacts, RegFact,
    RuleBodies, RuleId, Tier, Verdict, closure,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};

const LARGE: &str = "/common/build-cache/vhirschi/p_integral_dev/bank-conv/20261114";
const PRIO: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/bank-conv/out/20261114/DISPATCH-PRIORITY.json";

/// TEST-ONLY: the witness lift granted everywhere (regular, in case, no exception branch), coefficients unknown.
struct LiftAll;
impl FactSource for LiftAll {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        CoefFact::Unknown
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Regular
    }
    fn witness_lift(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Regular
    }
    fn label(&self) -> String {
        "test-only:lift-all".into()
    }
}

#[test]
#[ignore = "needs the bank-conv tables; run with --ignored in release"]
fn t162_bank_box_b_closure() {
    let Ok(prio) = std::fs::read_to_string(PRIO) else {
        inputs::missing(format_args!("SKIP: {PRIO} missing"));
        return;
    };
    let prio: serde_json::Value = serde_json::from_str(&prio).unwrap();
    let (mut tables, mut tfs) = (vec![], vec![]);
    let mut header = None;
    for e in prio["qualified"].as_array().unwrap() {
        let p = PathBuf::from(LARGE).join(e["file"].as_str().unwrap());
        let tf = read_table_file(
            &p,
            ReadOptions {
                rhs: RhsMode::KeysOnly,
            },
        )
        .unwrap();
        if header.is_none() {
            header = Some(tf.header.as_ref().unwrap().raw.clone());
        }
        let t = Table::from_table_file(
            e["origin"].as_str().unwrap(),
            p.display().to_string(),
            &tf,
            DomainOptions::default(),
        )
        .unwrap();
        assert_eq!(t.skipped_count(), 0);
        tables.push(t);
        tfs.push(tf);
    }
    let n = tables.len();
    let rules: usize = tables.iter().map(|t| t.len()).sum();
    println!("T162 bank: {n} tables, {rules} rules");
    assert_eq!((n, rules), (56, 2_440));
    let disp = Dispatcher::new(tables, (0..n).collect()).unwrap();
    let mut bodies = RuleBodies::new(&disp);
    for (t, tf) in tfs.iter().enumerate() {
        bodies.load_table_records(&disp, t, tf).unwrap();
    }
    drop(tfs);
    let cmp = NodeComparator::from_header(&header.unwrap()).unwrap();
    let mut b = KeySetBuilder::new(KeyCodec::new(20, 6).unwrap());
    for k in k3_common::t162_box_b() {
        b.push(&k).unwrap();
    }
    let bx = b.finish();
    assert_eq!(bx.len(), 55_704);
    // (a) Tier K, witness policy block
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 4,
        hand_downs: HandDowns::Counts,
        ..Default::default()
    };
    let a_cl = closure(&disp, &bodies, &cmp, &bx, &cfg, &NoFacts).unwrap();
    let a = a_cl.summary();
    println!(
        "T162 bank (a) Tier K, witness_absent block: keys {} open {} = {} + {} by status {:?} requires regularity fact {} used {}",
        a.keys_total,
        a.open_total,
        a.open_in_start,
        a.open_outside,
        a.open_by_status,
        a.requires_regularity_fact,
        a.used_rules
    );
    assert_eq!(a.verdict, Verdict::Open);
    // reference since lane closurefix (re-audit F1b, 2026-11-14): the witness-absent keys stay open (blocked, requiring
    // the per-key regularity fact) but Tier K FOLLOWS their records' bodies (superset of every Tier C run): it reaches
    // the 59,248 keys of the lift-granted run (b) below; 57,928 blocked + 1,318 uncovered open; 936 used rules (= (b)).
    // (Before: 55,704 keys, 54,712 blocked box keys, 1 used rule: the walk stopped at the blocked keys.)
    assert_eq!(
        (
            a.keys_total,
            a.requires_regularity_fact,
            a.open_in_start,
            a.used_rules,
            a.open_followed
        ),
        (59_248, 57_928, 55_702, 936, 57_928)
    );
    // (b) lift granted (test-only facts)
    let cfg_c = ClosureConfig {
        tier: Tier::C,
        table_digests: disp
            .tables()
            .iter()
            .map(|t| (t.name.clone(), "test-only".to_string()))
            .collect(),
        ..cfg.clone()
    };
    let c = closure(&disp, &bodies, &cmp, &bx, &cfg_c, &LiftAll).unwrap();
    let s = c.summary();
    println!(
        "T162 bank (b) lift granted: rounds {} keys {} (box {} + {}) edges {} violations {} cycles {:?} open {} = {} + {} by status {:?} \
         used {} (box {}) lifted {} keys over {} tables, verdict {:?} ({:.1} s)",
        s.rounds,
        s.keys_total,
        s.keys_in_start,
        s.keys_outside,
        s.edges,
        s.descent_violations,
        s.cycles,
        s.open_total,
        s.open_in_start,
        s.open_outside,
        s.open_by_status,
        s.used_rules,
        s.used_rules_in_start,
        s.witness_lifted.values().sum::<u64>(),
        s.witness_lifted.len(),
        s.verdict,
        c.seconds
    );
    assert_eq!(s.descent_violations, 0);
    assert_eq!(s.cycles, Some(0));
    assert_eq!(s.open_in_start, 990, "box B: 990 keys without a bank rule");
    // Tier K (a) is a superset of this Tier C run (re-audit F1)
    assert!(c.keys().difference(&a_cl.keys()).is_empty());
    // NEW reference (2026-11-14, bank only, Tier K reach): 20 rounds, 59,248 keys = 55,704 + 3,544, 954,974 edges,
    // open 1,318 = 990 + 328, 936 used rules (906 on the box = the coordinator's 906)
    assert_eq!(
        (
            s.rounds,
            s.keys_total,
            s.keys_outside,
            s.edges,
            s.open_total,
            s.open_outside,
            s.used_rules,
            s.used_rules_in_start
        ),
        (20, 59_248, 3_544, 954_974, 1_318, 328, 936, 906)
    );
}
