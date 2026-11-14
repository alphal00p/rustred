//! Independent differential property test of the incremental closure (lane audit-closure2, 2026-11-14).
//!
//! INVARIANT under test (closure.md 11.4, the brief): after ANY change of rules, terminals, exclusions or order,
//! `closure_incremental(prev.dispatch_cache(), ..)` equals `closure(..)` from scratch EXACTLY (whole report, fact
//! requests, summary, used rules, credits, exits); chained (cache of an incremental run) and from the scratch cache;
//! `closure_recheck(prev, extra)` equals the closure of the union (order-free view); the thread count never changes a
//! result. Written without the authors' harness, and wider than it: arity drawn per case (4..9), domains with
//! equations, bounds, exception branches, recenter bounds, pole loci and offset target patterns; witness statuses
//! (complete, elided, not carried, check disabled, unchecked attestation) mutated WITHOUT a domain change (the table
//! dispatch digest does not cover them); records inserted mid-table; tables renamed; relation sets before ANY table
//! or last, with partial verdict bits and set exclusions; chart on/off; comparator tie order; tier, credits,
//! hand-down mode, zero sectors and round limits toggled; facts include witness lifts. Seeded; default suite.
//! `AUDIT_CLOSURE2_CASES` sets the number of cases (default 40, 8 steps each).

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rustred_campaign_core::closure::{
    Closure, ClosureConfig, CoefFact, DispatchSlot, ExclusionReason, FactSource, HandDowns, KeyCodec,
    KeyExclusion, KeySet, RegFact, RelationSet, RelationStoreBuilder, RuleBodies, RuleId,
    TerminalSets, Tier, closure, closure_incremental, closure_recheck,
};
use rustred_campaign_core::cover::{Assurance, Dispatcher, RecordMeta, Table, WitnessStatus};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus, TargetMap, TargetSlot};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 11
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn p(&mut self, num: usize, den: usize) -> bool {
        self.below(den) < num
    }
}

#[derive(Clone, Debug)]
struct Rec {
    label: i64,
    fixed: Vec<Option<i32>>,
    equation: Option<String>,
    bound: Option<String>,
    exception: Option<Vec<String>>,
    recenter: Option<String>,
    pole: Option<String>,
    /// offset target on one slot (canonical otherwise)
    target: Option<(usize, i64)>,
    witness: u8,
    body: Vec<String>,
}

#[derive(Clone, Debug)]
struct Set {
    name: String,
    slot: Option<String>,
    fixed: Vec<Option<i32>>,
    keys: Vec<(Vec<i32>, u8)>,
}

#[derive(Clone, Debug)]
struct St {
    n: usize,
    nl: usize,
    tables: Vec<(String, Vec<Rec>)>,
    prio: Vec<usize>,
    chart: bool,
    ties: bool,
    pref: Vec<Vec<i32>>,
    other: Vec<Vec<i32>>,
    excl: Vec<(Vec<i32>, String, i64)>,
    start: Vec<Vec<i32>>,
    credits: bool,
    tier: Tier,
    sets: Vec<Set>,
    hand_downs: u8,
    zero: bool,
    max_rounds: Option<u32>,
}

fn lines(st: &St) -> Vec<bool> {
    (0..st.n).map(|s| s < st.nl).collect()
}

fn rkey(r: &mut Rng, st: &St) -> Vec<i32> {
    (0..st.n)
        .map(|s| if s < st.nl { 1 + r.below(3) as i32 } else { -(r.below(2) as i32) })
        .collect()
}

fn rpat(r: &mut Rng, n: usize, nl: usize) -> String {
    let mut a: Vec<String> = (0..n).map(|s| format!("n{s}")).collect();
    let l = r.below(nl);
    let m = (l + 1 + r.below(nl - 1)) % nl;
    let u = nl + r.below(n - nl);
    match r.below(12) {
        0..=3 => a[l] = format!("n{l}-1"),
        4 => a[l] = "0".into(),
        5 => {
            a[u] = format!("n{u}+1");
            a[l] = format!("n{l}-1");
        }
        6 => {
            a[u] = format!("n{u}-1");
            a[l] = format!("n{l}-1");
        }
        7 => {
            a[l] = format!("n{l}+1");
            a[m] = format!("n{m}-1");
        }
        8 => a[l] = format!("n{l}-2"),
        9 => {} // the rule point itself (a self term)
        10 => a[l] = "1".into(),
        _ => {
            a[l] = format!("n{m}");
            a[m] = format!("n{l}-1");
        }
    }
    format!("I({})", a.join(","))
}

fn rrec(r: &mut Rng, st: &St, label: i64) -> Rec {
    let (n, nl) = (st.n, st.nl);
    let fixed = (0..n)
        .map(|s| {
            r.p(1, 4)
                .then(|| if s < nl { 1 + r.below(3) as i32 } else { -(r.below(2) as i32) })
        })
        .collect();
    let v = |r: &mut Rng| format!("n{}", r.below(nl));
    let equation = r.p(1, 10).then(|| format!("{}-{}", v(r), v(r)));
    let bound = r.p(1, 6).then(|| format!("{}-2", v(r)));
    let exception = r.p(1, 4).then(|| {
        let mut b = vec![format!("{}-{}", v(r), 1 + r.below(3))];
        if r.p(1, 3) {
            b.push(format!("{}-{}", v(r), v(r)));
        }
        b
    });
    let target = r.p(1, 6).then(|| (r.below(nl), if r.p(1, 2) { 1 } else { -1 }));
    let recenter = (target.is_none() && r.p(1, 10)).then(|| format!("{}-2", v(r)));
    let pole = r.p(1, 8).then(|| format!("{}-{}", v(r), 2 + r.below(2)));
    let nb = 1 + r.below(3);
    Rec {
        label,
        fixed,
        equation,
        bound,
        exception,
        recenter,
        pole,
        target,
        witness: if r.p(1, 4) { r.below(5) as u8 } else { 0 },
        body: (0..nb).map(|_| rpat(r, n, nl)).collect(),
    }
}

fn domain(st: &St, rc: &Rec) -> CaseDomain {
    let n = st.n;
    let p = |s: &str| Poly::parse_d_free(s, n).unwrap();
    let mut d = CaseDomain::free(n, st.chart.then(|| lines(st)));
    d.fixed = rc.fixed.clone();
    d.equations = rc.equation.iter().map(|s| p(s)).collect();
    d.bounds = rc.bound.iter().map(|s| p(s)).collect();
    d.exceptions = rc.exception.iter().map(|b| b.iter().map(|s| p(s)).collect()).collect();
    d.recenter_bounds = rc.recenter.iter().map(|s| p(s)).collect();
    d.poles = rc
        .pole
        .iter()
        .map(|s| PoleLocus::new(PoleKind::Denominator, &p(s)))
        .collect();
    if let Some((slot, off)) = rc.target {
        d.target = TargetMap::Pattern(
            (0..n)
                .map(|s| TargetSlot::Param {
                    param: s,
                    offset: if s == slot { off } else { 0 },
                })
                .collect(),
        );
    }
    d
}

fn meta(i: usize, w: u8) -> RecordMeta {
    let (witness, witness_basis) = match w {
        0 => (WitnessStatus::Complete, Assurance::Record),
        1 => (WitnessStatus::Elided, Assurance::Record),
        2 => (WitnessStatus::NotCarried, Assurance::Record),
        3 => (WitnessStatus::Elided, Assurance::CheckDisabled),
        _ => (WitnessStatus::Complete, Assurance::AttestedUnchecked),
    };
    RecordMeta {
        source_index: i,
        line_no: 0,
        witness,
        witness_basis,
        target_basis: Assurance::Record,
    }
}

fn build(st: &St) -> (Dispatcher, RuleBodies) {
    let tabs: Vec<Table> = st
        .tables
        .iter()
        .map(|(name, recs)| {
            let mut t = Table::new(name.clone(), st.n);
            for (i, rc) in recs.iter().enumerate() {
                t.push_with_meta(rc.label, domain(st, rc), meta(i, rc.witness))
                    .unwrap();
            }
            t
        })
        .collect();
    let d = Dispatcher::new(tabs, st.prio.clone()).unwrap();
    let mut b = RuleBodies::new(&d);
    for (t, (_, recs)) in st.tables.iter().enumerate() {
        for (i, rc) in recs.iter().enumerate() {
            b.set(&d, t, i, &rc.body).unwrap();
        }
    }
    (d, b)
}

fn kset(st: &St, keys: &[Vec<i32>]) -> Option<KeySet> {
    (!keys.is_empty()).then(|| {
        KeySet::from_keys(
            KeyCodec::new(st.n, 6).unwrap(),
            keys.iter().map(|k| k.as_slice()),
        )
        .unwrap()
    })
}

fn cmp_of(st: &St) -> NodeComparator {
    let ties = st.ties.then(|| (0..st.n).rev().collect());
    NodeComparator::legacy(st.n, ties).unwrap()
}

fn config(st: &St, threads: usize) -> ClosureConfig {
    let exclusions = st
        .excl
        .iter()
        .filter_map(|(k, t, l)| {
            if st.sets.iter().any(|s| s.name == *t) {
                return Some(KeyExclusion {
                    key: k.clone(),
                    table: t.clone(),
                    record: 0,
                    label: 0,
                    reason: ExclusionReason::Other("set".into()),
                });
            }
            let recs = &st.tables.iter().find(|x| x.0 == *t)?.1;
            let record = recs.iter().position(|r| r.label == *l)?;
            Some(KeyExclusion {
                key: k.clone(),
                table: t.clone(),
                record,
                label: *l,
                reason: ExclusionReason::KeepsTerminal,
            })
        })
        .collect();
    let nl = st.nl;
    let mut swap = (0..nl).collect::<Vec<_>>();
    swap.swap(0, 1);
    let rot: Vec<usize> = (0..nl).map(|i| (i + 1) % nl).collect();
    let nn = st.n - nl;
    let nrev: Vec<usize> = (0..nn).rev().collect();
    let codec = KeyCodec::new(st.n, 6).unwrap();
    let relation_sets = st
        .sets
        .iter()
        .map(|s| {
            let mut b = RelationStoreBuilder::new(codec);
            let mut keys = s.keys.clone();
            keys.sort();
            keys.dedup_by(|a, b| a.0 == b.0);
            for (k, v) in &keys {
                let mut lo = k.clone();
                let l = (k.iter().map(|&x| x.unsigned_abs() as usize).sum::<usize>()) % nl;
                lo[l] -= 1;
                let mut rhs = vec![lo];
                if k[0] > 1 {
                    let mut lo2 = k.clone();
                    lo2[0] -= 1;
                    rhs.push(lo2);
                }
                b.push(k, &rhs, [k[0] as u8; 32], *v).unwrap();
            }
            let mut dom = CaseDomain::free(st.n, st.chart.then(|| lines(st)));
            dom.fixed = s.fixed.clone();
            Arc::new(RelationSet {
                name: s.name.clone(),
                recipe_digest: format!("sha256:{}", s.name),
                domain: dom,
                relations: b.finish().unwrap(),
                slot: match &s.slot {
                    Some(t) => DispatchSlot::BeforeTable(t.clone()),
                    None => DispatchSlot::Last,
                },
            })
        })
        .collect();
    ClosureConfig {
        tier: st.tier,
        threads,
        max_rounds: st.max_rounds,
        lines: (!st.chart).then(|| lines(st)),
        hand_downs: match st.hand_downs {
            0 => HandDowns::Off,
            1 => HandDowns::Counts,
            _ => HandDowns::Keys,
        },
        zero_sectors: st.zero.then(|| BTreeSet::from([1u64, 1 << (nl - 1)])),
        zero_sector_source: st.zero.then(|| "audit:zero".to_string()),
        terminals: Some(TerminalSets {
            preferred: kset(st, &st.pref),
            other: kset(st, &st.other),
        }),
        exclusions,
        credits: st.credits.then(|| CreditPolicy {
            group: vec![
                GroupElement {
                    lines: swap.clone(),
                    numerators: None,
                },
                GroupElement {
                    lines: rot.clone(),
                    numerators: Some(nrev.clone()),
                },
            ],
            pure_numerator_maps: BTreeSet::from([1]),
        }),
        relation_sets,
        table_digests: st
            .tables
            .iter()
            .map(|(n, _)| (n.clone(), format!("sha256:t-{n}")))
            .collect(),
        ..Default::default()
    }
}

struct HFacts;
fn hh(f: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    f(&mut s);
    s.finish()
}
impl FactSource for HFacts {
    fn coefficient(&self, r: &RuleId, key: &[i32], term: usize, tk: &[i32]) -> CoefFact {
        match hh(&|s| (r.digest, key, term, tk).hash(s)) % 5 {
            0 | 1 => CoefFact::Zero,
            2 => CoefFact::NonZero,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, r: &RuleId, key: &[i32]) -> RegFact {
        match hh(&|s| (r.digest, key, 1u8).hash(s)) % 7 {
            0 => RegFact::Pole,
            1 => RegFact::Unknown,
            _ => RegFact::Regular,
        }
    }
    fn witness_lift(&self, r: &RuleId, key: &[i32]) -> RegFact {
        match hh(&|s| (r.digest, key, 2u8).hash(s)) % 3 {
            0 => RegFact::Unknown,
            _ => RegFact::Regular,
        }
    }
    fn label(&self) -> String {
        "audit-closure2-hash".into()
    }
}

fn view(c: &Closure) -> String {
    let mut v = serde_json::to_value(c.report(&Default::default())).unwrap();
    v["fact_requests"] = serde_json::to_value(c.fact_requests()).unwrap();
    v["keys_in_order"] = serde_json::to_value((0..c.len()).map(|i| c.key(i)).collect::<Vec<_>>()).unwrap();
    serde_json::to_string(&v).unwrap()
}

/// Order-free view (recheck vs union: reach order and witness choice differ legitimately). `strict = false` compares
/// only whether a cycle exists and the longest chain only on acyclic closures (finding F6: on a cyclic graph both
/// numbers depend on the reach order).
fn canonical(c: &Closure, strict: bool) -> String {
    let s = c.summary();
    let mut open: Vec<_> = c
        .open_keys()
        .into_iter()
        .map(|o| format!("{:?}{}{:?}{:?}", o.key, o.in_start, o.status, o.rule))
        .collect();
    open.sort();
    let mut cr: Vec<_> = c.credits().into_iter().map(|x| format!("{x:?}")).collect();
    cr.sort();
    let mut tr: Vec<_> = c.terminal_records().into_iter().map(|t| format!("{t:?}")).collect();
    tr.sort();
    let mut winners: Vec<_> = (0..c.len()).map(|i| format!("{:?}{:?}", c.key(i), c.winner(i))).collect();
    winners.sort();
    let acyclic = s.cycles == Some(0);
    let (cycles, chain, blocking) = if strict {
        (format!("{:?}", s.cycles), format!("{:?}", s.longest_chain.map(|x| x.edges)), s.blocking.clone())
    } else {
        (
            format!("{:?}", s.cycles.map(|c| c > 0)),
            format!("{:?}", s.longest_chain.filter(|_| acyclic).map(|x| x.edges)),
            s.blocking.iter().map(|b| if b.starts_with("cycles:") { "cycles".into() } else { b.clone() }).collect(),
        )
    };
    serde_json::to_string(&serde_json::json!({
        "keys": c.keys().digest(), "verdict": s.verdict, "edges": s.edges, "terms": s.terms,
        "rz": s.requires_coefficient_zero, "viol": s.descent_violations, "pnz": s.positive_numerator_nonzero,
        "unrep": s.unrepresentable, "und": s.undecided, "cycles": cycles, "open": open, "credits": cr,
        "terminals": tr, "used": c.used_rules(), "rel": c.used_relations(), "winners": winners,
        "exits": c.hand_down_sets().iter().map(|(m, k)| (m, k.digest())).collect::<Vec<_>>(),
        "hd": s.hand_downs, "req": c.fact_requests(), "config": s.config_digest, "blocking": blocking,
        "chain": chain, "lifted": s.witness_lifted,
    }))
    .unwrap()
}

fn mutate(st: &mut St, r: &mut Rng, next: &mut i64, reached: &[Vec<i32>]) -> &'static str {
    let pick = |r: &mut Rng, st: &St| -> Vec<i32> {
        if !reached.is_empty() && r.p(3, 4) {
            reached[r.below(reached.len())].clone()
        } else {
            rkey(r, st)
        }
    };
    match r.below(20) {
        0 | 1 => {
            let name = format!("t{}", *next);
            let recs = (0..1 + r.below(4))
                .map(|_| {
                    *next += 1;
                    rrec(r, st, *next)
                })
                .collect();
            st.tables.push((name, recs));
            let pos = r.below(st.prio.len() + 1);
            st.prio.insert(pos, st.tables.len() - 1);
            "add table"
        }
        2 => {
            let t = r.below(st.tables.len());
            if st.tables[t].1.len() > 1 {
                let i = r.below(st.tables[t].1.len());
                st.tables[t].1.remove(i);
            }
            "remove record"
        }
        3 => {
            let t = r.below(st.tables.len());
            let i = r.below(st.tables[t].1.len() + 1);
            *next += 1;
            let rc = rrec(r, st, *next);
            st.tables[t].1.insert(i, rc);
            "insert record mid-table"
        }
        4 => {
            let t = r.below(st.tables.len());
            let i = r.below(st.tables[t].1.len());
            let label = st.tables[t].1[i].label;
            st.tables[t].1[i] = rrec(r, st, label);
            "change record in place"
        }
        5 => {
            // witness status only (same domain): the table's dispatch digest is unchanged
            let t = r.below(st.tables.len());
            let i = r.below(st.tables[t].1.len());
            st.tables[t].1[i].witness = r.below(5) as u8;
            "witness status only"
        }
        6 => {
            let t = r.below(st.tables.len());
            let i = r.below(st.tables[t].1.len());
            let (n, nl) = (st.n, st.nl);
            st.tables[t].1[i].body = (0..1 + r.below(3)).map(|_| rpat(r, n, nl)).collect();
            "body only"
        }
        7 => {
            let n = st.prio.len();
            if n > 1 {
                let (a, b) = (r.below(n), r.below(n));
                st.prio.swap(a, b);
            }
            "permute priority"
        }
        8 => {
            let t = r.below(st.tables.len());
            *next += 1;
            let old = st.tables[t].0.clone();
            let new = format!("t{}", *next);
            st.tables[t].0 = new.clone();
            for e in st.excl.iter_mut().filter(|e| e.1 == old) {
                e.1 = new.clone();
            }
            for s in st.sets.iter_mut().filter(|s| s.slot.as_deref() == Some(old.as_str())) {
                s.slot = Some(new.clone());
            }
            "rename table"
        }
        9 => {
            let k = pick(r, st);
            if r.p(1, 2) { st.pref.push(k) } else { st.other.push(k) }
            "add terminal"
        }
        10 => {
            if !st.pref.is_empty() && r.p(1, 2) {
                let i = r.below(st.pref.len());
                st.pref.remove(i);
            } else if !st.other.is_empty() {
                let i = r.below(st.other.len());
                st.other.remove(i);
            }
            "remove terminal"
        }
        11 => {
            let k = pick(r, st);
            if !st.sets.is_empty() && r.p(1, 4) {
                let s = st.sets[r.below(st.sets.len())].name.clone();
                st.excl.push((k, s, 0));
            } else {
                let t = r.below(st.tables.len());
                let i = r.below(st.tables[t].1.len());
                st.excl.push((k, st.tables[t].0.clone(), st.tables[t].1[i].label));
            }
            "add exclusion"
        }
        12 => {
            if !st.excl.is_empty() {
                let i = r.below(st.excl.len());
                st.excl.remove(i);
            }
            "remove exclusion"
        }
        13 => {
            if st.start.len() > 1 && r.p(1, 2) {
                let i = r.below(st.start.len());
                st.start.remove(i);
            } else {
                let k = rkey(r, st);
                st.start.push(k);
            }
            "start set"
        }
        14 => {
            match r.below(6) {
                0 => st.credits = !st.credits,
                1 => st.tier = if st.tier == Tier::K { Tier::C } else { Tier::K },
                2 => st.hand_downs = r.below(3) as u8,
                3 => st.zero = !st.zero,
                4 => st.ties = !st.ties,
                _ => st.max_rounds = if r.p(1, 3) { Some(1 + r.below(4) as u32) } else { None },
            }
            "toggle config"
        }
        15 => {
            st.chart = !st.chart;
            "toggle chart"
        }
        _ => {
            if st.sets.is_empty() || (st.sets.len() < 2 && r.p(1, 3)) {
                *next += 1;
                let slot = r
                    .p(2, 3)
                    .then(|| st.tables[r.below(st.tables.len())].0.clone());
                let (n, nl) = (st.n, st.nl);
                let fixed = (0..n)
                    .map(|s| (s < nl && r.p(1, 5)).then(|| 1 + r.below(3) as i32))
                    .collect();
                st.sets.push(Set {
                    name: format!("s{}", *next),
                    slot,
                    fixed,
                    keys: vec![],
                });
            }
            let i = r.below(st.sets.len());
            match r.below(6) {
                0 => {
                    let s = st.sets.remove(i);
                    st.excl.retain(|e| e.1 != s.name);
                }
                1 => st.sets[i].slot = r.p(2, 3).then(|| st.tables[r.below(st.tables.len())].0.clone()),
                _ => {
                    let k = pick(r, st);
                    let v = if r.p(1, 3) { [1u8, 3, 5][r.below(3)] } else { 7 };
                    if k[0] >= 1 && k.iter().take(st.nl).all(|&x| x >= 1) {
                        st.sets[i].keys.push((k, v));
                    }
                }
            }
            "relation set"
        }
    }
}

#[test]
fn audit_incremental_equals_scratch_under_random_edit_sequences() {
    run_sequences(false, None);
}

/// F6 (MINOR): the recheck's report equals the union's EXACTLY in `cycles` and `longest_chain` too (same identity,
/// closure.md 11.4 "result = closure of the union"). Fails on b5ba6b33: on cyclic (failed) closures the DFS back-edge
/// count and the memoised chain length depend on the reach order (seed below: case 28 step 2 with 60 cases).
#[test]
#[ignore = "AUDIT-CLOSURE2 F6: fails on b5ba6b33 (cycles / chain depend on reach order)"]
fn f6_recheck_equals_union_strictly() {
    run_sequences(true, Some(60));
}

fn run_sequences(strict: bool, cases: Option<u64>) {
    let cases: u64 = cases.unwrap_or_else(|| {
        std::env::var("AUDIT_CLOSURE2_CASES")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(40)
    });
    let mut r = Rng(0xa0d1_c105_2026_1114);
    let facts = HFacts;
    let mut stats = std::collections::BTreeMap::<&str, u64>::new();
    let (mut steps, mut reused, mut full, mut fallback, mut rechecks) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut agg = [0u64; 8];
    for case in 0..cases {
        let n = 4 + r.below(6);
        let nl = 2 + r.below(n - 3);
        let mut st = St {
            n,
            nl,
            tables: vec![],
            prio: vec![0],
            chart: r.p(3, 4),
            ties: false,
            pref: vec![],
            other: vec![],
            excl: vec![],
            start: vec![],
            credits: r.p(1, 2),
            tier: if case % 2 == 0 { Tier::K } else { Tier::C },
            sets: vec![],
            hand_downs: 2,
            zero: false,
            max_rounds: None,
        };
        let mut next = 0i64;
        let recs = (0..1 + r.below(4))
            .map(|_| {
                next += 1;
                rrec(&mut r, &st, next)
            })
            .collect();
        st.tables.push(("t0".into(), recs));
        st.start = (0..1 + r.below(3)).map(|_| rkey(&mut r, &st)).collect();
        let (d, b) = build(&st);
        let mut prev_inc = closure(&d, &b, &cmp_of(&st), &kset(&st, &st.start).unwrap(), &config(&st, 2), &facts).unwrap();
        let mut prev_scr = prev_inc.clone();
        for step in 0..8 {
            let reached: Vec<Vec<i32>> = prev_scr.keys().iter().collect();
            let what = mutate(&mut st, &mut r, &mut next, &reached);
            *stats.entry(what).or_default() += 1;
            let (d, b) = build(&st);
            let cmp = cmp_of(&st);
            let start = kset(&st, &st.start).unwrap();
            let cfg = config(&st, 2);
            let scr = closure(&d, &b, &cmp, &start, &cfg, &facts).unwrap();
            let vs = view(&scr);
            let inc = closure_incremental(&prev_inc.dispatch_cache(), &d, &b, &cmp, &start, &cfg, &facts).unwrap();
            assert!(view(&inc) == vs, "case {case} step {step} ({what}): chained incremental != scratch\n{st:?}");
            let inc2 = closure_incremental(&prev_scr.dispatch_cache(), &d, &b, &cmp, &start, &cfg, &facts).unwrap();
            assert!(view(&inc2) == vs, "case {case} step {step} ({what}): incremental != scratch\n{st:?}");
            // thread independence (1 and 5 threads)
            for th in [1usize, 5] {
                let t = closure(&d, &b, &cmp, &start, &config(&st, th), &facts).unwrap();
                assert!(view(&t) == vs, "case {case} step {step}: threads {th} changed the result");
            }
            let u = inc.incremental_use().unwrap();
            reused += u.reused;
            full += u.full;
            fallback += u.fallback.is_some() as u64;
            let s = scr.summary();
            for (i, x) in [
                s.credited,
                s.recipe_served,
                s.terminal_in_rule_domain,
                s.witness_lifted.values().sum::<u64>(),
                s.open_by_status.get("blocked").copied().unwrap_or(0),
                s.open_by_status.get("undecided").copied().unwrap_or(0),
                s.descent_violations,
                s.open_by_status.get("recipeunverified").copied().unwrap_or(0),
            ]
            .into_iter()
            .enumerate()
            {
                agg[i] += x;
            }
            if !s.truncated {
                let extra = kset(&st, &[rkey(&mut r, &st), rkey(&mut r, &st)]).unwrap();
                let re = closure_recheck(&scr, &d, &b, &cmp, &extra, &cfg, &facts).unwrap();
                let un = closure(&d, &b, &cmp, &start.union(&extra), &cfg, &facts).unwrap();
                if !un.summary().truncated {
                    assert_eq!(
                        canonical(&re, strict),
                        canonical(&un, strict),
                        "case {case} step {step}: recheck != union\n{st:?}"
                    );
                    rechecks += 1;
                }
            }
            prev_inc = inc;
            prev_scr = scr;
            steps += 1;
        }
    }
    println!(
        "audit-closure2: incremental == scratch (chained and from the scratch cache), threads 1/2/5 identical, on {steps} \
         steps of {cases} cases; recheck == union on {rechecks}; keys reused {reused}, full {full}, fallbacks {fallback}; \
         mutations {stats:?}; over all steps: credited {}, recipe-served {}, terminal-in-rule-domain {}, lifted {}, \
         blocked {}, undecided-dispatch {}, descent violations {}, recipe-unverified {}",
        agg[0], agg[1], agg[2], agg[3], agg[4], agg[5], agg[6], agg[7]
    );
    assert!(reused > 0 && full > 0 && fallback > 0);
    // (dispatch `Undecided` needs an evaluation error: not produced by these domains; it is a stop like `Blocked`)
    assert!(
        [0usize, 1, 2, 3, 4, 7].iter().all(|&i| agg[i] > 0),
        "generator does not reach every branch: {agg:?}"
    );
}
