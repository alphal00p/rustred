//! Lane partfix4 (audit notes/implementer.reaudit-part4.md): ONE meaning of a rule record's `domain_bounds` (the
//! engine's exception branches, `domain::bound_branches`) in every reader; the closed member list of rule records;
//! the selection-bound precondition with target patterns (B2 / B2b); the digest v4 (B1b); tests that kill the audit's
//! surviving mutants 102 105 112 114 115 116 117; the ENGINE cross-check of a retirement
//! (`ContainmentProof::with_engine_check`), incl. runs of the frozen engine (`rr_node first-match`, in a private pid
//! namespace) on generated records WITH bounds and target patterns.
//!
//! Synthetic tables only (lane rule L8). The frozen engine path is test data (env `RUSTRED_FROZEN_RR_NODE` overrides
//! it); the engine-backed tests say SKIPPED when it is absent.

use std::path::{Path, PathBuf};

use rustred_campaign_core::cover::{Assurance, CreditBasis, Dispatch, Dispatcher, RuleRef, Table};
use rustred_campaign_core::domain::{
    CaseDomain, DomainError, DomainOptions, ENGINE_MAX_BOUND_EXCLUSIONS, TargetMap, TargetSlot,
    bound_branches,
};
use rustred_campaign_core::economy::{
    CoverageState, EngineFirstMatchRun, ProofWork, RetireError, Retirement, RuleMeasure,
    SupersedeReason, engine_probe_table, engine_readable_bound, prove_coverage_preserved,
    prove_coverage_preserved_with_box, table_set_digest,
};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::nf::{ModularRisk, modular_risk};
use rustred_campaign_core::partition::{
    ClassHit, CoverageNotion, DispatchOrderStatement, DispatcherSource, DomainList,
    PartitionOptions, gap_partition,
};
use rustred_campaign_core::poly::{ENGINE_PRIMES, Poly};
use rustred_campaign_core::record::{DomainBounds, ReadOptions, read_table_file};
use rustred_campaign_core::replay_base::sha256::sha256_hex;

// ------------------------------------------------------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------------------------------------------------------

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pf4_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn header(mask: &str) -> String {
    format!(
        r#"{{"type":"header","schema":"table_dump.v1","family":"pf4","denominators":{},"mask":"{mask}"}}"#,
        mask.len()
    )
}

fn chart(mask: &str) -> String {
    let g: Vec<&str> = mask
        .chars()
        .map(|c| if c == '1' { "\"n>=1\"" } else { "\"n<=0\"" })
        .collect();
    format!(
        r#"{{"type":"chart","chart":"{mask}","guards":[{}]}}"#,
        g.join(",")
    )
}

fn write(dir: &Path, name: &str, lines: &[String]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();
    p
}

/// A rule record line (witness complete, dispatched).
fn rule(mask: &str, index: usize, fixed: &str, target: &str, exc: &str, extra: &str) -> String {
    format!(
        r#"{{"type":"rule","chart":"{mask}","index":{index},"fixed":{fixed},"equations":[],"exceptions":{exc},"target":"{target}","rhs":[],"witness_loci":[],"dispatch":"Partition"{extra}}}"#
    )
}

/// The table loaded as assembly / the import load it (`read_table_file` + `Table::from_table_file`), chart = mask.
fn load(p: &Path) -> Result<(Dispatcher, NodeFrame), String> {
    let tf = read_table_file(p, ReadOptions::default()).map_err(|e| e.to_string())?;
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).map_err(|e| e.to_string())?;
    let t = Table::from_table_file("t", "pf4", &tf, DomainOptions::default())
        .map_err(|e| e.to_string())?;
    let d = Dispatcher::new(vec![t], vec![0])
        .map_err(|e| format!("{e:?}"))?
        .with_chart(frame.mask().to_vec())
        .map_err(|e| format!("{e:?}"))?;
    Ok((d, frame))
}

fn pos(d: &Dispatcher, k: &[i32]) -> Option<usize> {
    match d.first_match(k).unwrap() {
        Dispatch::Covered(r) => Some(r.record),
        _ => None,
    }
}

fn list(ds: &[CaseDomain], n: usize) -> DomainList {
    let mut l = DomainList::new(n);
    for (i, d) in ds.iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d.clone());
    }
    l
}

fn disp(ds: &[CaseDomain], n: usize, mask: &[bool]) -> Dispatcher {
    let mut t = Table::new("t", n);
    for (i, d) in ds.iter().enumerate() {
        t.push(i as i64, d.clone()).unwrap();
    }
    Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(mask.to_vec())
        .unwrap()
}

fn mint(ds: &[CaseDomain], f: &NodeFrame) -> Result<(), String> {
    let l = list(ds, f.arity());
    prove_coverage_preserved(
        &l,
        &l,
        f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .map(|_| ())
}

fn dispatch_opts() -> PartitionOptions {
    PartitionOptions {
        coverage: CoverageNotion::DispatchCoverage,
        ..Default::default()
    }
}

/// The dispatch-mode partition's class of `k` (the partition itself, independent of the minting cross-check).
fn class(ds: &[CaseDomain], f: &NodeFrame, k: &[i32]) -> ClassHit {
    let l = list(ds, f.arity());
    let rep = gap_partition(&l, f, &dispatch_opts(), None).unwrap();
    rep.class_of(k).unwrap()
}

fn measure(t: u64) -> RuleMeasure {
    RuleMeasure {
        cases: 1,
        rhs_terms: t,
        monomials: t,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 0,
    }
}

fn p1() -> i128 {
    i128::from(ENGINE_PRIMES[0])
}
fn p2() -> i128 {
    i128::from(ENGINE_PRIMES[1])
}

/// `p1 * (a x + b)` in slot `v`: content p1, so the modular test needs `a x + b` divisible by p2 (and nonzero).
fn risky(n: usize, v: usize, a: i128, b: i128) -> Poly {
    Poly::parse_d_free_exact(&format!("{}*n{v}+{}", p1() * a, p1() * b), n).unwrap()
}

// ------------------------------------------------------------------------------------------------------------------
// 1. ONE meaning of domain_bounds: the engine's (node/bounds.rs), ported
// ------------------------------------------------------------------------------------------------------------------

/// The engine's own test vectors (rustred-core node/bounds.rs `absent_bounds_add_nothing_and_finite_cuts_only`),
/// through `domain::bound_branches`: same branches, same refusals.
#[test]
fn pf4_bound_branches_are_the_engines() {
    let chart = [true, true, false];
    let fixed = [None, Some(1), None];
    let b = |min: [Option<i64>; 3], max: [Option<i64>; 3]| DomainBounds {
        min: min.to_vec(),
        max: max.to_vec(),
    };
    let none = [None; 3];
    let text = |r: Result<Vec<Vec<Poly>>, String>| -> Vec<Vec<String>> {
        r.unwrap()
            .iter()
            .map(|br| br.iter().map(|p| p.to_string()).collect())
            .collect()
    };
    assert!(
        bound_branches(&b(none, none), &chart, &fixed)
            .unwrap()
            .is_empty()
    );
    // min [3,-,-], max [-,-,-2]: n0 = 1, 2 and n2 = -1, 0 excluded (one branch each)
    let got = bound_branches(
        &b([Some(3), None, None], [None, None, Some(-2)]),
        &chart,
        &fixed,
    )
    .unwrap();
    let want: Vec<Vec<Poly>> = [(0, 1), (0, 2), (2, -1), (2, 0)]
        .iter()
        .map(|&(i, v)| vec![Poly::slot_minus(i, v)])
        .collect();
    assert_eq!(got, want);
    // a max on a line slot / a min on a numerator slot (fixed or not): refused
    assert!(bound_branches(&b(none, [Some(4), None, None]), &chart, &fixed).is_err());
    assert!(bound_branches(&b(none, [None, Some(3), None]), &chart, &fixed).is_err());
    assert!(
        bound_branches(
            &b([None, None, Some(-1)], none),
            &chart,
            &[None, Some(1), Some(0)]
        )
        .is_err()
    );
    // a violated bound on a FIXED slot: the branch n1 - 1 (the whole case on a canonical target)
    assert_eq!(
        text(bound_branches(
            &b([None, Some(2), None], none),
            &chart,
            &fixed
        )),
        vec![vec![Poly::slot_minus(1, 1).to_string()]]
    );
    // far: refused on a free slot, a plain comparison on a fixed slot
    assert!(bound_branches(&b([Some(100), None, None], none), &chart, &fixed).is_err());
    assert_eq!(
        bound_branches(&b([None, Some(100), None], none), &chart, &fixed)
            .unwrap()
            .len(),
        1
    );
    // the limit itself (min 65 excludes 64 values) is accepted, one more is refused; the same on a numerator slot
    let lim = ENGINE_MAX_BOUND_EXCLUSIONS;
    assert_eq!(
        bound_branches(&b([Some(lim + 1), None, None], none), &chart, &fixed)
            .unwrap()
            .len(),
        64
    );
    assert!(bound_branches(&b([Some(lim + 2), None, None], none), &chart, &fixed).is_err());
    assert_eq!(
        bound_branches(&b(none, [None, None, Some(-lim)]), &chart, &fixed)
            .unwrap()
            .len(),
        64
    );
    assert!(bound_branches(&b(none, [None, None, Some(-lim - 1)]), &chart, &fixed).is_err());
    // i64 edges where the engine's arithmetic would overflow (min = i64::MIN; max = i64::MIN would make the engine
    // enumerate 2^63 values): refused
    assert!(bound_branches(&b([Some(i64::MIN), None, None], none), &chart, &fixed).is_err());
    assert!(bound_branches(&b(none, [None, None, Some(i64::MIN)]), &chart, &fixed).is_err());
    // lengths other than the chart's
    let short = DomainBounds {
        min: vec![None; 2],
        max: vec![None; 3],
    };
    assert!(bound_branches(&short, &chart, &fixed).is_err());
}

/// The reader keeps `domain_bounds` (B1: it dropped them) with the engine's shape rules, and every rule-record member
/// outside the closed list makes the record unloadable (typed), in the table loader used by assembly and the import.
#[test]
fn pf4_reader_keeps_bounds_and_refuses_unknown_members() {
    let dir = scratch("reader");
    let m = "11";
    let ok = rule(
        m,
        0,
        "[null,null]",
        "I(n0,n1)",
        "[]",
        r#","domain_bounds":{"min":[3,null],"max":null}"#,
    );
    let p = write(&dir, "ok.jsonl", &[header(m), chart(m), ok]);
    let tf = read_table_file(&p, ReadOptions::default()).unwrap();
    assert_eq!(
        tf.rules[0].domain_bounds,
        Some(DomainBounds {
            min: vec![Some(3), None],
            max: vec![None, None]
        })
    );
    let (d, _) = load(&p).unwrap();
    assert_eq!(
        d.tables()[0].domains[0].exceptions.len(),
        2,
        "n0 = 1, 2 excluded"
    );
    assert_eq!(pos(&d, &[1, 1]), None);
    assert_eq!(pos(&d, &[2, 5]), None);
    assert_eq!(pos(&d, &[3, 1]), Some(0));
    // malformed shapes: read errors (never ignored)
    for bad in [
        r#","domain_bounds":[3,null]"#,
        r#","domain_bounds":{"min":[3,null],"low":[1,1]}"#,
        r#","domain_bounds":{"min":[3]}"#,
        r#","domain_bounds":{"min":[3.0,null]}"#,
        r#","domain_bounds":{"min":"3"}"#,
    ] {
        let p = write(
            &dir,
            "bad.jsonl",
            &[
                header(m),
                chart(m),
                rule(m, 0, "[null,null]", "I(n0,n1)", "[]", bad),
            ],
        );
        assert!(
            read_table_file(&p, ReadOptions::default()).is_err(),
            "{bad}"
        );
    }
    // engine refusals: typed domain error at load
    let p = write(
        &dir,
        "max_on_line.jsonl",
        &[
            header(m),
            chart(m),
            rule(
                m,
                0,
                "[null,null]",
                "I(n0,n1)",
                "[]",
                r#","domain_bounds":{"max":[2,null]}"#,
            ),
        ],
    );
    let e = load(&p).err().expect("refused");
    assert!(e.contains("domain_bounds") && e.contains("infinite"), "{e}");
    // an unknown member (e.g. one the engine reads in a future version) is refused, never ignored
    let p = write(
        &dir,
        "unknown.jsonl",
        &[
            header(m),
            chart(m),
            rule(
                m,
                0,
                "[null,null]",
                "I(n0,n1)",
                "[]",
                r#","domain_upper":{"max":[null,-1]}"#,
            ),
        ],
    );
    let tf = read_table_file(&p, ReadOptions::default()).unwrap();
    assert_eq!(
        tf.rules[0].unknown_members,
        vec!["domain_upper".to_string()]
    );
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap();
    assert!(matches!(
        CaseDomain::from_rule_record(&tf.rules[0], &frame, DomainOptions::default()),
        Err(DomainError::UnknownMember(m)) if m == vec!["domain_upper".to_string()]
    ));
    assert!(
        load(&p)
            .err()
            .expect("refused")
            .contains("unknown to the reader")
    );
    // the members the engine and the campaign write (informational ones included) load
    let all = rule(
        m,
        0,
        "[null,null]",
        "I(n0,n1)",
        "[]",
        r#","case":"c","seeds":[],"nsources":1,"direct_hit":false,"seconds_search":0.1,"raw_pivot_encoded":[1,1],"recenter":[0,0],"trace_rows":1,"lift_seconds":0.1,"source_order":"input","numerical":false,"monomials":1,"oversized_fallback":false,"witness_poles":0,"witness_d_poles":[],"guard_version":2,"origin":"x","origin_index":1"#,
    );
    let p = write(&dir, "all.jsonl", &[header(m), chart(m), all]);
    assert!(load(&p).is_ok());
}

// ------------------------------------------------------------------------------------------------------------------
// 2. B2 / B2b: the engine's reading is the core's now; selection bounds with patterns are refused for proofs
// ------------------------------------------------------------------------------------------------------------------

/// B2 (target pattern `I(n0+3)`, `domain_bounds.min = [2]`): the core reads the record as the engine does (one
/// exclusion branch `n0 - 1` at the rule point): R serves keys 1, 2, 3 (rule point -2..0) and 5.., S serves key 4
/// (rule point 1). The frozen engine's answers recorded by the audit: label 3 (R) at 1..=3, 4 (S) at 4, 3 from 5.
#[test]
fn pf4_b2_pattern_bounds_dispatch_as_the_engine() {
    let dir = scratch("b2");
    let rr = r#"{"type":"rule","chart":"1","index":3,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0+3)","rhs":[["I(n0+2)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[2],"max":[null]}}"#;
    let ss = r#"{"type":"rule","chart":"1","index":4,"fixed":[null],"equations":[],"exceptions":[],"target":"I(n0)","rhs":[["I(n0-1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let p = write(
        &dir,
        "b2.jsonl",
        &[header("1"), chart("1"), rr.into(), ss.into()],
    );
    let (d, _) = load(&p).unwrap();
    let got: Vec<Option<usize>> = (1..=7).map(|k| pos(&d, &[k])).collect();
    assert_eq!(
        got,
        vec![
            Some(0),
            Some(0),
            Some(0),
            Some(1),
            Some(0),
            Some(0),
            Some(0)
        ]
    );
    // the GE reading (a selection bound on a pattern record) is never proved: precondition
    let mut r = CaseDomain::free(1, Some(vec![true]));
    r.target = TargetMap::Pattern(vec![TargetSlot::Param {
        param: 0,
        offset: 3,
    }]);
    r.bounds = vec![Poly::parse_d_free_exact("n0-2", 1).unwrap()];
    let s = CaseDomain::free(1, Some(vec![true]));
    let e = mint(&[r, s], &NodeFrame::without_header_lines_first(1, 1)).unwrap_err();
    assert!(e.contains("target pattern"), "{e}");
}

/// B2b (bound on a FIXED slot, target `I(n1,n0)`, `fixed = [5, null]`, `min = [3, null]`): the engine compares the
/// key's fixed value 5 with the bound and excludes nothing; the core does the same (R serves (5,1), (5,2): the audit's
/// engine answers). A selection GE bound with that pattern is refused for proofs.
#[test]
fn pf4_b2b_fixed_slot_bound_dispatch_as_the_engine() {
    let dir = scratch("b2b");
    let rr = r#"{"type":"rule","chart":"11","index":0,"fixed":[5,null],"equations":[],"exceptions":[],"target":"I(n1,n0)","rhs":[["I(4,1)","1"]],"witness_loci":[],"dispatch":"Partition","domain_bounds":{"min":[3,null],"max":[null,null]}}"#;
    let ss = r#"{"type":"rule","chart":"11","index":1,"fixed":[null,null],"equations":[],"exceptions":[],"target":"I(n0,n1)","rhs":[["I(n0-1,n1)","1"]],"witness_loci":[],"dispatch":"Partition"}"#;
    let p = write(
        &dir,
        "b2b.jsonl",
        &[header("11"), chart("11"), rr.into(), ss.into()],
    );
    let (d, _) = load(&p).unwrap();
    for k in [[5, 1], [5, 2], [5, 3], [5, 9]] {
        assert_eq!(pos(&d, &k), Some(0), "{k:?}");
    }
    assert_eq!(pos(&d, &[4, 1]), Some(1));
    // violated on the fixed slot (fixed 2 < min 3): the branch n0 - 2 at the rule point (pt0 = k1): R excluded where
    // k1 = 2 only (the engine's behaviour, reproduced)
    let rv = rr.replace("[5,null]", "[2,null]");
    let p = write(
        &dir,
        "b2b_v.jsonl",
        &[header("11"), chart("11"), rv, ss.into()],
    );
    let (d, _) = load(&p).unwrap();
    assert_eq!(pos(&d, &[2, 2]), Some(1));
    assert_eq!(pos(&d, &[2, 1]), Some(0));
    assert_eq!(pos(&d, &[2, 3]), Some(0));
    let mut r = CaseDomain::free(2, Some(vec![true, true]));
    r.fixed = vec![Some(5), None];
    r.target = TargetMap::Pattern(vec![
        TargetSlot::Param {
            param: 1,
            offset: 0,
        },
        TargetSlot::Param {
            param: 0,
            offset: 0,
        },
    ]);
    r.bounds = vec![Poly::parse_d_free_exact("n0-3", 2).unwrap()];
    let s = CaseDomain::free(2, Some(vec![true, true]));
    assert!(mint(&[r, s], &NodeFrame::without_header_lines_first(2, 2)).is_err());
}

/// B1b: two files that differ only in `domain_bounds` have different digests (v4); a digest is never the v3 one.
#[test]
fn pf4_b1b_digest_v4_binds_the_bounds() {
    let dir = scratch("b1b");
    let m = "11";
    let plain = rule(m, 0, "[null,null]", "I(n0,n1)", "[]", "");
    let bounded = rule(
        m,
        0,
        "[null,null]",
        "I(n0,n1)",
        "[]",
        r#","domain_bounds":{"min":[5,7],"max":[null,null]}"#,
    );
    let (d1, f1) = load(&write(&dir, "p.jsonl", &[header(m), chart(m), plain])).unwrap();
    let (d2, f2) = load(&write(&dir, "b.jsonl", &[header(m), chart(m), bounded])).unwrap();
    let g1 = table_set_digest(&DispatcherSource::new(&d1), &f1);
    let g2 = table_set_digest(&DispatcherSource::new(&d2), &f2);
    assert_ne!(g1, g2);
}

// ------------------------------------------------------------------------------------------------------------------
// 3. the audit's surviving mutants
// ------------------------------------------------------------------------------------------------------------------

/// Mutant 102 (unchecked-attestation precondition disabled): a record whose target rests on an UNCHECKED attestation
/// is never proved, although its domain covers every key.
#[test]
fn pf4_m102_unchecked_attestation_is_refused() {
    let f = NodeFrame::without_header_lines_first(1, 1);
    let mut l = DomainList::new(1);
    let mut r = RuleRef::bare(0, 0, 0);
    r.basis = CreditBasis {
        witness: Assurance::Record,
        target: Assurance::AttestedUnchecked,
        chart_checked: true,
    };
    l.push(r, CaseDomain::free(1, Some(vec![true])));
    let e = prove_coverage_preserved(
        &l,
        &l,
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .unwrap_err();
    assert!(e.contains("unchecked attestation"), "{e}");
    // the same record on a checked basis: minted
    let mut l2 = DomainList::new(1);
    l2.push(
        RuleRef::bare(0, 0, 0),
        CaseDomain::free(1, Some(vec![true])),
    );
    assert!(
        prove_coverage_preserved(
            &l2,
            &l2,
            &f,
            &ProofWork::default(),
            &DispatchOrderStatement::SameOrder
        )
        .is_ok()
    );
}

/// Mutant 105 (the engine's 64-value limit x10): a bound excluding 65 chart values is not engine-readable (the
/// engine refuses the record at load), 64 are; on a numerator slot the same.
#[test]
fn pf4_m105_engine_limit_of_64_excluded_values() {
    let f = NodeFrame::without_header_lines_first(2, 1);
    let g = |t: &str| Poly::parse_d_free_exact(t, 2).unwrap();
    assert!(
        engine_readable_bound(&g("n0-65"), &f).is_ok(),
        "excludes 1..64"
    );
    assert!(
        engine_readable_bound(&g("n0-66"), &f).is_err(),
        "excludes 1..65"
    );
    assert!(
        engine_readable_bound(&g("-n1-64"), &f).is_ok(),
        "excludes -63..0"
    );
    assert!(
        engine_readable_bound(&g("-n1-65"), &f).is_err(),
        "excludes -64..0"
    );
    let mut r = CaseDomain::free(2, Some(vec![true, false]));
    r.bounds = vec![g("n0-66")];
    let s = CaseDomain::free(2, Some(vec![true, false]));
    let e = mint(&[r, s], &f).unwrap_err();
    assert!(e.contains("engine-readable"), "{e}");
}

/// Mutant 114 (the |pt| bound of the modular-risk test 2^31 -> 2^20): `p1 (a n0 + b)` with `a = 2^30`, `b = p2 - a 2^25`
/// is a nonzero multiple of p1 p2 exactly at the key 2^25 (between 2^20 and 2^31): the dispatcher is Undecided there,
/// the risk is the exact point, the partition reports the key open, no proof.
#[test]
fn pf4_m114_modular_risk_up_to_2_pow_31() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let x0: i32 = 1 << 25;
    let a: i128 = 1 << 30;
    let mut r = CaseDomain::free(n, Some(vec![true]));
    r.equations = vec![risky(n, 0, a, p2() - a * i128::from(x0))];
    let s = CaseDomain::free(n, Some(vec![true]));
    assert_eq!(
        modular_risk(&r),
        Some(ModularRisk::Points(vec![(0, i128::from(x0))]))
    );
    let ds = vec![r, s];
    let d = disp(&ds, n, &[true]);
    assert!(matches!(
        d.first_match(&[x0]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    assert_eq!(pos(&d, &[x0 - 1]), Some(1));
    assert_ne!(class(&ds, &f, &[x0]), ClassHit::Covered);
    assert!(mint(&ds, &f).is_err());
}

/// Mutant 116 (the negative residue `r - pm` dropped): on a NUMERATOR slot the risky key is `-2^25` (`r - p2`).
#[test]
fn pf4_m116_negative_residue_is_a_stop() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 0);
    let x0: i32 = -(1 << 25);
    let a: i128 = 1 << 30;
    let mut r = CaseDomain::free(n, Some(vec![false]));
    r.equations = vec![risky(n, 0, a, p2() - a * i128::from(x0))];
    let s = CaseDomain::free(n, Some(vec![false]));
    assert_eq!(
        modular_risk(&r),
        Some(ModularRisk::Points(vec![(0, i128::from(x0))]))
    );
    let ds = vec![r, s];
    let d = disp(&ds, n, &[false]);
    assert!(matches!(
        d.first_match(&[x0]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    assert_ne!(class(&ds, &f, &[x0]), ClassHit::Covered);
    assert!(mint(&ds, &f).is_err());
}

fn isqrt(v: u128) -> u128 {
    let mut x = (v as f64).sqrt() as u128;
    while x * x > v {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= v {
        x += 1;
    }
    x
}

/// Mutant 115 (the FAR radius doubled): `p1 (n0^2 - c)` with `c = x0^2 - p2` for `x0 = isqrt(p2) + 10` is a nonzero
/// multiple of p1 p2 at `x0`, which lies between the exact radius `r = ceil(sqrt(p2 - c))` and `2 r`: the far piece
/// starts at r (exactly), the dispatcher is Undecided at x0, no proof.
#[test]
fn pf4_m115_far_radius_is_exact() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let pp2 = ENGINE_PRIMES[1] as u128;
    let mut x0 = isqrt(pp2) + 10;
    let mut c = x0 * x0 - pp2;
    while isqrt(c) * isqrt(c) == c {
        x0 += 1;
        c = x0 * x0 - pp2;
    }
    let need = pp2 - c; // smallest r with r^2 + c >= p2
    let mut rad = isqrt(need);
    if rad * rad < need {
        rad += 1;
    }
    assert!(rad <= x0 && x0 < 2 * rad && x0 < (1u128 << 31));
    let mut r = CaseDomain::free(n, Some(vec![true]));
    r.equations =
        vec![Poly::parse_d_free_exact(&format!("{}*n0^2-{}", p1(), p1() * c as i128), n).unwrap()];
    let s = CaseDomain::free(n, Some(vec![true]));
    assert_eq!(
        modular_risk(&r),
        Some(ModularRisk::Far(vec![(0, rad as i128)]))
    );
    let x = x0 as i32;
    let ds = vec![r, s];
    let d = disp(&ds, n, &[true]);
    assert!(matches!(
        d.first_match(&[x]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    assert_ne!(class(&ds, &f, &[x]), ClassHit::Covered);
    assert!(mint(&ds, &f).is_err());
}

/// Mutant 117 (modular risk only for the FIRST equation): the risky literal is the SECOND equation, reached where the
/// first (`n1 - 1`) vanishes: the key (2^25, 1) is Undecided, open in the partition, no proof.
#[test]
fn pf4_m117_every_reached_equation_is_risk_checked() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let x0: i32 = 1 << 25;
    let a: i128 = 1 << 30;
    let mut r = CaseDomain::free(n, Some(vec![true, true]));
    r.equations = vec![
        Poly::parse_d_free_exact("n1-1", n).unwrap(),
        risky(n, 0, a, p2() - a * i128::from(x0)),
    ];
    let s = CaseDomain::free(n, Some(vec![true, true]));
    assert!(modular_risk(&r).is_some());
    let ds = vec![r, s];
    let d = disp(&ds, n, &[true, true]);
    assert!(matches!(
        d.first_match(&[x0, 1]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    assert_eq!(pos(&d, &[x0, 2]), Some(1));
    assert_ne!(class(&ds, &f, &[x0, 1]), ClassHit::Covered);
    assert!(mint(&ds, &f).is_err());
}

/// Mutant 112 (the BOUNDS added to an equation's risk prefix): the dispatcher evaluates equations BEFORE bounds, so
/// the risky equation stops dispatch also where the bound `n1 >= 3` fails: (2^25, 1) is Undecided and open.
#[test]
fn pf4_m112_bounds_are_not_in_an_equations_prefix() {
    let n = 2;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let x0: i32 = 1 << 25;
    let a: i128 = 1 << 30;
    let mut r = CaseDomain::free(n, Some(vec![true, true]));
    r.equations = vec![risky(n, 0, a, p2() - a * i128::from(x0))];
    r.bounds = vec![Poly::parse_d_free_exact("n1-3", n).unwrap()];
    let s = CaseDomain::free(n, Some(vec![true, true]));
    let ds = vec![r, s];
    let d = disp(&ds, n, &[true, true]);
    assert!(matches!(
        d.first_match(&[x0, 1]).unwrap(),
        Dispatch::Undecided { .. }
    ));
    assert_ne!(class(&ds, &f, &[x0, 1]), ClassHit::Covered);
    assert!(mint(&ds, &f).is_err());
}

// ------------------------------------------------------------------------------------------------------------------
// 4. the ENGINE cross-check (no engine needed: runs written by the test)
// ------------------------------------------------------------------------------------------------------------------

fn honest(
    p: &rustred_campaign_core::economy::ContainmentProof,
    table_sha: &str,
) -> EngineFirstMatchRun {
    let req = p.engine_probe();
    EngineFirstMatchRun {
        engine_sha256: "engine".into(),
        table_sha256: table_sha.into(),
        keys_sha256: req.keys_sha256.clone(),
        positions: req.expected.iter().map(|&x| Some(x)).collect(),
    }
}

/// A proof carries the keys the engine must answer (probe keys + box keys) and the core's first match at each; only a
/// matching engine run is bound; a FULL retirement without it is refused.
#[test]
fn pf4_engine_cross_check_is_mandatory_for_full_retirements() {
    let n = 1;
    let f = NodeFrame::without_header_lines_first(n, 1);
    let mut p = CaseDomain::free(n, Some(vec![true]));
    p.fixed = vec![Some(1)];
    let a = CaseDomain::free(n, Some(vec![true]));
    let old = list(&[p.clone(), a.clone(), a.clone()], n);
    let new = list(&[p, a], n);
    let box_keys: Vec<Vec<i32>> = (1..=12).map(|k| vec![k]).collect();
    let proof = prove_coverage_preserved_with_box(
        &old,
        &new,
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
        &box_keys,
    )
    .unwrap();
    let req = proof.engine_probe().clone();
    assert!(box_keys.iter().all(|k| req.keys.contains(k)));
    assert_eq!(req.keys.len(), req.expected.len());
    let i1 = req.keys.iter().position(|k| k == &vec![1]).unwrap();
    let i5 = req.keys.iter().position(|k| k == &vec![5]).unwrap();
    assert_eq!((req.expected[i1], req.expected[i5]), (0, 1));
    assert_eq!(req.keys_sha256, sha256_hex(&req.keys_file()));
    // a box key outside the chart is refused
    assert!(
        prove_coverage_preserved_with_box(
            &old,
            &new,
            &f,
            &ProofWork::default(),
            &DispatchOrderStatement::SameOrder,
            &[vec![0]]
        )
        .is_err()
    );
    let full = |pr| {
        Retirement::new(
            "N",
            CoverageState::Full,
            &[("b".to_string(), measure(10))],
            ("a", &measure(2)),
            SupersedeReason::CheaperSameOrLargerDomain,
            Some(pr),
        )
    };
    assert_eq!(
        full(proof.clone()).unwrap_err(),
        RetireError::EngineCheckMissing
    );
    // disagreeing engine: refused, naming the key and both answers
    let mut bad = honest(&proof, "t");
    bad.positions[i5] = Some(0);
    let e = proof.clone().with_engine_check(&bad, "t").unwrap_err();
    assert!(e.contains("[5]") && e.contains("record 1"), "{e}");
    let mut gap = honest(&proof, "t");
    gap.positions[i1] = None;
    assert!(proof.clone().with_engine_check(&gap, "t").is_err());
    // another keys file, another table than the caller wrote, a missing engine name, a short answer list
    let mut k2 = honest(&proof, "t");
    k2.keys_sha256 = "other".into();
    assert!(proof.clone().with_engine_check(&k2, "t").is_err());
    assert!(
        proof
            .clone()
            .with_engine_check(&honest(&proof, "t"), "u")
            .is_err()
    );
    let mut noname = honest(&proof, "t");
    noname.engine_sha256 = String::new();
    assert!(proof.clone().with_engine_check(&noname, "t").is_err());
    let mut short = honest(&proof, "t");
    short.positions.pop();
    assert!(proof.clone().with_engine_check(&short, "t").is_err());
    // the honest run binds; the artefact digest changes; the record carries the check; a FULL retirement accepts it
    let checked = proof
        .clone()
        .with_engine_check(&honest(&proof, "t"), "t")
        .unwrap();
    assert_ne!(checked.artefact_sha256(), proof.artefact_sha256());
    let c = checked.engine_check().unwrap();
    assert_eq!((c.table_sha256.as_str(), c.keys), ("t", req.keys.len()));
    assert!(
        checked
            .clone()
            .with_engine_check(&honest(&proof, "t"), "t")
            .is_err(),
        "bound once"
    );
    let r = full(checked.clone()).unwrap();
    let rec = r.to_record();
    assert_eq!(rec.proof.as_ref().unwrap().engine_check.as_ref(), Some(c));
    assert_eq!(rec.verify(Some(checked)).unwrap(), r);
    // the engine's output format is read strictly (keys in the request's order)
    let txt: String = req
        .keys
        .iter()
        .zip(&req.expected)
        .map(|(k, p)| format!("{{\"key\":{k:?},\"position\":{p},\"label\":{p}}}\n"))
        .collect();
    let run = EngineFirstMatchRun::from_rr_node_output(&txt, &req, "engine", "t").unwrap();
    assert_eq!(run, honest(&proof, "t"));
    let swapped = txt.replacen("[1]", "[2]", 1);
    assert!(EngineFirstMatchRun::from_rr_node_output(&swapped, &req, "engine", "t").is_err());
}

// ------------------------------------------------------------------------------------------------------------------
// 5. the frozen engine (rr_node first-match), in a private pid namespace
// ------------------------------------------------------------------------------------------------------------------

fn rr_node() -> Option<PathBuf> {
    let p = std::env::var_os("RUSTRED_FROZEN_RR_NODE").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(
            "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/bin/frozen/20261114T2332Z-engine-line7a/rr_node",
        )
    });
    p.is_file().then_some(p)
}

/// `rr_node first-match --table T --keys K` (inside a private pid namespace unless this process already is pid 1 of
/// one): the answers in order.
fn engine_answers(bin: &Path, table: &Path, keys: &Path) -> String {
    let mut cmd = if std::process::id() == 1 {
        std::process::Command::new(bin)
    } else {
        let mut c = std::process::Command::new("unshare");
        c.args(["-Urpf", "--mount-proc", "--"]).arg(bin);
        c
    };
    let out = cmd
        .arg("first-match")
        .arg("--table")
        .arg(table)
        .arg("--keys")
        .arg(keys)
        .output()
        .expect("engine runs");
    assert!(
        out.status.success(),
        "engine failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn engine_positions(bin: &Path, dir: &Path, table: &Path, keys: &[Vec<i32>]) -> Vec<Option<usize>> {
    let kf = dir.join("keys.jsonl");
    std::fs::write(
        &kf,
        keys.iter()
            .map(|k| format!("{k:?}\n").replace(' ', ""))
            .collect::<String>(),
    )
    .unwrap();
    engine_answers(bin, table, &kf)
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            v["position"].as_u64().map(|x| x as usize)
        })
        .collect()
}

/// Deterministic generator (splitmix64).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
}

/// One generated record: target pattern (slot permutation, offsets, constants), fixed slots, `domain_bounds` (min on
/// line slots / max on numerator slots, incl. fixed slots), small linear exceptions.
fn gen_record(g: &mut Rng, mask: &[bool], index: usize) -> String {
    let n = mask.len();
    let ms: String = mask.iter().map(|&b| if b { '1' } else { '0' }).collect();
    let in_chart = |i: usize, g: &mut Rng| {
        if mask[i] {
            g.range(1, 4)
        } else {
            g.range(-3, 0)
        }
    };
    let fixed: Vec<Option<i64>> = (0..n)
        .map(|i| (g.below(4) == 0).then(|| in_chart(i, g)))
        .collect();
    let mut perm: Vec<usize> = (0..n).collect();
    if g.below(2) == 0 {
        for i in (1..n).rev() {
            let j = g.below(i as u64 + 1) as usize;
            perm.swap(i, j);
        }
    }
    let args: Vec<String> = (0..n)
        .map(|j| match fixed[j] {
            Some(v) if g.below(2) == 0 => v.to_string(),
            _ => {
                let o = if g.below(2) == 0 { 0 } else { g.range(-2, 3) };
                match o {
                    0 => format!("n{}", perm[j]),
                    o if o > 0 => format!("n{}+{o}", perm[j]),
                    o => format!("n{}{o}", perm[j]),
                }
            }
        })
        .collect();
    let mut min = vec!["null".to_string(); n];
    let mut max = vec!["null".to_string(); n];
    let mut any = false;
    for i in 0..n {
        if g.below(2) == 0 {
            any = true;
            if mask[i] {
                min[i] = g.range(-1, 5).to_string();
            } else {
                max[i] = g.range(-4, 1).to_string();
            }
        }
    }
    let bounds = if any {
        format!(
            r#","domain_bounds":{{"min":[{}],"max":[{}]}}"#,
            min.join(","),
            max.join(",")
        )
    } else {
        String::new()
    };
    let exc = if g.below(3) == 0 {
        let i = g.below(n as u64) as usize;
        let v = g.range(-3, 4);
        format!("[[\"{}\"]]", Poly::slot_minus(i, i128::from(v)))
    } else {
        "[]".into()
    };
    let fx: Vec<String> = fixed
        .iter()
        .map(|f| f.map_or("null".to_string(), |v| v.to_string()))
        .collect();
    format!(
        r#"{{"type":"rule","chart":"{ms}","index":{index},"fixed":[{}],"equations":[],"exceptions":{exc},"target":"I({})","rhs":[],"witness_loci":[],"dispatch":"Partition"{bounds}}}"#,
        fx.join(","),
        args.join(",")
    )
}

/// DIFFERENTIAL core vs engine on generated records WITH `domain_bounds` and target patterns (incl. bounds on fixed
/// slots and rule points outside the chart: B2 / B2b): the core dispatcher's first match equals the frozen engine's
/// at every key of a grid around the chart corner, on every generated table.
#[test]
fn pf4_differential_core_vs_frozen_engine_with_bounds_and_patterns() {
    let Some(bin) = rr_node() else {
        eprintln!("SKIPPED: no frozen rr_node (set RUSTRED_FROZEN_RR_NODE)");
        return;
    };
    let dir = scratch("diff");
    // sample size / seed: env PF4_DIFF_TABLES (default 120), PF4_DIFF_SEED
    let ntab: usize = std::env::var("PF4_DIFF_TABLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let seed: u64 = std::env::var("PF4_DIFF_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0x5eed_0000_2026_1115);
    let mut g = Rng(seed);
    let (mut tables, mut keys_total, mut bounded, mut patterned) = (0usize, 0usize, 0usize, 0usize);
    for t in 0..ntab {
        let mask: Vec<bool> = match t % 3 {
            0 => vec![true, false],
            1 => vec![true, true],
            _ => vec![true, true, false],
        };
        let ms: String = mask.iter().map(|&b| if b { '1' } else { '0' }).collect();
        let n = mask.len();
        let nrec = 2 + g.below(4) as usize;
        let mut lines = vec![header(&ms), chart(&ms)];
        for i in 0..nrec {
            let r = gen_record(&mut g, &mask, i);
            bounded += usize::from(r.contains("domain_bounds"));
            patterned += usize::from(!r.contains(&format!(
                "\"target\":\"I({})\"",
                (0..n).map(|j| format!("n{j}")).collect::<Vec<_>>().join(",")
            )));
            lines.push(r);
        }
        let p = write(&dir, &format!("t{t}.jsonl"), &lines);
        let (d, _) = match load(&p) {
            Ok(x) => x,
            Err(e) => panic!(
                "core refused generated table {t}: {e}\n{}",
                lines.join("\n")
            ),
        };
        let mut keys: Vec<Vec<i32>> = vec![vec![]];
        for &line in &mask {
            let vals: Vec<i32> = if line {
                (1..=6).collect()
            } else {
                (-5..=0).collect()
            };
            keys = keys
                .into_iter()
                .flat_map(|k| {
                    vals.iter().map(move |&v| {
                        let mut k2 = k.clone();
                        k2.push(v);
                        k2
                    })
                })
                .collect();
        }
        let eng = engine_positions(&bin, &dir, &p, &keys);
        assert_eq!(eng.len(), keys.len());
        for (k, e) in keys.iter().zip(&eng) {
            let c = match d.first_match(k).unwrap() {
                Dispatch::Covered(r) => Some(r.record),
                Dispatch::Uncovered => None,
                other => panic!(
                    "table {t} key {k:?}: core {other:?} (generated records are never blocked)"
                ),
            };
            assert_eq!(
                c,
                *e,
                "table {t} key {k:?}: core {c:?} engine {e:?}\n{}",
                lines.join("\n")
            );
        }
        tables += 1;
        keys_total += keys.len();
    }
    eprintln!(
        "DIFFERENTIAL: {tables} tables, {keys_total} keys, {bounded} bounded records, {patterned} pattern targets: core == engine"
    );
    assert!(bounded > ntab / 2 && patterned > ntab / 3);
}

/// The cross-check catches what the core misreads: a proof minted on domains WITHOUT A's bounds (as the reader of
/// B1 built them), checked by the engine against the table as SHIPPED (with the bounds): the engine has no rule at
/// keys 1, 2, so the proof retires nothing.
#[test]
fn pf4_engine_cross_check_catches_a_misread_table() {
    let Some(bin) = rr_node() else {
        eprintln!("SKIPPED: no frozen rr_node");
        return;
    };
    let dir = scratch("misread");
    let m = "1";
    let a_shipped = rule(
        m,
        0,
        "[null]",
        "I(n0)",
        "[]",
        r#","domain_bounds":{"min":[3],"max":[null]}"#,
    );
    let a_misread = rule(m, 0, "[null]", "I(n0)", "[]", "");
    let b = rule(m, 1, "[null]", "I(n0)", "[]", "");
    let (dold, f) = load(&write(
        &dir,
        "old.jsonl",
        &[header(m), chart(m), a_misread.clone(), b],
    ))
    .unwrap();
    let (dnew, _) = load(&write(&dir, "new.jsonl", &[header(m), chart(m), a_misread])).unwrap();
    let proof = prove_coverage_preserved(
        &DispatcherSource::new(&dold),
        &DispatcherSource::new(&dnew),
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
    )
    .expect("the misread set is fully covered");
    let req = proof.engine_probe().clone();
    let shipped = engine_probe_table(
        header(m).as_bytes(),
        &[chart(m).into_bytes()],
        &[a_shipped.into_bytes()],
    );
    let tp = dir.join("probe.jsonl");
    std::fs::write(&tp, &shipped).unwrap();
    let kp = dir.join("keys.jsonl");
    std::fs::write(&kp, req.keys_file()).unwrap();
    let out = engine_answers(&bin, &tp, &kp);
    let tsha = sha256_hex(&shipped);
    let run =
        EngineFirstMatchRun::from_rr_node_output(&out, &req, "frozen-rr_node", &tsha).unwrap();
    let e = proof.with_engine_check(&run, &tsha).unwrap_err();
    assert!(e.contains("[1]") || e.contains("[2]"), "{e}");
}

/// END TO END through assembly with the frozen engine: retiring the catch-all B behind A (bounds exclude 1, 2) and the
/// point records P1, P2 is proved, checked by the engine on `engine_probe_table_for` and accepted; without the engine
/// check, or with a check bound to another table, it is refused.
#[test]
fn pf4_assembly_full_retirement_needs_the_engine_check() {
    use rustred_campaign_core::accept::NodeTarget;
    use rustred_campaign_core::assemble::{
        AssemblyInput, FullRetirements, InputTable, assemble, engine_probe_table_for,
        tables_in_force,
    };
    let Some(bin) = rr_node() else {
        eprintln!("SKIPPED: no frozen rr_node");
        return;
    };
    let dir = scratch("asm");
    let m = "1";
    let a = rule(
        m,
        0,
        "[null]",
        "I(n0)",
        "[]",
        r#","domain_bounds":{"min":[3],"max":[null]}"#,
    );
    let p1r = rule(m, 1, "[1]", "I(1)", "[]", "");
    let p2r = rule(m, 2, "[2]", "I(2)", "[]", "");
    let b = rule(m, 3, "[null]", "I(n0)", "[]", "");
    let t = write(
        &dir,
        "node.table.jsonl",
        &[
            header(m),
            chart(m),
            a.clone(),
            p1r,
            p2r,
            b.clone(),
            r#"{"type":"end"}"#.into(),
        ],
    );
    let tag = |l: &str| format!("sha256:{}", sha256_hex(l.as_bytes()));
    let (va, vb) = (tag(&a), tag(&b));
    let mut inp = AssemblyInput {
        node: "SYNTH".into(),
        target: NodeTarget::Full,
        inputs: vec![InputTable {
            id: "node".into(),
            path: t.clone(),
            priority: 0,
            sha256: None,
        }],
        accepted: vec![],
        terminals: vec![],
        credits: vec![],
        retirements: vec![],
        removals: vec![],
        full_retirements: FullRetirements::BoundProof,
        keep_skipped: false,
        used: None,
        renumber_index: true,
        recipes: 0,
    };
    let f = NodeFrame::without_header_lines_first(1, 1);
    let before = tables_in_force(&inp, &[]).unwrap();
    let after = tables_in_force(&inp, std::slice::from_ref(&vb)).unwrap();
    let box_keys: Vec<Vec<i32>> = (1..=9).map(|k| vec![k]).collect();
    let proof = prove_coverage_preserved_with_box(
        &DispatcherSource::new(&before),
        &DispatcherSource::new(&after),
        &f,
        &ProofWork::default(),
        &DispatchOrderStatement::SameOrder,
        &box_keys,
    )
    .expect("A + P1 + P2 cover every key");
    let req = proof.engine_probe().clone();
    let table = engine_probe_table_for(&inp, std::slice::from_ref(&vb)).unwrap();
    let tsha = sha256_hex(&table);
    let tp = dir.join("probe.jsonl");
    std::fs::write(&tp, &table).unwrap();
    let kp = dir.join("keys.jsonl");
    std::fs::write(&kp, req.keys_file()).unwrap();
    let run = EngineFirstMatchRun::from_rr_node_output(
        &engine_answers(&bin, &tp, &kp),
        &req,
        "frozen-rr_node",
        &tsha,
    )
    .unwrap();
    let checked = proof
        .clone()
        .with_engine_check(&run, &tsha)
        .expect("the engine agrees");
    let retire = |pr| {
        Retirement::new(
            "SYNTH",
            CoverageState::Full,
            &[(vb.clone(), measure(10))],
            (&va, &measure(2)),
            SupersedeReason::CheaperSameOrLargerDomain,
            Some(pr),
        )
    };
    assert_eq!(
        retire(proof.clone()).unwrap_err(),
        RetireError::EngineCheckMissing
    );
    // bound to another table (the one WITHOUT the retirement): refused by assembly
    let other = engine_probe_table_for(&inp, &[]).unwrap();
    let osha = sha256_hex(&other);
    let mut orun = run.clone();
    orun.table_sha256 = osha.clone();
    let wrong = proof.with_engine_check(&orun, &osha).unwrap();
    inp.retirements = vec![retire(wrong).unwrap()];
    let e = assemble(&inp, &dir.join("asm_wrong")).unwrap_err();
    assert!(format!("{e:?}").contains("EngineCheckMissing"), "{e:?}");
    inp.retirements = vec![retire(checked).unwrap()];
    let out =
        assemble(&inp, &dir.join("asm_ok")).expect("assembly accepts the engine-checked proof");
    assert_eq!(out.report.retired, vec![vb]);
}
