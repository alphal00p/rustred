//! Independent audit (lane audit-credits, 2026-11-14): closure semantics v4 (per-slot numerator purity + image lift)
//! on RANDOM small nodes with a VERIFIED group (two-loop sunset family, arity 5, lines D1 D2 D3, numerators D4 D5;
//! no element of interest has a fully pure numerator map, so per-slot purity is exercised).
//!  1. TIER K SUPERSET: the keys-only closure (per-key lift `Proof`) reaches every key of every Tier C run, whatever the
//!     facts (random per-key proofs, regularity facts, coefficient facts, witness-lift facts); random witness status
//!     (complete / elided / not carried) of every record.
//!  2. WELL-FOUNDED CREDITS: with strictly descending rule bodies and DECIDED facts, a Tier C run has no cycle and no
//!     descent violation; every credit of every run is the exact one-term relation I(key) = I(image) (the expanded
//!     symmetry row of the credited element at the key is {key: 1, image: -1}); every followed top term of the image's
//!     rule is strictly below the ORIGINAL key; no credit's image is itself credited (never chained).

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::closure::{
    ClosureConfig, CoefFact, FactSource, HandDowns, KeyCodec, KeySet, NoFacts, PerKeyLift, RegFact,
    RuleBodies, RuleId, Tier, closure,
};
use rustred_campaign_core::cover::{Assurance, Dispatcher, RecordMeta, Table, WitnessStatus};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, SymRowMode, VerifiedGroup, rows_at, verify_derived,
};
use rustred_campaign_core::symmetry::CreditPolicy;

const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";
const N: usize = 5;
const LINES: [bool; 5] = [true, true, true, false, false];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i32
    }
}

fn h(seed: u64, a: i64, k: &[i32], t: usize) -> u64 {
    let mut x = seed ^ (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (t as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    for &v in k {
        x = (x ^ (v as u64).wrapping_add(0x165667B1)).wrapping_mul(0x100_0000_01B3);
        x ^= x >> 29;
    }
    x
}

/// Random deterministic facts keyed by (record label, key, term). `decided`: never `Unknown`.
struct RandFacts {
    seed: u64,
    decided: bool,
}
impl FactSource for RandFacts {
    fn coefficient(&self, r: &RuleId, key: &[i32], term: usize, _tk: &[i32]) -> CoefFact {
        match h(self.seed, r.label, key, term + 1) % if self.decided { 2 } else { 3 } {
            0 => CoefFact::NonZero,
            1 => CoefFact::Zero,
            _ => CoefFact::Unknown,
        }
    }
    fn regularity(&self, r: &RuleId, key: &[i32]) -> RegFact {
        match h(self.seed, r.label, key, 0) % if self.decided { 4 } else { 5 } {
            0 => RegFact::Pole,
            1..=3 => RegFact::Regular,
            _ => RegFact::Unknown,
        }
    }
    fn witness_lift(&self, r: &RuleId, key: &[i32]) -> RegFact {
        match h(self.seed ^ 0x77, r.label, key, 0) % 3 {
            0 => RegFact::Regular,
            1 => RegFact::Pole,
            _ => RegFact::Unknown,
        }
    }
    fn per_key_proof(&self, r: &RuleId, key: &[i32]) -> bool {
        h(self.seed ^ 0x55, r.label, key, 0) % 2 == 0
    }
    fn label(&self) -> String {
        format!("audit-credits-rand-{}", self.seed)
    }
}

fn kt(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

struct Node {
    disp: Dispatcher,
    bodies: RuleBodies,
    /// record key -> (body keys, witness, label)
    recs: BTreeMap<Vec<i32>, (Vec<Vec<i32>>, WitnessStatus, i64)>,
}

fn random_node(rng: &mut Rng, cmp: &NodeComparator, descending: bool) -> Node {
    let nrec = rng.range(8, 40) as usize;
    let mut keys: BTreeSet<Vec<i32>> = BTreeSet::new();
    while keys.len() < nrec {
        keys.insert(
            (0..N)
                .map(|s| if LINES[s] { rng.range(1, 3) } else { rng.range(-2, 0) })
                .collect(),
        );
    }
    let pool: Vec<Vec<i32>> = keys.iter().cloned().collect();
    let mut recs: BTreeMap<Vec<i32>, (Vec<Vec<i32>>, WitnessStatus)> = BTreeMap::new();
    for k in keys {
        let nt = rng.range(1, 3);
        let mut body = vec![];
        let mut tries = 0;
        while body.len() < nt as usize && tries < 200 {
            tries += 1;
            // half the terms from the other records' keys (deep closures), half random (gaps, numerators > 0)
            let t: Vec<i32> = if rng.next() % 2 == 0 {
                pool[rng.next() as usize % pool.len()].clone()
            } else {
                (0..N)
                    .map(|s| if LINES[s] { rng.range(0, 3) } else { rng.range(-2, 1) })
                    .collect()
            };
            let pos_num = (0..N).any(|s| !LINES[s] && t[s] > 0);
            let top = (0..N).all(|s| if LINES[s] { t[s] >= 1 } else { t[s] <= 0 });
            if descending && (pos_num || (top && !cmp.lower(&t, &k))) {
                continue;
            }
            body.push(t);
        }
        if body.is_empty() {
            body.push(vec![0, 1, 1, 0, 0]);
        }
        let w = match rng.next() % 3 {
            0 => WitnessStatus::Complete,
            1 => WitnessStatus::Elided,
            _ => WitnessStatus::NotCarried,
        };
        recs.insert(k, (body, w));
    }
    let mut t = Table::new("R".to_string(), N);
    for (i, (k, (_, w))) in recs.iter().enumerate() {
        let mut d = CaseDomain::free(N, Some(LINES.to_vec()));
        d.fixed = k.iter().map(|&v| Some(v)).collect();
        t.push_with_meta(
            i as i64 + 1,
            d,
            RecordMeta {
                source_index: i,
                line_no: 0,
                witness: *w,
                witness_basis: Assurance::Record,
                target_basis: Assurance::Record,
            },
        )
        .unwrap();
    }
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut bodies = RuleBodies::new(&disp);
    for (i, (_, (body, _))) in recs.iter().enumerate() {
        let txt: Vec<String> = body.iter().map(|b| kt(b)).collect();
        bodies.set(&disp, 0, i, &txt).unwrap();
    }
    Node {
        disp,
        bodies,
        recs: recs
            .into_iter()
            .enumerate()
            .map(|(i, (k, (b, w)))| (k, (b, w, i as i64 + 1)))
            .collect(),
    }
}

fn cfg(tier: Tier, d: &Dispatcher, pol: &CreditPolicy) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads: 1,
        hand_downs: HandDowns::Keys,
        table_digests: d
            .tables()
            .iter()
            .map(|t| (t.name.clone(), format!("sha256:audit-credits-{}", t.name)))
            .collect(),
        credits: Some(pol.clone()),
        per_key_lift: PerKeyLift::Proof,
        ..Default::default()
    }
}

fn one_term(vg: &VerifiedGroup, ei: usize, key: &[i32], image: &[i32]) -> bool {
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let Ok(rows) = rows_at(vg, &seed, SymRowMode::Expanded, 1 << 12) else {
        return false;
    };
    let Some(r) = rows.iter().find(|r| r.element == ei) else {
        return false;
    };
    let got: BTreeSet<(Vec<i64>, i128, i128)> = r.terms.iter().map(|((_, k), q)| (k.clone(), q.n, q.d)).collect();
    let want: BTreeSet<(Vec<i64>, i128, i128)> = [
        (seed.1.clone(), 1, 1),
        (image.iter().map(|&x| x as i64).collect(), -1, 1),
    ]
    .into_iter()
    .collect();
    got == want
}

#[derive(Default, Debug)]
struct Stats {
    nodes: usize,
    tier_c_runs: usize,
    tier_c_keys: usize,
    tier_k_keys: usize,
    superset_violations: Vec<String>,
    credits_checked: usize,
    credits_with_numerator: usize,
    credits_lifted_image: usize,
    credit_refuted: Vec<String>,
    chained: Vec<String>,
    not_below: Vec<String>,
    acyclic_runs: usize,
    cyclic: Vec<String>,
}

fn check_credits(
    tag: &str,
    cl: &rustred_campaign_core::closure::Closure,
    node: &Node,
    vg: &VerifiedGroup,
    cmp: &NodeComparator,
    facts: Option<&RandFacts>,
    st: &mut Stats,
) {
    let credited: BTreeSet<Vec<i32>> = cl.credits().into_iter().map(|c| c.key).collect();
    let rid = |label: i64| RuleId {
        table: "R",
        record: (label - 1) as usize,
        label,
        digest: "",
    };
    for c in cl.credits() {
        st.credits_checked += 1;
        if (0..N).any(|s| !LINES[s] && c.key[s] != 0) {
            st.credits_with_numerator += 1;
        }
        if !one_term(vg, c.element, &c.key, &c.image) {
            st.credit_refuted.push(format!("{tag}: {:?} -> {:?} (element {})", c.key, c.image, c.element));
        }
        if credited.contains(&c.image) {
            st.chained.push(format!("{tag}: {:?} -> credited image {:?}", c.key, c.image));
        }
        let Some((body, w, label)) = node.recs.get(&c.image) else {
            st.credit_refuted.push(format!("{tag}: {:?} -> {:?}: image has no record", c.key, c.image));
            continue;
        };
        // the image's record must serve the image: complete witness, or lifted by a per-key PROOF at (record, image)
        if *w != WitnessStatus::Complete {
            let proof = facts.is_some_and(|f| f.per_key_proof(&rid(*label), &c.image));
            if proof {
                st.credits_lifted_image += 1;
            } else {
                st.credit_refuted.push(format!(
                    "{tag}: {:?} -> {:?}: image record {w:?} without a per-key proof",
                    c.key, c.image
                ));
            }
        }
        // Tier C: every top term not strictly below the ORIGINAL key and every positive-numerator term must carry a
        // Zero coefficient fact (else the credit is not decided: it may be pending, never with a NonZero fact)
        if cl.tier() == Tier::C
            && let Some(f) = facts
        {
            for (j, t) in body.iter().enumerate() {
                let pos_num = (0..N).any(|s| !LINES[s] && t[s] > 0);
                let top = (0..N).all(|s| if LINES[s] { t[s] >= 1 } else { t[s] <= 0 });
                if (pos_num || (top && !cmp.lower(t, &c.key)))
                    && f.coefficient(&rid(*label), &c.image, j, t) == CoefFact::NonZero
                {
                    st.not_below.push(format!(
                        "{tag}: credit {:?} -> {:?}: term {j} {t:?} (not below / positive numerator) has a NonZero fact",
                        c.key, c.image
                    ));
                }
            }
        }
    }
}

#[test]
fn audit_credits_tier_k_superset_and_well_founded_v4() {
    let fam = Family::parse(SUNSET).unwrap();
    let vg = verify_derived(&fam, &LINES, &DeriveOptions::default()).unwrap();
    let pol = CreditPolicy::from_verified(&vg, &LINES).unwrap();
    println!(
        "sunset: {} elements, fully pure numerator maps {:?}",
        pol.group().len(),
        pol.pure_numerator_maps()
    );
    let cmp = NodeComparator::legacy(N, None).unwrap();
    let iters: u64 = std::env::var("AUDIT_CREDITS_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(400);
    let mut st = Stats::default();
    for it in 0..iters {
        let mut rng = Rng(0x1234_5678_9abc_def1 ^ it.wrapping_mul(0x9E37_79B9));
        let descending = it % 2 == 0;
        let node = random_node(&mut rng, &cmp, descending);
        // start: images-rich keys (numerators on single slots exercise per-slot purity)
        let mut start: BTreeSet<Vec<i32>> = BTreeSet::new();
        while start.len() < 8 {
            let mut k: Vec<i32> = (0..N)
                .map(|s| if LINES[s] { rng.range(1, 3) } else { 0 })
                .collect();
            if rng.next() % 2 == 0 {
                k[3 + (rng.next() % 2) as usize] = rng.range(-2, -1);
            }
            start.insert(k);
        }
        let start = KeySet::from_keys(
            KeyCodec::new(N, 6).unwrap(),
            start.iter().map(|k| k.as_slice()),
        )
        .unwrap();
        st.nodes += 1;
        let kc = cfg(Tier::K, &node.disp, &pol);
        let tk = closure(&node.disp, &node.bodies, &cmp, &start, &kc, &NoFacts).unwrap();
        let tk2 = closure(
            &node.disp,
            &node.bodies,
            &cmp,
            &start,
            &kc,
            &RandFacts { seed: it, decided: false },
        )
        .unwrap();
        let kset: BTreeSet<Vec<i32>> = (0..tk.len()).map(|i| tk.key(i)).collect();
        let kset2: BTreeSet<Vec<i32>> = (0..tk2.len()).map(|i| tk2.key(i)).collect();
        st.tier_k_keys += kset.len();
        if kset != kset2 {
            st.superset_violations.push(format!(
                "node {it}: Tier K key set depends on the facts ({} vs {})",
                kset.len(),
                kset2.len()
            ));
        }
        check_credits(&format!("node {it} K"), &tk, &node, &vg, &cmp, None, &mut st);
        for fs in 0..4u64 {
            let decided = fs % 2 == 0;
            let facts = RandFacts {
                seed: it * 31 + fs,
                decided,
            };
            let cc = cfg(Tier::C, &node.disp, &pol);
            let tc = closure(&node.disp, &node.bodies, &cmp, &start, &cc, &facts).unwrap();
            st.tier_c_runs += 1;
            st.tier_c_keys += tc.len();
            for i in 0..tc.len() {
                let k = tc.key(i);
                if !kset.contains(&k) {
                    st.superset_violations.push(format!(
                        "node {it} facts {fs}: Tier C key {k:?} not reached by Tier K (credited {}, open {})",
                        tc.is_credited(i),
                        tc.is_open(i)
                    ));
                }
            }
            check_credits(&format!("node {it} C{fs}"), &tc, &node, &vg, &cmp, Some(&facts), &mut st);
            let s = tc.summary();
            if descending && decided {
                if s.cycles.unwrap_or(0) == 0 && s.descent_violations == 0 {
                    st.acyclic_runs += 1;
                } else {
                    st.cyclic.push(format!(
                        "node {it} facts {fs}: cycles {:?} descent violations {}",
                        s.cycles, s.descent_violations
                    ));
                }
            }
        }
    }
    println!(
        "STATS nodes {} tierC runs {} keys C {} K {} credits checked {} (with numerator {}) acyclic {}",
        st.nodes,
        st.tier_c_runs,
        st.tier_c_keys,
        st.tier_k_keys,
        st.credits_checked,
        st.credits_with_numerator,
        st.acyclic_runs
    );
    println!("credits through a lifted image (proof bound): {}", st.credits_lifted_image);
    assert!(st.not_below.is_empty(), "{:#?}", &st.not_below[..st.not_below.len().min(10)]);
    assert!(st.superset_violations.is_empty(), "{:#?}", &st.superset_violations[..st.superset_violations.len().min(10)]);
    assert!(st.credit_refuted.is_empty(), "{:#?}", &st.credit_refuted[..st.credit_refuted.len().min(10)]);
    assert!(st.chained.is_empty(), "{:#?}", &st.chained[..st.chained.len().min(10)]);
    assert!(st.cyclic.is_empty(), "{:#?}", &st.cyclic[..st.cyclic.len().min(10)]);
    assert!(st.credits_checked > 0 && st.credits_with_numerator > 0);
}
