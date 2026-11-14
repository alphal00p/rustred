//! INVARIANT (audit loop-A C1, M2, M3): for random sequences of changes on small random nodes (tables added at any
//! priority, rules retired, records changed in place, priorities permuted, terminals and exclusions added AND
//! removed, start set / envelope changed, credit policy toggled, Tier K and Tier C with deterministic random facts)
//! the incremental closure equals the closure from scratch EXACTLY (the whole report: reached set, defect, used set
//! with counts, non-descending edges, credits, terminals, exits, configuration identity), both chained (incremental of
//! incremental) and from the scratch cache; and a recheck with extra start keys equals the closure of the union.
//! Seeded, default suite. `CLOSURE_PROPS_CASES` overrides the number of cases (e.g. 2000 in release).

use std::hash::{Hash, Hasher};

use std::sync::Arc;

use rustred_campaign_core::closure::{
    DispatchSlot, RelationSet, RelationStoreBuilder, VERDICT_CERTIFIED,
    Closure, ClosureConfig, CoefFact, ExclusionReason, FactSource, HandDowns, KeyCodec,
    KeyExclusion, KeySet, RegFact, RuleBodies, RuleId, TerminalSets, Tier, closure,
    closure_incremental, closure_recheck,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::symmetry::{CreditPolicy, GroupElement};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// Node shape: 3 lines (slots 0..3), 2 numerators (slots 3, 4).
const N: usize = 5;
const NL: usize = 3;

#[derive(Clone, Debug)]
struct Rec {
    label: i64,
    fixed: [Option<i32>; N],
    elided: bool,
    body: Vec<String>,
}

#[derive(Clone, Debug)]
struct State {
    tables: Vec<(String, Vec<Rec>)>,
    prio: Vec<usize>,
    pref: Vec<[i32; N]>,
    other: Vec<[i32; N]>,
    /// (key, table name, label)
    excl: Vec<([i32; N], String, i64)>,
    start: Vec<[i32; N]>,
    credits: bool,
    tier: Tier,
    /// a relation set (R48): certified keys and whether it precedes the first table
    recipe: Option<(Vec<[i32; N]>, bool)>,
}

fn chart() -> Vec<bool> {
    (0..N).map(|s| s < NL).collect()
}

fn rand_key(r: &mut Lcg) -> [i32; N] {
    let mut k = [0; N];
    for (s, v) in k.iter_mut().enumerate() {
        *v = if s < NL {
            1 + r.below(3) as i32
        } else {
            -(r.below(2) as i32)
        };
    }
    k
}

fn rand_pattern(r: &mut Lcg) -> String {
    let mut a: Vec<String> = (0..N).map(|s| format!("n{s}")).collect();
    match r.below(10) {
        0..=4 => {
            let l = r.below(NL as u64) as usize;
            a[l] = format!("n{l}-1");
        }
        5 => {
            let l = r.below(NL as u64) as usize;
            a[l] = "0".into();
        }
        6 => {
            let s = NL + r.below((N - NL) as u64) as usize;
            a[s] = format!("n{s}+1");
            let l = r.below(NL as u64) as usize;
            a[l] = format!("n{l}-1");
        }
        7 => {
            // a numerator lowered with a line lowered (descending)
            let s = NL + r.below((N - NL) as u64) as usize;
            a[s] = format!("n{s}-1");
            let l = r.below(NL as u64) as usize;
            a[l] = format!("n{l}-1");
        }
        8 => {
            // swap-like move: one line up, another down (may not descend)
            let l = r.below(NL as u64) as usize;
            let m = (l + 1) % NL;
            a[l] = format!("n{l}+1");
            a[m] = format!("n{m}-1");
        }
        _ => {
            let l = r.below(NL as u64) as usize;
            a[l] = format!("n{l}-2");
        }
    }
    format!("I({})", a.join(","))
}

fn rand_rec(r: &mut Lcg, label: i64) -> Rec {
    let mut fixed = [None; N];
    for (s, f) in fixed.iter_mut().enumerate() {
        if r.chance(1, 3) {
            *f = Some(if s < NL {
                1 + r.below(3) as i32
            } else {
                -(r.below(2) as i32)
            });
        }
    }
    let nb = 1 + r.below(3) as usize;
    Rec {
        label,
        fixed,
        elided: r.chance(1, 12),
        body: (0..nb).map(|_| rand_pattern(r)).collect(),
    }
}

fn build(st: &State) -> (Dispatcher, RuleBodies) {
    let tabs: Vec<Table> = st
        .tables
        .iter()
        .map(|(name, recs)| {
            let mut t = Table::new(name.clone(), N);
            for rc in recs {
                let mut d = CaseDomain::free(N, Some(chart()));
                d.fixed = rc.fixed.to_vec();
                d.witness_elided = rc.elided;
                t.push(rc.label, d).unwrap();
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

fn keyset(keys: &[[i32; N]]) -> Option<KeySet> {
    (!keys.is_empty()).then(|| {
        KeySet::from_keys(
            KeyCodec::new(N, 6).unwrap(),
            keys.iter().map(|k| k.as_slice()),
        )
        .unwrap()
    })
}

fn config(st: &State) -> ClosureConfig {
    let exclusions = st
        .excl
        .iter()
        .filter_map(|(k, t, l)| {
            let recs = &st.tables.iter().find(|x| x.0 == *t)?.1;
            let record = recs.iter().position(|r| r.label == *l)?;
            Some(KeyExclusion {
                key: k.to_vec(),
                table: t.clone(),
                record,
                label: *l,
                reason: ExclusionReason::KeepsTerminal,
            })
        })
        .collect();
    ClosureConfig {
        tier: st.tier,
        threads: 2,
        max_rounds: Some(40),
        hand_downs: HandDowns::Keys,
        terminals: Some(TerminalSets {
            preferred: keyset(&st.pref),
            other: keyset(&st.other),
        }),
        exclusions,
        credits: st.credits.then(|| {
            CreditPolicy::new(vec![
                GroupElement {
                    lines: vec![1, 0, 2],
                    numerators: None,
                },
                GroupElement {
                    lines: vec![2, 1, 0],
                    numerators: Some(vec![1, 0]),
                },
            ])
        }),
        relation_sets: st
            .recipe
            .iter()
            .map(|(keys, first)| {
                let mut b = RelationStoreBuilder::new(KeyCodec::new(N, 6).unwrap());
                let mut ks = keys.clone();
                ks.sort();
                ks.dedup();
                for k in &ks {
                    let mut lo = k.to_vec();
                    lo[1] -= 1;
                    let mut sub = k.to_vec();
                    sub[2] = 0;
                    b.push(k, &[lo, sub], [k[0] as u8; 32], VERDICT_CERTIFIED).unwrap();
                }
                Arc::new(RelationSet {
                    name: "recipe".into(),
                    recipe_digest: "sha256:r".into(),
                    domain: CaseDomain::free(N, Some(chart())),
                    relations: b.finish().unwrap(),
                    slot: if *first {
                        DispatchSlot::BeforeTable(st.tables[st.prio[0]].0.clone())
                    } else {
                        DispatchSlot::Last
                    },
                })
            })
            .collect(),
        table_digests: st
            .tables
            .iter()
            .map(|(n, _)| (n.clone(), format!("sha256:test-{n}")))
            .collect(),
        ..Default::default()
    }
}

/// Deterministic pseudo-random facts (a function of the rule content, key and term key).
struct HashFacts;
fn h(parts: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    parts(&mut s);
    s.finish()
}
impl FactSource for HashFacts {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact {
        match h(&|s| (rule.digest, key, term, term_key).hash(s)) % 4 {
            0 | 1 => CoefFact::Zero,
            2 => CoefFact::NonZero,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        match h(&|s| (rule.digest, key, 7u8).hash(s)) % 6 {
            0 => RegFact::Pole,
            1 => RegFact::Unknown,
            _ => RegFact::Regular,
        }
    }
    fn label(&self) -> String {
        "hash-facts".into()
    }
}

fn view(c: &Closure) -> serde_json::Value {
    let mut v = serde_json::to_value(c.report(&Default::default())).unwrap();
    v["fact_requests"] = serde_json::to_value(c.fact_requests()).unwrap();
    v
}

/// Order-independent view (for the recheck, whose reach order differs from the closure of the union).
fn canonical(c: &Closure) -> serde_json::Value {
    let s = c.summary();
    let mut open: Vec<_> = c
        .open_keys()
        .into_iter()
        .map(|o| (o.key, o.in_start, o.status, o.rule))
        .collect();
    open.sort();
    let mut credits: Vec<_> = c
        .credits()
        .into_iter()
        .map(|x| (x.key, x.in_start, x.element, x.image, x.rule))
        .collect();
    credits.sort();
    let mut terms: Vec<_> = c
        .terminal_records()
        .into_iter()
        .map(|t| format!("{t:?}"))
        .collect();
    terms.sort();
    serde_json::json!({
        "keys": c.keys().digest(), "verdict": s.verdict, "edges": s.edges, "terms": s.terms,
        "requires_zero": s.requires_coefficient_zero, "violations": s.descent_violations,
        "pos_nonzero": s.positive_numerator_nonzero, "unrepresentable": s.unrepresentable,
        "undecided": s.undecided, "cycles": s.cycles, "open": format!("{open:?}"),
        "credits": format!("{credits:?}"), "terminals": terms, "used": c.used_rules(),
        "exits": c.hand_down_sets().iter().map(|(m, k)| (m, k.digest())).collect::<Vec<_>>(),
        "requests": c.fact_requests(), "config": s.config_digest, "blocking": s.blocking,
        "chain": s.longest_chain.map(|x| x.edges),
    })
}

fn mutate(st: &mut State, r: &mut Lcg, next_label: &mut i64, reached: &[Vec<i32>]) {
    let pick_key = |r: &mut Lcg| -> [i32; N] {
        if !reached.is_empty() && r.chance(3, 4) {
            let k = &reached[r.below(reached.len() as u64) as usize];
            let mut a = [0; N];
            a.copy_from_slice(k);
            a
        } else {
            rand_key(r)
        }
    };
    match r.below(12) {
        0 | 1 => {
            // add a table at a random priority position
            let name = format!("t{}", *next_label);
            let nrec = 1 + r.below(5) as usize;
            let recs = (0..nrec)
                .map(|_| {
                    *next_label += 1;
                    rand_rec(r, *next_label)
                })
                .collect();
            st.tables.push((name, recs));
            let pos = r.below(st.prio.len() as u64 + 1) as usize;
            st.prio.insert(pos, st.tables.len() - 1);
        }
        2 => {
            // retire a rule
            let t = r.below(st.tables.len() as u64) as usize;
            if st.tables[t].1.len() > 1 {
                let i = r.below(st.tables[t].1.len() as u64) as usize;
                st.tables[t].1.remove(i);
            }
        }
        3 => {
            // change a record in place (same table, position and label)
            let t = r.below(st.tables.len() as u64) as usize;
            let i = r.below(st.tables[t].1.len() as u64) as usize;
            let label = st.tables[t].1[i].label;
            st.tables[t].1[i] = rand_rec(r, label);
        }
        4 => {
            // permute the priority
            let n = st.prio.len();
            if n > 1 {
                let (a, b) = (r.below(n as u64) as usize, r.below(n as u64) as usize);
                st.prio.swap(a, b);
            }
        }
        5 => {
            let k = pick_key(r);
            if r.chance(1, 2) {
                st.pref.push(k)
            } else {
                st.other.push(k)
            }
        }
        6 => {
            if !st.pref.is_empty() && r.chance(1, 2) {
                let i = r.below(st.pref.len() as u64) as usize;
                st.pref.remove(i);
            } else if !st.other.is_empty() {
                let i = r.below(st.other.len() as u64) as usize;
                st.other.remove(i);
            }
        }
        7 => {
            let k = pick_key(r);
            let t = r.below(st.tables.len() as u64) as usize;
            let i = r.below(st.tables[t].1.len() as u64) as usize;
            st.excl
                .push((k, st.tables[t].0.clone(), st.tables[t].1[i].label));
        }
        8 => {
            if !st.excl.is_empty() {
                let i = r.below(st.excl.len() as u64) as usize;
                st.excl.remove(i);
            }
        }
        9 => {
            // envelope / start set change
            if st.start.len() > 1 && r.chance(1, 2) {
                let i = r.below(st.start.len() as u64) as usize;
                st.start.remove(i);
            } else {
                st.start.push(rand_key(r));
            }
        }
        10 => st.credits = !st.credits,
        _ => match &mut st.recipe {
            None => st.recipe = Some((vec![pick_key(r)], r.chance(1, 2))),
            Some((keys, first)) => {
                if r.chance(1, 4) {
                    *first = !*first;
                } else if r.chance(1, 5) {
                    st.recipe = None;
                } else {
                    keys.push(pick_key(r));
                }
            }
        },
    }
}

#[test]
fn incremental_equals_scratch_random_sequences() {
    let cases: u64 = std::env::var("CLOSURE_PROPS_CASES")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(60);
    let cmp = NodeComparator::legacy(N, None).unwrap();
    let mut r = Lcg(0xc1c1_2026);
    let (mut steps, mut reused, mut full, mut fallbacks, mut credited, mut viol, mut tirds) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut recipe = 0u64;
    for case in 0..cases {
        let mut next_label = 0i64;
        let tier = if case % 2 == 0 { Tier::K } else { Tier::C };
        let mut st = State {
            tables: vec![(
                "t0".into(),
                (0..1 + r.below(4))
                    .map(|_| {
                        next_label += 1;
                        rand_rec(&mut r, next_label)
                    })
                    .collect(),
            )],
            prio: vec![0],
            pref: vec![],
            other: vec![],
            excl: vec![],
            start: (0..1 + r.below(3)).map(|_| rand_key(&mut r)).collect(),
            credits: r.chance(1, 2),
            tier,
            recipe: None,
        };
        let facts = HashFacts;
        let (d, b) = build(&st);
        let start = keyset(&st.start).unwrap();
        let mut prev_inc = closure(&d, &b, &cmp, &start, &config(&st), &facts).unwrap();
        let mut prev_scratch = prev_inc.clone();
        for _ in 0..6 {
            let reached: Vec<Vec<i32>> = prev_scratch.keys().iter().collect();
            mutate(&mut st, &mut r, &mut next_label, &reached);
            let (d, b) = build(&st);
            let cfg = config(&st);
            let start = keyset(&st.start).unwrap();
            let scratch = closure(&d, &b, &cmp, &start, &cfg, &facts).unwrap();
            let inc = closure_incremental(
                &prev_inc.dispatch_cache(),
                &d,
                &b,
                &cmp,
                &start,
                &cfg,
                &facts,
            )
            .unwrap();
            let inc2 = closure_incremental(
                &prev_scratch.dispatch_cache(),
                &d,
                &b,
                &cmp,
                &start,
                &cfg,
                &facts,
            )
            .unwrap();
            let vs = view(&scratch);
            assert_eq!(
                view(&inc),
                vs,
                "case {case}: chained incremental != scratch; state {st:?}"
            );
            assert_eq!(
                view(&inc2),
                vs,
                "case {case}: incremental != scratch; state {st:?}"
            );
            let u = inc.incremental_use().unwrap();
            reused += u.reused;
            full += u.full;
            fallbacks += u.fallback.is_some() as u64;
            let s = scratch.summary();
            recipe += s.recipe_served;
            credited += s.credited;
            viol += s.descent_violations;
            tirds += s.terminal_in_rule_domain;
            steps += 1;
            // recheck with extra start keys == closure of the union (when not truncated)
            if !s.truncated {
                let extra = keyset(&[rand_key(&mut r), rand_key(&mut r)]).unwrap();
                let re = closure_recheck(&scratch, &d, &b, &cmp, &extra, &cfg, &facts).unwrap();
                let union = closure(&d, &b, &cmp, &start.union(&extra), &cfg, &facts).unwrap();
                if !union.summary().truncated {
                    assert_eq!(
                        canonical(&re),
                        canonical(&union),
                        "case {case}: recheck != union"
                    );
                }
            }
            prev_inc = inc;
            prev_scratch = scratch;
        }
    }
    println!(
        "incremental == scratch on {steps} steps ({cases} cases): keys reused {reused}, dispatched in full {full}, \
         cache fallbacks {fallbacks}; credited {credited}, descent violations {viol}, terminals in a rule domain {tirds}, recipe-served keys {recipe}"
    );
    assert!(reused > 0 && full > 0 && credited > 0 && tirds > 0 && recipe > 0);
}
