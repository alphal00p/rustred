//! Lane closurefix regressions (2026-11-14), beyond the re-audit reproductions in `audit_closure2_*`:
//! - credits rest only on the identity-test lane's unforgeable `VerifiedGroup` (directing session; ladderfix2 finding
//!   3): `CreditPolicy::from_verified`, any other policy stamps the closure `diagnostic`;
//! - accept-B M7: on a table whose records carry no witness loci, the Tier K closure reports exactly the keys the core
//!   blocks (open, followed), never `superset_clean` with 0 open;
//! - spill (re-audit F5, F9): a run that spills its fact requests and exit key sets to files gives the same result as
//!   the in-memory run; the streaming request writer writes each distinct request once;
//! - recheck under a round limit truncates exactly as the closure of the union (F6b);
//! - comparator frame check (audit loop-A m7) and the arity refusal (F4).
//! Fixture names and numbers are TEST DATA only (L8).

mod accept_common;

use std::collections::{BTreeMap, BTreeSet};

use accept_common::{available, load_node, load_table, slice};
use rustred_campaign_core::closure::{
    ClosureConfig, ClosureError, ClosureSummary, CoefFact, FactSource, HandDowns, KeyCodec,
    KeySet, NoFacts, OpenStatus, RegFact, RuleBodies, RuleId, SpillConfig, Tier, Verdict,
    closure, closure_recheck,
};
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{SlotImage, SymGroup, verify_group};
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement, apply_framed};

// ------------------------------------------------------------------------------------------------ verified groups

/// The policy of a verified group: same elements (no identity), the rows' key action (`b[target] = a[i]`), pure maps
/// exactly where every non-line slot has an exact image, basis `Verified`; every other way to make a policy is
/// unverified and stamps the closure diagnostic.
#[test]
fn credit_policy_only_from_a_verified_group() {
    let mut seen = 0;
    for name in ["TOY2kite", "L3a", "L4a", "T281"] {
        let dir = slice(name);
        let (Ok(fam), Ok(sym)) = (
            std::fs::read_to_string(dir.join("family.sym")),
            std::fs::read_to_string(dir.join("SYMMETRY.json")),
        ) else {
            println!("SKIP {name}");
            continue;
        };
        let fam = Family::parse(&fam).unwrap();
        let sym: serde_json::Value = serde_json::from_str(&sym).unwrap();
        let sg = SymGroup::from_cfix_json(&sym, &fam).unwrap();
        let vg = verify_group(&fam, sg, &format!("{name}/SYMMETRY.json")).unwrap();
        let lines = vg.group().chart.clone();
        let pol = CreditPolicy::from_verified(&vg, &lines).unwrap();
        assert!(pol.is_verified(), "{name}");
        assert_eq!(pol.group().len(), vg.group().elements.len(), "{name}");
        let n = lines.len();
        // a numerator-free key with distinct line values: the policy's image = the rows' action
        let key: Vec<i32> = (0..n)
            .map(|s| if lines[s] { 1 + (s as i32 % 4) } else { 0 })
            .collect();
        for (ei, e) in vg.group().elements.iter().enumerate() {
            let mut want = key.clone();
            for s in (0..n).filter(|&s| lines[s]) {
                let t = match &e.action[s] {
                    SlotImage::Exact { target, flip: false } => *target,
                    SlotImage::Linear { terms, .. } => terms[0].0,
                    a => panic!("{name}: line slot {s} image {a:?}"),
                };
                want[t] = key[s];
            }
            assert_eq!(
                apply_framed(&pol.group()[ei], &key, &lines),
                Some(want),
                "{name} element {ei}"
            );
            let exact_numerators = (0..n).filter(|&s| !lines[s]).all(|s| {
                matches!(&e.action[s], SlotImage::Exact { target, flip: false } if !lines[*target])
            });
            assert_eq!(
                pol.pure_numerator_maps().contains(&ei),
                exact_numerators,
                "{name} element {ei}"
            );
        }
        // forging is impossible: re-declaring pure maps or rebuilding from the elements gives an unverified policy
        assert!(
            !pol.clone()
                .with_pure_numerator_maps((0..pol.group().len()).collect())
                .is_verified()
        );
        assert!(!CreditPolicy::new(pol.group().to_vec()).is_verified());
        // the group's chart must be the node's line mask
        let mut other = lines.clone();
        other[0] = !other[0];
        assert!(CreditPolicy::from_verified(&vg, &other).is_err(), "{name}");
        seen += 1;
        // in a closure: verified -> no relaxation from the group; hand-built -> diagnostic `credits_group_unverified`
        if name == "L3a" && available(name) {
            let node = load_node(name);
            let (tf, t) = load_table(&dir.join("table.jsonl"), name);
            let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
            let mut bodies = RuleBodies::new(&disp);
            bodies.load_table_records(&disp, 0, &tf).unwrap();
            let codec = KeyCodec::new(n, 8).unwrap();
            let start =
                KeySet::from_keys(codec, node.box_keys.iter().map(|k| k.as_slice())).unwrap();
            let run = |p: CreditPolicy| -> ClosureSummary {
                let cfg = ClosureConfig {
                    tier: Tier::K,
                    threads: 2,
                    bits: 8,
                    hand_downs: HandDowns::Off,
                    zero_sectors: Some(node.zero_dropped.clone()),
                    zero_sector_source: Some(node.zero_source.clone()),
                    credits: Some(p),
                    ..Default::default()
                };
                closure(&disp, &bodies, &node.cmp, &start, &cfg, &NoFacts)
                    .unwrap()
                    .summary()
            };
            let sv = run(pol.clone());
            let su = run(CreditPolicy::new(pol.group().to_vec()));
            println!(
                "L3a with the verified group: verdict {:?}, relaxed {:?}; hand-built copy: {:?} {:?}",
                sv.verdict, sv.relaxed_checks, su.verdict, su.relaxed_checks
            );
            assert!(
                !sv.relaxed_checks
                    .contains(&"credits_group_unverified".to_string())
            );
            assert!(
                su.relaxed_checks
                    .contains(&"credits_group_unverified".to_string())
            );
            assert_eq!(su.verdict, Verdict::Diagnostic);
            assert_ne!(sv.config_digest, su.config_digest);
        }
    }
    println!("verified-group policies checked on {seen} nodes");
}

// ------------------------------------------------------------------------------------------------ accept-B M7

/// accept-B M7: the ledger node's table carries no witness loci, so the core blocks (witness not carried) where its
/// records are selected. The Tier K closure must report exactly those box keys open (`blocked`, requiring the per-key
/// regularity fact) and follow their bodies (F1b), never `superset_clean` with 0 open (the audit saw that at 347495de,
/// before the closure's dispatch applied the witness status).
#[test]
fn m7_tier_k_reports_the_core_blocked_keys_on_a_witness_absent_table() {
    let name = "T281";
    if !available(name) {
        println!("SKIP {name}: fixture slice not readable");
        return;
    }
    let node = load_node(name);
    let (tf, t) = load_table(&slice(name).join("table.jsonl"), name);
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut bodies = RuleBodies::new(&disp);
    bodies.load_table_records(&disp, 0, &tf).unwrap();
    let n = node.frame.arity();
    let codec = KeyCodec::new(n, 8).unwrap();
    let start = KeySet::from_keys(codec, node.box_keys.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 2,
        bits: 8,
        hand_downs: HandDowns::Off,
        longest_chain: false,
        zero_sectors: Some(node.zero_dropped.clone()),
        zero_sector_source: Some(node.zero_source.clone()),
        ..Default::default()
    };
    let c = closure(&disp, &bodies, &node.cmp, &start, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    let core_blocked: BTreeSet<Vec<i32>> = node
        .box_keys
        .iter()
        .filter(|k| matches!(disp.first_match(k), Ok(Dispatch::Blocked { .. })))
        .cloned()
        .collect();
    let closure_blocked: BTreeSet<Vec<i32>> = c
        .open_keys()
        .into_iter()
        .filter(|o| o.in_start && o.status == OpenStatus::Blocked)
        .map(|o| o.key)
        .collect();
    println!(
        "{name}: box {} keys, core first_match Blocked {}, closure blocked box keys {}, open {} (followed {}), \
         requires regularity fact {}, verdict {:?} / {:?}, keys {}",
        node.box_keys.len(),
        core_blocked.len(),
        closure_blocked.len(),
        s.open_total,
        s.open_followed,
        s.requires_regularity_fact,
        s.verdict,
        s.verdict_if_not_diagnostic,
        s.keys_total
    );
    assert!(!core_blocked.is_empty(), "the fixture's records carry no loci");
    assert_eq!(closure_blocked, core_blocked);
    assert_ne!(s.verdict, Verdict::SupersetClean);
    assert!(s.open_total >= core_blocked.len() as u64);
    assert!(s.requires_regularity_fact >= core_blocked.len() as u64);
    assert!(s.open_followed >= 1, "Tier K follows witness-absent bodies (F1b)");
}

// ------------------------------------------------------------------------------------------------ spill (F5, F9)

const CHART: [bool; 4] = [true, true, false, false];

struct UnknownFacts;
impl FactSource for UnknownFacts {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        CoefFact::Unknown
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Unknown
    }
    fn label(&self) -> String {
        "closurefix-unknown".into()
    }
}

/// A free rule lowering line 0 or line 1 with hand-downs to both children and a raised numerator (requests), plus a
/// point rule at (3,1,0,0) through which the uncovered (1,3,0,0) is credited (an image request coinciding with the
/// direct request of the reached key (3,1,0,0)).
fn spill_node() -> (Dispatcher, RuleBodies) {
    let mut t = Table::new("A", 4);
    let mut p = CaseDomain::free(4, Some(CHART.to_vec()));
    p.fixed = vec![Some(3), Some(1), Some(0), Some(0)];
    t.push(0, p).unwrap();
    let mut f = CaseDomain::free(4, Some(CHART.to_vec()));
    f.bounds = vec![rustred_campaign_core::poly::Poly::parse_d_free("n0-n1-1", 4).unwrap()];
    t.push(1, f).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(2,1,0,0)", "I(0,1,0,0)", "I(3,1,1,0)"])
        .unwrap();
    b.set(
        &d,
        0,
        1,
        &[
            "I(n0-1,n1,n2,n3)",
            "I(0,n1,n2,n3)",
            "I(n0,0,n2,n3)",
            "I(n0,n1,n2+1,n3)",
            "I(n0,n1-1,n2,n3)",
        ],
    )
    .unwrap();
    (d, b)
}

fn spill_cfg(spill: Option<SpillConfig>) -> ClosureConfig {
    ClosureConfig {
        tier: Tier::C,
        threads: 3,
        hand_downs: HandDowns::Keys,
        table_digests: [("A".to_string(), "sha256:closurefix-A".to_string())].into(),
        credits: Some(CreditPolicy::new(vec![GroupElement {
            lines: vec![1, 0],
            numerators: None,
        }])),
        spill,
        ..Default::default()
    }
}

#[test]
fn spilled_run_equals_the_in_memory_run_and_streams_each_request_once() {
    let (d, b) = spill_node();
    let cmp = NodeComparator::legacy(4, None).unwrap();
    let codec = KeyCodec::new(4, 6).unwrap();
    let mut keys: Vec<Vec<i32>> = vec![vec![1, 3, 0, 0], vec![3, 1, 0, 0]];
    for a in 2..=20 {
        for c in 1..a {
            keys.push(vec![a, c, 0, -(a % 3)]);
        }
    }
    let start = KeySet::from_keys(codec, keys.iter().map(|k| k.as_slice())).unwrap();
    let mem = closure(&d, &b, &cmp, &start, &spill_cfg(None), &UnknownFacts).unwrap();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("closurefix-spill");
    let sp = closure(
        &d,
        &b,
        &cmp,
        &start,
        &spill_cfg(Some(SpillConfig {
            dir: dir.clone(),
            budget_bytes: 256,
        })),
        &UnknownFacts,
    )
    .unwrap();
    let (sm, ss) = (mem.summary(), sp.summary());
    println!(
        "keys {} credited {} requests {} exits {:?}; spilled requests {} exits {} (dir {:?}, files {})",
        sm.keys_total,
        sm.credited,
        sm.fact_requests,
        sm.hand_downs,
        ss.fact_requests_spilled,
        ss.exits_spilled,
        sp.spill_dir(),
        sp.exit_files().len()
    );
    assert!(ss.fact_requests_spilled && ss.exits_spilled);
    assert!(!sm.fact_requests_spilled && !sm.exits_spilled);
    let strip = |s: &ClosureSummary| {
        let mut s = s.clone();
        s.fact_requests_spilled = false;
        s.exits_spilled = false;
        s
    };
    assert_eq!(strip(&sm), strip(&ss));
    assert_eq!(mem.exit_counts(), sp.exit_counts());
    let mut r = mem.report(&Default::default());
    let mut q = sp.report(&Default::default());
    r.summary = strip(&r.summary);
    q.summary = strip(&q.summary);
    assert_eq!(r, q);
    // the spilled files are complete key-set files with the digests of the in-memory sets
    for (m, f) in sp.exit_files() {
        let ks =
            KeySet::read(std::io::BufReader::new(std::fs::File::open(&f.path).unwrap())).unwrap();
        assert_eq!(ks, mem.hand_down_sets()[m]);
    }
    // requests: identical, and the distinct count needs no materialisation
    let reqs = mem.fact_requests();
    assert_eq!(reqs, sp.fact_requests());
    assert_eq!(sm.fact_requests, reqs.len() as u64);
    assert!(sm.credited >= 1 && sm.undecided.regularity_unknown >= 2);
    for c in [&mem, &sp] {
        let mut buf = vec![];
        let n = c.write_fact_requests_jsonl(&mut buf).unwrap();
        let lines: Vec<serde_json::Value> = String::from_utf8(buf)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(n, lines.len() as u64);
        assert_eq!(n, sm.fact_requests);
        let set: BTreeSet<String> = lines.iter().map(|v| v.to_string()).collect();
        assert_eq!(set.len(), lines.len(), "each request once");
        let want: BTreeSet<String> = reqs
            .iter()
            .map(|r| serde_json::to_value(r).unwrap().to_string())
            .collect();
        assert_eq!(set, want);
    }
    // recheck of the spilled closure: needs a spill directory, then equals the closure of the union
    let extra = KeySet::from_keys(codec, [[24, 2, 0, 0].as_slice()]).unwrap();
    assert!(matches!(
        closure_recheck(&sp, &d, &b, &cmp, &extra, &spill_cfg(None), &UnknownFacts),
        Err(ClosureError::Io(_)) | Err(ClosureError::NotIncremental(_))
    ));
    let re = closure_recheck(
        &sp,
        &d,
        &b,
        &cmp,
        &extra,
        &spill_cfg(Some(SpillConfig {
            dir,
            budget_bytes: 256,
        })),
        &UnknownFacts,
    )
    .unwrap();
    let un = closure(&d, &b, &cmp, &start.union(&extra), &spill_cfg(None), &UnknownFacts).unwrap();
    assert_eq!(re.keys(), un.keys());
    assert_eq!(re.exit_counts(), un.exit_counts());
    assert_eq!(re.fact_requests(), un.fact_requests());
    assert_eq!(re.summary().fact_requests, un.summary().fact_requests);
}

// ------------------------------------------------------------------------------------------------ F6b, A-m7, F4

/// F6b: under a round limit the recheck truncates exactly the keys the closure of the union truncates (the
/// continuation counts its own rounds).
#[test]
fn recheck_under_a_round_limit_truncates_as_the_union() {
    let mut t = Table::new("A", 4);
    t.push(0, CaseDomain::free(4, Some(CHART.to_vec()))).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(n0-1,n1,n2,n3)"]).unwrap();
    let cmp = NodeComparator::legacy(4, None).unwrap();
    let codec = KeyCodec::new(4, 6).unwrap();
    let ks = |v: &[[i32; 4]]| KeySet::from_keys(codec, v.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 1,
        max_rounds: Some(3),
        ..Default::default()
    };
    let prev = closure(&d, &b, &cmp, &ks(&[[2, 1, 0, 0]]), &cfg, &NoFacts).unwrap();
    assert!(!prev.summary().truncated);
    let extra = ks(&[[6, 1, 0, 0]]);
    let re = closure_recheck(&prev, &d, &b, &cmp, &extra, &cfg, &NoFacts).unwrap();
    let un = closure(&d, &b, &cmp, &ks(&[[2, 1, 0, 0], [6, 1, 0, 0]]), &cfg, &NoFacts).unwrap();
    let open = |c: &rustred_campaign_core::closure::Closure| -> BTreeMap<Vec<i32>, OpenStatus> {
        c.open_keys().into_iter().map(|o| (o.key, o.status)).collect()
    };
    println!(
        "recheck open {:?} truncated {}; union open {:?} truncated {}",
        open(&re),
        re.summary().truncated,
        open(&un),
        un.summary().truncated
    );
    assert_eq!(open(&re), open(&un));
    assert_eq!(re.keys(), un.keys());
    assert_eq!(re.summary().truncated, un.summary().truncated);
    assert_eq!(re.summary().rounds, un.summary().rounds);
}

/// Audit loop-A m7: a declared table frame must equal the comparator's presentation (typed error otherwise), and the
/// identity records whether it was checked. F4: arity above the mask width is a typed error.
#[test]
fn frame_check_and_arity_refusal() {
    let mut t = Table::new("A", 4);
    t.push(0, CaseDomain::free(4, Some(CHART.to_vec()))).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(n0-1,n1,n2,n3)"]).unwrap();
    let cmp = NodeComparator::legacy(4, None).unwrap();
    let st = KeySet::from_keys(KeyCodec::new(4, 6).unwrap(), [[2, 1, 0, 0].as_slice()]).unwrap();
    let c0 = closure(&d, &b, &cmp, &st, &ClosureConfig::default(), &NoFacts).unwrap();
    assert!(c0.config_identity().components["frame"].starts_with("unchecked"));
    let ok = ClosureConfig {
        table_presentation: Some(vec![0, 1, 2, 3]),
        ..Default::default()
    };
    let c1 = closure(&d, &b, &cmp, &st, &ok, &NoFacts).unwrap();
    assert!(c1.config_identity().components["frame"].starts_with("checked"));
    let bad = ClosureConfig {
        table_presentation: Some(vec![1, 0, 2, 3]),
        ..Default::default()
    };
    assert!(matches!(
        closure(&d, &b, &cmp, &st, &bad, &NoFacts),
        Err(ClosureError::Config(_))
    ));
    // F4
    let n = 70usize;
    let t = Table::new("T", n);
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let b = RuleBodies::new(&d);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let codec = KeyCodec::new(n, 4).unwrap();
    let k = vec![1i32; n];
    let st = KeySet::from_keys(codec, [k.as_slice()]).unwrap();
    let cfg = ClosureConfig {
        bits: 4,
        lines: Some(vec![true; n]),
        ..Default::default()
    };
    assert!(matches!(
        closure(&d, &b, &cmp, &st, &cfg, &NoFacts),
        Err(ClosureError::Arity { arity: 70, max: 64 })
    ));
}
