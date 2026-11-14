//! Independent re-audit of the forward closure (lane audit-closure2, 2026-11-14): targeted reproductions.
//!
//! Tests named `fN_*` and marked `#[ignore = "AUDIT-CLOSURE2 ..."]` assert the SPECIFIED behaviour (closure.md, the
//! brief) and FAIL on campaign b5ba6b33: they are the reproductions of the findings (run with `--include-ignored`).
//! Tests named `ok_*` probe edge cases that HOLD (they pass) and stay in the default suite.
//! Small hand-built nodes only (no fixtures); arity is a parameter of every helper.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred_campaign_core::closure::{
    ClosureConfig, CoefFact, DispatchSlot, FactSource, HandDowns, KeyCodec, KeySet, NoFacts,
    RegFact, RelationSet, RelationStoreBuilder, RuleBodies, RuleId, Tier, VERDICT_CERTIFIED,
    Verdict, closure,
};
use rustred_campaign_core::cover::{Assurance, Dispatcher, RecordMeta, Table, WitnessStatus};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement};

// ------------------------------------------------------------------------------------------------ helpers

fn chart(n: usize, nl: usize) -> Vec<bool> {
    (0..n).map(|s| s < nl).collect()
}

struct Rec {
    fixed: Vec<Option<i32>>,
    body: Vec<String>,
    meta: Option<(WitnessStatus, Assurance)>,
    domain: Option<CaseDomain>,
}

fn rec_at(key: &[i32], body: &[&str]) -> Rec {
    Rec {
        fixed: key.iter().map(|&v| Some(v)).collect(),
        body: body.iter().map(|s| s.to_string()).collect(),
        meta: None,
        domain: None,
    }
}

fn rec_free(n: usize, body: &[&str]) -> Rec {
    Rec {
        fixed: vec![None; n],
        body: body.iter().map(|s| s.to_string()).collect(),
        meta: None,
        domain: None,
    }
}

/// tables: (name, records); priority = list order; labels 1.. per table.
fn node(n: usize, nl: usize, tables: Vec<(&str, Vec<Rec>)>) -> (Dispatcher, RuleBodies) {
    let mut tabs = vec![];
    for (name, recs) in &tables {
        let mut t = Table::new(name.to_string(), n);
        for (i, r) in recs.iter().enumerate() {
            let mut d = r
                .domain
                .clone()
                .unwrap_or_else(|| CaseDomain::free(n, Some(chart(n, nl))));
            d.fixed = r.fixed.clone();
            match r.meta {
                None => t.push(i as i64 + 1, d).unwrap(),
                Some((w, b)) => t
                    .push_with_meta(
                        i as i64 + 1,
                        d,
                        RecordMeta {
                            source_index: i,
                            line_no: 0,
                            witness: w,
                            witness_basis: b,
                            target_basis: Assurance::Record,
                        },
                    )
                    .unwrap(),
            }
        }
        tabs.push(t);
    }
    let prio = (0..tabs.len()).collect();
    let d = Dispatcher::new(tabs, prio).unwrap();
    let mut b = RuleBodies::new(&d);
    for (t, (_, recs)) in tables.iter().enumerate() {
        for (i, r) in recs.iter().enumerate() {
            if !r.body.is_empty() {
                b.set(&d, t, i, &r.body).unwrap();
            }
        }
    }
    (d, b)
}

fn ks(n: usize, bits: u32, keys: &[&[i32]]) -> KeySet {
    KeySet::from_keys(KeyCodec::new(n, bits).unwrap(), keys.iter().copied()).unwrap()
}

fn cfg(tier: Tier, d: &Dispatcher) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 2,
        hand_downs: HandDowns::Keys,
        table_digests: d
            .tables()
            .iter()
            .map(|t| (t.name.clone(), format!("sha256:audit-{}", t.name)))
            .collect(),
        ..Default::default()
    }
}

type CoefFn = dyn Fn(&[i32], usize, &[i32]) -> CoefFact + Sync;

/// A fact source given by closures (exact facts).
struct Facts {
    coef: Box<CoefFn>,
    reg: RegFact,
    lift: RegFact,
}
impl Facts {
    fn new(coef: impl Fn(&[i32], usize, &[i32]) -> CoefFact + Sync + 'static) -> Self {
        Facts {
            coef: Box::new(coef),
            reg: RegFact::Regular,
            lift: RegFact::Unknown,
        }
    }
}
impl FactSource for Facts {
    fn coefficient(&self, _r: &RuleId, key: &[i32], term: usize, tk: &[i32]) -> CoefFact {
        (self.coef)(key, term, tk)
    }
    fn regularity(&self, _r: &RuleId, _key: &[i32]) -> RegFact {
        self.reg
    }
    fn label(&self) -> String {
        "audit-closure2-facts".into()
    }
    fn witness_lift(&self, _r: &RuleId, _key: &[i32]) -> RegFact {
        self.lift
    }
}

fn keyset_vec(s: &KeySet) -> Vec<Vec<i32>> {
    s.iter().collect()
}

// ------------------------------------------------------------------------------------------------ F1

/// F1a (MAJOR): Tier K must be a SUPERSET of the true closure (closure.md 2; the brief). With symmetry credits the
/// two tiers choose DIFFERENT group elements: Tier K takes g1 (its positive-numerator term only "requires zero"),
/// Tier C refuses g1 (that coefficient is NonZero, exact) and credits through g2. The key reached through g2 in the
/// true (Tier C) closure is absent from the Tier K "superset".
#[test]

fn f1a_tier_k_is_a_superset_of_tier_c_with_credits() {
    let (n, nl) = (4, 3);
    let k = [1, 2, 3, 0];
    let g1k = [2, 1, 3, 0];
    let g2k = [3, 2, 1, 0];
    let (d, b) = node(
        n,
        nl,
        vec![(
            "T",
            vec![
                // R1 at g1 k: a lower top term and a positive-numerator term
                rec_at(&g1k, &["I(1,1,2,0)", "I(2,1,3,1)"]),
                // R2 at g2 k: another lower top term
                rec_at(&g2k, &["I(2,1,1,0)"]),
            ],
        )],
    );
    let cmp = NodeComparator::legacy(n, None).unwrap();
    assert!(cmp.lower(&[1, 1, 2, 0], &k) && cmp.lower(&[2, 1, 1, 0], &k));
    let pol = CreditPolicy::new(vec![
        GroupElement {
            lines: vec![1, 0, 2],
            numerators: None,
        },
        GroupElement {
            lines: vec![2, 1, 0],
            numerators: None,
        },
    ]);
    let start = ks(n, 6, &[&k]);
    let mut ck = cfg(Tier::K, &d);
    ck.credits = Some(pol.clone());
    let mut cc = cfg(Tier::C, &d);
    cc.credits = Some(pol);
    let facts = Facts::new(|_, _, tk| {
        if tk == [2, 1, 3, 1] {
            CoefFact::NonZero
        } else {
            CoefFact::Unknown
        }
    });
    let kk = closure(&d, &b, &cmp, &start, &ck, &NoFacts).unwrap();
    let kc = closure(&d, &b, &cmp, &start, &cc, &facts).unwrap();
    println!(
        "Tier K credits {:?}\nTier C credits {:?}",
        kk.credits(),
        kc.credits()
    );
    let missing = kc.keys().difference(&kk.keys());
    assert_eq!(
        keyset_vec(&missing),
        Vec::<Vec<i32>>::new(),
        "keys of the true (Tier C) closure missing from the Tier K superset"
    );
}

/// F1b (MAJOR): the same with a witness-absent record: Tier C lifts it with the exact per-key fact (closure.md 11.6a)
/// and follows its body; Tier K always blocks (no expansion), so the Tier K key set misses the lifted key's descendants.
#[test]

fn f1b_tier_k_is_a_superset_of_tier_c_with_witness_lift() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, 0];
    let mut r = rec_at(&k, &["I(1,1,1,0)"]);
    r.meta = Some((WitnessStatus::NotCarried, Assurance::Record));
    let (d, b) = node(n, nl, vec![("T", vec![r])]);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let start = ks(n, 6, &[&k]);
    let mut facts = Facts::new(|_, _, _| CoefFact::Unknown);
    facts.lift = RegFact::Regular;
    let kk = closure(&d, &b, &cmp, &start, &cfg(Tier::K, &d), &NoFacts).unwrap();
    let kc = closure(&d, &b, &cmp, &start, &cfg(Tier::C, &d), &facts).unwrap();
    println!(
        "Tier K keys {:?} open {:?}; Tier C keys {:?} lifted {:?}",
        keyset_vec(&kk.keys()),
        kk.summary().open_by_status,
        keyset_vec(&kc.keys()),
        kc.summary().witness_lifted
    );
    let missing = kc.keys().difference(&kk.keys());
    assert_eq!(keyset_vec(&missing), Vec::<Vec<i32>>::new());
}

// ------------------------------------------------------------------------------------------------ F2

/// F2 (MAJOR): "elided-witness rules are unusable" and closure.md 11 ("every option that weakens a check is recorded
/// and makes `closed` impossible"). A dispatcher whose witness check is OFF (`Assurance::CheckDisabled`, the legacy
/// T178/T266 reference setting) lets an ELIDED record serve; a record resting on an UNCHECKED attestation and a
/// dispatcher without a chart are likewise non-strict (`Dispatcher::summary().is_strict() == false`). The Tier C
/// verdict ignores all three: it is `closed` with the elided rule firing.
#[test]

fn f2_weakened_dispatch_checks_never_give_closed() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, 0];
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let facts = Facts::new(|_, _, _| CoefFact::Unknown);
    let mut verdicts = vec![];
    for (label, meta) in [
        (
            "elided+check_disabled",
            (WitnessStatus::Elided, Assurance::CheckDisabled),
        ),
        (
            "complete+attested_unchecked",
            (WitnessStatus::Complete, Assurance::AttestedUnchecked),
        ),
    ] {
        // body: one hand-down (line 0 removed): nothing to decide
        let mut r = rec_at(&k, &["I(0,1,1,0)"]);
        r.meta = Some(meta);
        let (d, b) = node(n, nl, vec![("T", vec![r])]);
        assert!(
            !d.summary().is_strict(),
            "{label}: dispatcher is not strict"
        );
        let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &cfg(Tier::C, &d), &facts)
            .unwrap()
            .summary();
        println!(
            "{label}: verdict {:?}, blocking {:?}",
            s.verdict, s.blocking
        );
        verdicts.push((label, s.verdict));
    }
    // control: the same elided record with the check ON is blocked (open)
    let mut r = rec_at(&k, &["I(0,1,1,0)"]);
    r.meta = Some((WitnessStatus::Elided, Assurance::Record));
    let (d, b) = node(n, nl, vec![("T", vec![r])]);
    let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &cfg(Tier::C, &d), &facts)
        .unwrap()
        .summary();
    assert_eq!(
        s.verdict,
        Verdict::Open,
        "control: elided with the check on"
    );
    for (label, v) in verdicts {
        assert_ne!(
            v,
            Verdict::Closed,
            "{label}: a weakened dispatch check gave `closed`"
        );
    }
}

// ------------------------------------------------------------------------------------------------ F3

/// F3 (MINOR): `longest_chain` is outside the configuration identity ("adds the chain pass only", closure.md 11.2) but
/// changes the VERDICT: a Tier C rule whose right-hand side contains its own key with an UNKNOWN coefficient is
/// `undecided` without the chain pass and `failed` (cycles: 1) with it, under the same config digest.
#[test]

fn f3_longest_chain_never_changes_the_verdict() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, 0];
    let (d, b) = node(
        n,
        nl,
        vec![("T", vec![rec_at(&k, &["I(n0,n1,n2,n3)", "I(0,1,1,0)"])])],
    );
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let facts = Facts::new(|_, _, _| CoefFact::Unknown);
    let mut on = cfg(Tier::C, &d);
    on.longest_chain = true;
    let mut off = on.clone();
    off.longest_chain = false;
    let a = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &on, &facts)
        .unwrap()
        .summary();
    let z = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &off, &facts)
        .unwrap()
        .summary();
    println!(
        "chain on: {:?} {:?} cycles {:?}; chain off: {:?} {:?}",
        a.verdict, a.blocking, a.cycles, z.verdict, z.blocking
    );
    assert_eq!(a.config_digest, z.config_digest);
    assert_eq!(a.verdict, z.verdict, "same identity, different verdict");
}

// ------------------------------------------------------------------------------------------------ F4

/// F4 (MAJOR for genericity, L8): arity is a parameter; the key codec accepts up to 128 slots (4 bits x 8 words) and
/// `run` does not refuse arity > 64, but term classification uses u64 masks (`1 << slot`). A relation set at arity 70
/// whose recorded right-hand side removes line 65: debug build panics (shift overflow), release build aliases the
/// child sector to line 1 and, with sector "line 1 removed" declared zero, DROPS the term as zero. Expected: a typed
/// error for arity > 64, or a correct classification (subsector, never zero_sector).
#[test]

fn f4_arity_above_64_is_refused_or_classified_correctly() {
    let (n, nl) = (70usize, 66usize);
    let lines = chart(n, nl);
    let t = Table::new("T", n);
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let b = RuleBodies::new(&d);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let codec = KeyCodec::new(n, 4).unwrap();
    let k: Vec<i32> = (0..n).map(|s| if s < nl { 1 } else { 0 }).collect();
    let mut rhs = k.clone();
    rhs[65] = 0; // line 65 removed: a hand-down to child sector {65}
    let mut sb = RelationStoreBuilder::new(codec);
    sb.push(&k, &[rhs], [1; 32], VERDICT_CERTIFIED).unwrap();
    let set = RelationSet {
        name: "recipe".into(),
        recipe_digest: "sha256:r".into(),
        domain: CaseDomain::free(n, Some(lines.clone())),
        relations: sb.finish().unwrap(),
        slot: DispatchSlot::Last,
    };
    let c = ClosureConfig {
        tier: Tier::K,
        threads: 1,
        bits: 4,
        lines: Some(lines),
        hand_downs: HandDowns::Keys,
        zero_sectors: Some(BTreeSet::from([1u64 << 1])),
        zero_sector_source: Some("audit: line 1 removed is scaleless".into()),
        relation_sets: vec![Arc::new(set)],
        ..Default::default()
    };
    let start = KeySet::from_keys(codec, [k.as_slice()]).unwrap();
    let r = std::panic::catch_unwind(|| closure(&d, &b, &cmp, &start, &c, &NoFacts));
    match r {
        Err(_) => panic!("closure panicked at arity {n} (shift overflow)"),
        Ok(Err(e)) => println!("typed error (acceptable): {e:?}"),
        Ok(Ok(cl)) => {
            let s = cl.summary();
            println!("terms {:?}, hand_downs {:?}", s.terms, s.hand_downs);
            assert_eq!(
                s.terms.zero_sector, 0,
                "a live hand-down was dropped as zero"
            );
            assert_eq!(s.terms.subsector, 1);
        }
    }
}

// ------------------------------------------------------------------------------------------------ F5

/// F5 (MAJOR, memory at T266 scale): a Tier C run keeps ONE heap-allocated request per Unknown term (`ReqC` with a
/// boxed term key) and `summary()` (also called by `report()`) materialises, sorts and de-duplicates EVERY
/// `FactRequest` (strings: table path, rule digest, config digest; two key vectors) just to count them. On a node with
/// P positive-numerator terms and no facts yet (the first Tier C run of the batch protocol, closure.md 7) memory grows
/// by ~P x (in-run request + transient FactRequest). Measured here per request; the assertion encodes a budget of
/// 64 bytes per request for the transient of `summary()` (the count needs no allocation at all).
#[test]

fn f5_summary_does_not_materialise_the_fact_requests() {
    // arity 12: 8 lines, 4 numerators; one free rule: each of lines 0..3 down by one (4 top terms) and each numerator
    // raised by one (4 positive-numerator terms, Unknown coefficient -> 4 requests per key)
    let (n, nl) = (12usize, 8usize);
    let pat = |slot: usize, off: &str| {
        format!(
            "I({})",
            (0..n)
                .map(|s| if s == slot {
                    format!("n{s}{off}")
                } else {
                    format!("n{s}")
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let mut body: Vec<String> = (0..4).map(|l| pat(l, "-1")).collect();
    body.extend((nl..n).map(|s| pat(s, "+1")));
    let br: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
    let (d, b) = node(n, nl, vec![("T", vec![rec_free(n, &br)])]);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let mut k = vec![1; nl];
    for v in k.iter_mut().take(4) {
        *v = 20;
    }
    k.extend(std::iter::repeat_n(0, n - nl));
    let start = ks(n, 6, &[&k]);
    let mut c = cfg(Tier::C, &d);
    c.longest_chain = false;
    c.threads = 4;
    let facts = Facts::new(|_, _, _| CoefFact::Unknown);
    let hwm_a = hwm();
    let cl = closure(&d, &b, &cmp, &start, &c, &facts).unwrap();
    let hwm_b = hwm();
    let t0 = std::time::Instant::now();
    let s = cl.summary();
    let dt = t0.elapsed();
    let hwm_c = hwm();
    let nreq = s.fact_requests.max(1) as f64;
    let in_run = hwm_b.saturating_sub(hwm_a) as f64 / nreq;
    let transient = hwm_c.saturating_sub(hwm_b) as f64 / nreq;
    println!(
        "keys {}, fact requests {} (undecided {:?}); closure peak +{} MB ({in_run:.0} B/request incl. the key table), \
         summary() {:?} peak +{} MB ({transient:.0} B/request transient)",
        s.keys_total,
        s.fact_requests,
        s.undecided,
        (hwm_b.saturating_sub(hwm_a)) >> 20,
        dt,
        (hwm_c.saturating_sub(hwm_b)) >> 20,
    );
    assert_eq!(s.keys_total, 160_000);
    assert!(
        transient < 64.0,
        "summary() allocated {transient:.0} bytes per fact request"
    );
}

fn hwm() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM:")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap() * 1024
}

// ------------------------------------------------------------------------------------------------ checks that hold

/// A rule whose right-hand side contains its OWN key: Tier K counts a descent violation and a cycle (failed); Tier C
/// with an exact ZERO coefficient for that term drops it (closed); with NonZero it fails.
#[test]
fn ok_rule_rhs_contains_its_own_key() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, 0];
    let (d, b) = node(
        n,
        nl,
        vec![("T", vec![rec_at(&k, &["I(n0,n1,n2,n3)", "I(0,1,1,0)"])])],
    );
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let start = ks(n, 6, &[&k]);
    let sk = closure(&d, &b, &cmp, &start, &cfg(Tier::K, &d), &NoFacts)
        .unwrap()
        .summary();
    assert_eq!(sk.verdict, Verdict::Failed);
    assert_eq!(sk.descent_violations, 1);
    assert_eq!(sk.cycles, Some(1));
    let zero = Facts::new(|key, _, tk| {
        if tk == key {
            CoefFact::Zero
        } else {
            CoefFact::Unknown
        }
    });
    let sz = closure(&d, &b, &cmp, &start, &cfg(Tier::C, &d), &zero)
        .unwrap()
        .summary();
    assert_eq!(sz.verdict, Verdict::Closed, "{:?}", sz.blocking);
    assert_eq!(sz.terms.dropped_zero, 1);
    let nz = Facts::new(|key, _, tk| {
        if tk == key {
            CoefFact::NonZero
        } else {
            CoefFact::Unknown
        }
    });
    let sn = closure(&d, &b, &cmp, &start, &cfg(Tier::C, &d), &nz)
        .unwrap()
        .summary();
    assert_eq!(sn.verdict, Verdict::Failed);
}

/// Codec boundary: a key with a slot at the top of the codec range whose rule raises that slot: the term is
/// unrepresentable (counted, failed), never wrapped; a hand-down out of range is counted likewise.
#[test]
fn ok_codec_boundary_never_wraps() {
    let (n, nl) = (4, 3);
    for bits in [4u32, 5, 9] {
        let max = (1i32 << (bits - 1)) - 1;
        let k = [max, 1, 1, 0];
        // n1+1 at slot 1 is fine; n0+1 leaves the range; the term is still LOWER? no: it is higher -> violation too
        let (d, b) = node(
            n,
            nl,
            vec![("T", vec![rec_at(&k, &["I(n0+1,n1-1,n2,n3-1)"])])],
        );
        let cmp = NodeComparator::legacy(n, None).unwrap();
        let mut c = cfg(Tier::K, &d);
        c.bits = bits;
        let s = closure(&d, &b, &cmp, &ks(n, bits, &[&k]), &c, &NoFacts)
            .unwrap()
            .summary();
        assert_eq!(s.unrepresentable, 1, "bits {bits}");
        assert_eq!(s.keys_total, 1, "bits {bits}: no wrapped key reached");
        assert_eq!(s.verdict, Verdict::Failed);
    }
}

/// Domain boundaries: a key exactly ON an exception locus is NotSelected (the dispatch falls through to the next
/// record), a key exactly on a bound is inside, one unit beyond is outside; a key on a recenter bound / pole is
/// Blocked (open, no fall-through).
#[test]
fn ok_domain_boundaries_and_exception_branches() {
    let (n, nl) = (4, 3);
    let lines = chart(n, nl);
    // R1: free with exception n0 = n1 (so (2,2,1,0) falls through to R2), bound n0 - 2 >= 0
    let mut d1 = CaseDomain::free(n, Some(lines.clone()));
    d1.exceptions = vec![vec![Poly::parse_d_free("n0-n1", n).unwrap()]];
    d1.bounds = vec![Poly::parse_d_free("n0-2", n).unwrap()];
    let mut r1 = rec_free(n, &["I(n0-1,n1,n2,n3)"]);
    r1.domain = Some(d1);
    // R2: free, pole locus at n2 = 3 (blocked there)
    let mut d2 = CaseDomain::free(n, Some(lines.clone()));
    d2.poles = vec![rustred_campaign_core::domain::PoleLocus::new(
        rustred_campaign_core::domain::PoleKind::Denominator,
        &Poly::parse_d_free("n2-3", n).unwrap(),
    )];
    let mut r2 = rec_free(n, &["I(n0,n1,n2,n3-1)"]);
    r2.domain = Some(d2);
    let (d, b) = node(n, nl, vec![("T", vec![r1, r2])]);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let c = cfg(Tier::K, &d);
    let one = |k: &[i32]| {
        let cl = closure(&d, &b, &cmp, &ks(n, 6, &[k]), &c, &NoFacts).unwrap();
        (cl.winner(0), cl.is_open(0))
    };
    // on the exception: R1 not selected, R2 serves
    let (w, _) = one(&[2, 2, 1, 0]);
    assert_eq!(w.unwrap().label, 2);
    // exactly on the bound n0 = 2, off the exception: R1
    let (w, _) = one(&[2, 1, 1, 0]);
    assert_eq!(w.unwrap().label, 1);
    // one below the bound (n0 = 1): R1 not selected -> R2
    let (w, _) = one(&[1, 2, 1, 0]);
    assert_eq!(w.unwrap().label, 2);
    // on R2's pole with R1 not selected: blocked, open (never falls through to "uncovered")
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&[1, 2, 3, 0]]), &c, &NoFacts).unwrap();
    assert!(cl.is_open(0));
    assert_eq!(
        format!("{:?}", cl.open_keys()[0].status),
        "Blocked",
        "{:?}",
        cl.open_keys()
    );
}

/// Credits: (i) the DIRECT comparison with the ORIGINAL key decides (a term below the image but not below k is
/// refused; a term below k but not below the image is accepted); (ii) never chained (an image that is itself only
/// creditable is refused); (iii) an Elided image record is refused even with a lift fact; (iv) a witness-absent image
/// is refused even when Tier C could lift it.
#[test]
fn ok_credit_direct_comparison_and_no_chaining() {
    let (n, nl) = (4, 3);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let swap01 = CreditPolicy::new(vec![GroupElement {
        lines: vec![1, 0, 2],
        numerators: None,
    }]);
    // (i-a) k = (2,1,2,0), image (1,2,2,0): terms below the image but NOT below k must refuse the credit
    let k = [2, 1, 2, 0];
    let gk = [1, 2, 2, 0];
    // find a term strictly below gk but NOT below k (exists by the legacy tie order: (2,1,2,0) < (1,2,2,0))
    let cand: Vec<[i32; 4]> = (1..4)
        .flat_map(|a| (1..4).flat_map(move |b2| (1..4).map(move |c| [a, b2, c, 0])))
        .filter(|t| cmp.lower(t, &gk) && !cmp.lower(t, &k) && *t != k)
        .collect();
    let mut checked = 0;
    for t in cand.iter().take(3) {
        let body = format!("I({},{},{},{})", t[0], t[1], t[2], t[3]);
        let (d, b) = node(n, nl, vec![("T", vec![rec_at(&gk, &[&body])])]);
        let mut c = cfg(Tier::K, &d);
        c.credits = Some(swap01.clone());
        let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &c, &NoFacts).unwrap();
        assert!(
            !cl.is_credited(0),
            "credited through a term not below k: {t:?}"
        );
        checked += 1;
    }
    // (i-b) a term below k but not below the image: accepted
    let k2 = [1, 2, 2, 0];
    let gk2 = [2, 1, 2, 0];
    let cand2: Vec<[i32; 4]> = (1..4)
        .flat_map(|a| (1..4).flat_map(move |b2| (1..4).map(move |c| [a, b2, c, 0])))
        .filter(|t| cmp.lower(t, &k2) && !cmp.lower(t, &gk2))
        .collect();
    for t in cand2.iter().take(3) {
        let body = format!("I({},{},{},{})", t[0], t[1], t[2], t[3]);
        let (d, b) = node(n, nl, vec![("T", vec![rec_at(&gk2, &[&body])])]);
        let mut c = cfg(Tier::K, &d);
        c.credits = Some(swap01.clone());
        let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k2]), &c, &NoFacts).unwrap();
        assert!(cl.is_credited(0), "direct comparison refused {t:?}");
        checked += 1;
    }
    assert!(checked >= 2, "no candidate terms found");
    // (ii) never chained: k3 -> image g k3 is uncovered but creditable through g again? g is an involution, so use a
    // 3-cycle: lines [1,2,0]; only g^2 k3 is covered; the image g k3 is not covered -> refused, although g k3 would
    // itself be credited through its own image g (g k3) = g^2 k3.
    let cyc = CreditPolicy::new(vec![GroupElement {
        lines: vec![1, 2, 0],
        numerators: None,
    }]);
    let k3 = [1, 2, 3, 0];
    let g = |k: &[i32]| -> Vec<i32> { vec![k[2], k[0], k[1], k[3]] };
    let g2k3 = g(&g(&k3));
    let (d, b) = node(n, nl, vec![("T", vec![rec_at(&g2k3, &["I(1,1,1,0)"])])]);
    let mut c = cfg(Tier::K, &d);
    c.credits = Some(cyc);
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k3, &g(&k3)]), &c, &NoFacts).unwrap();
    let i_k3 = cl.index_of(&k3).unwrap();
    let i_gk3 = cl.index_of(&g(&k3)).unwrap();
    assert!(
        cl.is_credited(i_gk3),
        "g k3 is credited through g^2 k3 (one step)"
    );
    assert!(
        !cl.is_credited(i_k3),
        "k3 must not be credited through a credited image (chain)"
    );
    // (iii) Elided image record (check on): refused even with lift facts in Tier C
    let mut r = rec_at(&gk2, &["I(1,1,1,0)"]);
    r.meta = Some((WitnessStatus::Elided, Assurance::Record));
    let (d, b) = node(n, nl, vec![("T", vec![r])]);
    let mut facts = Facts::new(|_, _, _| CoefFact::Unknown);
    facts.lift = RegFact::Regular;
    let mut c = cfg(Tier::C, &d);
    c.credits = Some(swap01.clone());
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k2]), &c, &facts).unwrap();
    assert!(!cl.is_credited(0) && cl.is_open(0));
    // the elided record itself: never lifted
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&gk2]), &c, &facts).unwrap();
    assert!(cl.is_open(0), "elided record lifted");
    // (iv) witness-absent image: refused even when liftable
    let mut r = rec_at(&gk2, &["I(1,1,1,0)"]);
    r.meta = Some((WitnessStatus::NotCarried, Assurance::Record));
    let (d, b) = node(n, nl, vec![("T", vec![r])]);
    let mut c = cfg(Tier::C, &d);
    c.credits = Some(swap01);
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k2, &gk2]), &c, &facts).unwrap();
    assert!(!cl.is_credited(cl.index_of(&k2).unwrap()));
    assert_eq!(cl.summary().witness_lifted.values().sum::<u64>(), 1);
}

/// Relation sets: a certified key OUTSIDE the set's domain falls through to the table rule; a key in the domain but
/// not certified falls through; a set placed before a MIDDLE table serves only keys whose table first match is at or
/// after that table; an excluded set falls through.
#[test]
fn ok_recipe_match_needs_domain_and_certified_and_respects_its_slot() {
    use rustred_campaign_core::closure::{ExclusionReason, KeyExclusion};
    let (n, nl) = (4, 3);
    let lines = chart(n, nl);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let a = [2, 1, 1, 0]; // A covers it (first table)
    let bkey = [1, 2, 1, 0]; // B covers it (second table)
    let c_out = [1, 1, 2, 0]; // certified, but outside the set's domain (n2 = 2 excluded by fixed n2 = 1)
    let (d, b) = node(
        n,
        nl,
        vec![
            ("A", vec![rec_at(&a, &["I(1,1,1,0)"])]),
            (
                "B",
                vec![
                    rec_at(&bkey, &["I(1,1,1,0)"]),
                    rec_at(&c_out, &["I(1,1,1,0)"]),
                ],
            ),
        ],
    );
    let mut dom = CaseDomain::free(n, Some(lines.clone()));
    dom.fixed[2] = Some(1);
    let mut sb = RelationStoreBuilder::new(KeyCodec::new(n, 6).unwrap());
    for k in [a, bkey, c_out] {
        sb.push(&k, &[vec![1, 1, 1, 0]], [9; 32], VERDICT_CERTIFIED)
            .unwrap();
    }
    let set = Arc::new(RelationSet {
        name: "S".into(),
        recipe_digest: "sha256:s".into(),
        domain: dom,
        relations: sb.finish().unwrap(),
        slot: DispatchSlot::BeforeTable("B".into()),
    });
    let mut c = cfg(Tier::K, &d);
    c.relation_sets = vec![set];
    let start = ks(n, 6, &[&a, &bkey, &c_out, &[1, 1, 1, 0]]);
    let cl = closure(&d, &b, &cmp, &start, &c, &NoFacts).unwrap();
    let w = |k: &[i32]| cl.winner(cl.index_of(k).unwrap()).unwrap().table;
    assert_eq!(w(&a), "A", "A precedes the set");
    assert_eq!(w(&bkey), "S", "the set precedes B");
    assert_eq!(
        w(&c_out),
        "B",
        "certified but outside the domain: falls through"
    );
    // in domain, not certified: (1,1,1,0) is uncovered by the tables and not certified -> open
    assert!(cl.is_open(cl.index_of(&[1, 1, 1, 0]).unwrap()));
    // excluded set at bkey: falls through to B
    c.exclusions = vec![KeyExclusion {
        key: bkey.to_vec(),
        table: "S".into(),
        record: 0,
        label: 0,
        reason: ExclusionReason::Other("audit".into()),
    }];
    let cl = closure(&d, &b, &cmp, &start, &c, &NoFacts).unwrap();
    assert_eq!(cl.winner(cl.index_of(&bkey).unwrap()).unwrap().table, "B");
}

/// Arity is a parameter: the same small node shape at arities 3, 7, 9, 27 and 64 (bits 4..16) closes with the
/// expected key counts (a staircase below the start key) and identical results on 1 and 5 threads.
#[test]
fn ok_arity_parameter_and_thread_independence() {
    for &(n, nl, bits) in &[
        (3usize, 2usize, 4u32),
        (7, 4, 8),
        (9, 6, 5),
        (27, 21, 6),
        (64, 40, 8),
    ] {
        let body: Vec<String> = (0..2)
            .map(|l| {
                format!(
                    "I({})",
                    (0..n)
                        .map(|s| if s == l {
                            format!("n{s}-1")
                        } else {
                            format!("n{s}")
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        let br: Vec<&str> = body.iter().map(|s| s.as_str()).collect();
        let (d, b) = node(n, nl, vec![("T", vec![rec_free(n, &br)])]);
        let cmp = NodeComparator::legacy(n, None).unwrap();
        let mut k = vec![1; nl];
        k[0] = 3;
        k[1] = 4;
        k.extend(std::iter::repeat_n(0, n - nl));
        let mut c = cfg(Tier::K, &d);
        c.bits = bits;
        c.threads = 1;
        let a = closure(&d, &b, &cmp, &ks(n, bits, &[&k]), &c, &NoFacts).unwrap();
        c.threads = 5;
        let z = closure(&d, &b, &cmp, &ks(n, bits, &[&k]), &c, &NoFacts).unwrap();
        let s = a.summary();
        // keys (a, b) with 1 <= a <= 3, 1 <= b <= 4: 12
        assert_eq!(s.keys_total, 12, "arity {n}");
        assert_eq!(s.descent_violations, 0);
        assert_eq!(
            serde_json::to_string(&a.report(&Default::default())).unwrap(),
            serde_json::to_string(&z.report(&Default::default())).unwrap(),
            "arity {n}: threads changed the report"
        );
    }
}

// ------------------------------------------------------------------------------------------------ F7

/// F7 (MAJOR): `closure_recheck` reuses every earlier disposition when the configuration identity is unchanged, but
/// the identity binds the records' WITNESS STATUS only through the per-table COUNTS of `Dispatcher::summary()` (the
/// table dispatch digest covers labels and domains, not `RecordMeta`). Swapping the statuses of two records (one
/// NotCarried, one Elided) keeps every count: the recheck accepts, keeps the old LIFTED key served (Tier C `closed`),
/// while the closure of the union under the new dispatcher has that key blocked by `Elided` (`open`).
/// (`closure_incremental` is not affected: it re-evaluates the stopping record; checked here as the control.)
#[test]

fn f7_recheck_refuses_or_matches_after_a_witness_status_change() {
    use rustred_campaign_core::closure::{closure_incremental, closure_recheck};
    let (n, nl) = (4, 3);
    let k1 = [2, 1, 1, 0];
    let k2 = [1, 2, 1, 0];
    let mk = |w1: WitnessStatus, w2: WitnessStatus| {
        let mut r1 = rec_at(&k1, &["I(0,1,1,0)"]);
        r1.meta = Some((w1, Assurance::Record));
        let mut r2 = rec_at(&k2, &["I(1,0,1,0)"]);
        r2.meta = Some((w2, Assurance::Record));
        node(n, nl, vec![("T", vec![r1, r2])])
    };
    let (d0, b0) = mk(WitnessStatus::NotCarried, WitnessStatus::Elided);
    let (d1, b1) = mk(WitnessStatus::Elided, WitnessStatus::NotCarried);
    assert_eq!(format!("{:?}", d0.summary()), format!("{:?}", d1.summary()));
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let mut facts = Facts::new(|_, _, _| CoefFact::Unknown);
    facts.lift = RegFact::Regular;
    let c0 = cfg(Tier::C, &d0);
    let prev = closure(&d0, &b0, &cmp, &ks(n, 6, &[&k1]), &c0, &facts).unwrap();
    assert_eq!(
        prev.summary().verdict,
        Verdict::Closed,
        "{:?}",
        prev.summary().blocking
    );
    let c1 = cfg(Tier::C, &d1);
    let union = closure(&d1, &b1, &cmp, &ks(n, 6, &[&k1, &k2]), &c1, &facts).unwrap();
    let inc = closure_incremental(
        &prev.dispatch_cache(),
        &d1,
        &b1,
        &cmp,
        &ks(n, 6, &[&k1, &k2]),
        &c1,
        &facts,
    )
    .unwrap();
    assert_eq!(
        inc.summary().verdict,
        union.summary().verdict,
        "control: incremental"
    );
    match closure_recheck(&prev, &d1, &b1, &cmp, &ks(n, 6, &[&k2]), &c1, &facts) {
        Err(e) => println!("refused (acceptable): {e:?}"),
        Ok(re) => {
            println!(
                "recheck: {:?} {:?}; union: {:?} {:?}",
                re.summary().verdict,
                re.summary().blocking,
                union.summary().verdict,
                union.summary().blocking
            );
            assert_eq!(
                re.summary().verdict,
                union.summary().verdict,
                "stale recheck"
            );
        }
    }
}

/// Audit loop-A C2 / C3 closed: a Tier C credit through an image whose rule has a POLE there is refused (the credit
/// takes the closure's tier); the diagnostic relaxation of the regularity check stamps the run `diagnostic`.
#[test]
fn ok_audit_a_c2_c3_closed() {
    let (n, nl) = (4, 3);
    let k = [1, 2, 1, 0];
    let gk = [2, 1, 1, 0];
    let (d, b) = node(n, nl, vec![("T", vec![rec_at(&gk, &["I(0,1,1,0)"])])]);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let mut c = cfg(Tier::C, &d);
    c.credits = Some(CreditPolicy::new(vec![GroupElement {
        lines: vec![1, 0, 2],
        numerators: None,
    }]));
    let mut pole = Facts::new(|_, _, _| CoefFact::Unknown);
    pole.reg = RegFact::Pole;
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &c, &pole).unwrap();
    assert!(!cl.is_credited(0) && cl.is_open(0), "credit through a pole");
    let mut unknown = Facts::new(|_, _, _| CoefFact::Unknown);
    unknown.reg = RegFact::Unknown;
    let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &c, &unknown)
        .unwrap()
        .summary();
    // a hand-built credit group stamps the run diagnostic (lane closurefix); underneath: undecided (pending credit)
    assert_eq!(s.verdict, Verdict::Diagnostic);
    assert_eq!(
        s.verdict_if_not_diagnostic,
        Some(Verdict::Undecided),
        "pending credit: {:?}",
        s.blocking
    );
    c.diagnostic.allow_unknown_regularity = true;
    let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &c, &unknown)
        .unwrap()
        .summary();
    assert_eq!(s.verdict, Verdict::Diagnostic);
    c.diagnostic.allow_unknown_regularity = false;
    c.require_regularity = false;
    let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &c, &unknown)
        .unwrap()
        .summary();
    assert_eq!(s.verdict, Verdict::Diagnostic);
}

// ------------------------------------------------------------------------------------------------ nits

/// N1 (NIT): `ClosureSummary.open_by_status` keys are the Debug names lowercased ("outsidechart", "nobody",
/// "recipeunverified") while `OpenKey.status` serialises snake_case ("outside_chart", "no_body", "recipe_unverified"):
/// a consumer joining the two misses every multi-word status.
#[test]

fn n1_open_by_status_uses_the_serde_names() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, 0];
    let (d, b) = node(n, nl, vec![("T", vec![rec_at(&k, &[])])]); // no body: open `no_body`
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &cfg(Tier::K, &d), &NoFacts).unwrap();
    let ser = serde_json::to_value(&cl.open_keys()[0]).unwrap()["status"].clone();
    let keys: Vec<String> = cl.summary().open_by_status.keys().cloned().collect();
    println!("open_keys status {ser}, open_by_status keys {keys:?}");
    assert_eq!(keys, vec![ser.as_str().unwrap().to_string()]);
}

/// N2 (NIT, statistics only): a positive numerator produced by an OFFSET on a FIXED numerator slot (canonical target,
/// fixed n3 = -1, body `n3+2`) is counted `positive_numerator_symbolic`, although the slot is fixed (a constant of the
/// rule), not "a symbolic slot raised past zero" (closure.md 1, term class 1).
#[test]

fn n2_fixed_slot_offset_is_not_symbolic() {
    let (n, nl) = (4, 3);
    let k = [2, 1, 1, -1];
    let (d, b) = node(
        n,
        nl,
        vec![("T", vec![rec_at(&k, &["I(n0,n1,n2,n3+2)", "I(0,1,1,-1)"])])],
    );
    let cmp = NodeComparator::legacy(n, None).unwrap();
    let s = closure(&d, &b, &cmp, &ks(n, 6, &[&k]), &cfg(Tier::K, &d), &NoFacts)
        .unwrap()
        .summary();
    println!("terms {:?}", s.terms);
    assert_eq!(s.terms.positive_numerator_constant, 1);
}

// ------------------------------------------------------------------------------------------------ F8

/// F8 (MINOR, completeness, fail closed): R44 "the dispatch condition ranges over ALL accepted rules of the node
/// (table and new)" and closure.md 12 ("Credits may point to a recipe-served image under the same adopted condition")
/// against closure.md 12 "NOT YET ... REFUSED". A key whose image is served by a CERTIFIED relation set whose recorded
/// right-hand side lies strictly below the ORIGINAL key is refused (`ImageServedByRecipe`) and stays open.
#[test]

fn f8_credit_through_a_recipe_served_image() {
    let (n, nl) = (4, 3);
    let k = [1, 2, 1, 0];
    let gk = [2, 1, 1, 0];
    let low = [1, 1, 1, 0];
    let (d, b) = node(n, nl, vec![("T", vec![rec_at(&low, &["I(0,1,1,0)"])])]);
    let cmp = NodeComparator::legacy(n, None).unwrap();
    assert!(cmp.lower(&low, &k));
    let mut sb = RelationStoreBuilder::new(KeyCodec::new(n, 6).unwrap());
    sb.push(&gk, &[low.to_vec()], [3; 32], VERDICT_CERTIFIED)
        .unwrap();
    let set = Arc::new(RelationSet {
        name: "S".into(),
        recipe_digest: "sha256:s".into(),
        domain: CaseDomain::free(n, Some(chart(n, nl))),
        relations: sb.finish().unwrap(),
        slot: DispatchSlot::Last,
    });
    let mut c = cfg(Tier::K, &d);
    c.relation_sets = vec![set];
    c.credits = Some(CreditPolicy::new(vec![GroupElement {
        lines: vec![1, 0, 2],
        numerators: None,
    }]));
    let cl = closure(&d, &b, &cmp, &ks(n, 6, &[&k, &gk]), &c, &NoFacts).unwrap();
    assert!(
        cl.is_recipe_served(cl.index_of(&gk).unwrap()),
        "the image is recipe-served"
    );
    let i = cl.index_of(&k).unwrap();
    println!(
        "k open: {}, credited: {}, open keys {:?}",
        cl.is_open(i),
        cl.is_credited(i),
        cl.open_keys()
    );
    assert!(
        cl.is_credited(i),
        "R44: credit through an accepted (recipe) rule at the image"
    );
}
