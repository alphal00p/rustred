//! AUDIT r2-poles (independent auditor, question 5): NON-INTERFERENCE of pole results.
//! 1. `poles::pole_objective` (THE selection) under every objective (measure subsets and orders, size ratios) and with
//!    or without an eps box: it returns a candidate iff one is admissible, never a non-admissible one, always one of the
//!    highest evidence level, and with ONE admissible candidate always that one (the call sites closeloop/accept.rs and
//!    termmin/machine.rs pass exactly one candidate: there the pole code can only annotate).
//! 2. The seed minimiser (seedmin) with `pole_objective: off`: the chosen lists, relations and solve counts do NOT
//!    depend on the pole orders the judge reports (byte-identical decisions with all-regular vs random poles).
//! 3. With `pole_objective: on` (default): a key served by its start list is served by the minimised list whatever the
//!    poles (a pole only refuses drops), and the minimised relation never has a WORSE known pole order than the start.
//!
//! Pure, no Symbolica. Toy data (arity 3), seeded PRNG.

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::epsdepth::epsbox::EpsBox;
use rustred_campaign_core::poles::{
    Candidate, MaxSizeRatio, Measure, PoleObjective, PoleProfile, pole_objective,
};
use rustred_campaign_core::seedmin::{
    EngineCaps, Key, Offset, PoleOrder, RelationFacts, SeedMinPolicy, StubSystemPoles, Trial,
    TrialAnswer, TrialMode, TrialSolver, minimise_key, sha256_hex,
};
use serde_json::json;

struct Mix(u64);
impl Mix {
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
}

const ALL: [Measure; 6] = [
    Measure::EpsBoxExcess,
    Measure::Regular,
    Measure::MaxPoleOrder,
    Measure::SumPoleOrders,
    Measure::SingularTerms,
    Measure::Size,
];

#[test]
fn audit_pole_objective_only_chooses_among_admissible_candidates() {
    let mut r = Mix(20261115);
    let terminals: Vec<Vec<i32>> = vec![vec![1, 1, 1], vec![2, 1, 1]];
    let boxes = [
        None,
        Some(EpsBox::from_value(&json!({"needed_order": 0, "available": {"default": 1}})).unwrap()),
    ];
    let ratios = [
        MaxSizeRatio::Unbounded,
        MaxSizeRatio::parse(&json!(1)).unwrap(),
        MaxSizeRatio::parse(&json!("3/2")).unwrap(),
        MaxSizeRatio::parse(&json!(8)).unwrap(),
    ];
    for case in 0..4000 {
        let n = 1 + r.below(6) as usize;
        let cands: Vec<Candidate> = (0..n)
            .map(|i| Candidate {
                id: format!("c{i:02}"),
                admissible: r.below(4) != 0,
                excluded_by: (r.below(5) == 0).then(|| "cap".to_string()),
                evidence_level: r.below(3) as u32,
                size: 1 + r.below(100),
                pole_order: match r.below(5) {
                    0 => None,
                    x => Some(x - 1),
                },
            })
            .collect();
        let profiles: Vec<PoleProfile> = if r.below(2) == 0 {
            vec![]
        } else {
            (0..n)
                .map(|_| PoleProfile {
                    local: (0..r.below(4)).map(|_| r.below(3) as u32).collect(),
                    composed: (r.below(2) == 0).then(|| {
                        terminals
                            .iter()
                            .map(|t| (t.clone(), (r.below(6) != 0).then(|| r.below(3) as u32)))
                            .collect()
                    }),
                })
                .collect()
        };
        // a random non-empty subset of measures in a random order
        let mut ms: Vec<Measure> = ALL.to_vec();
        for i in (1..ms.len()).rev() {
            ms.swap(i, r.below(i as u64 + 1) as usize);
        }
        ms.truncate(1 + r.below(6) as usize);
        let obj = PoleObjective {
            measures: ms,
            max_size_ratio: ratios[case % ratios.len()],
        };
        let bx = boxes[case % 2].as_ref();
        let got = pole_objective(&cands, &profiles, &obj, bx).expect("covering eps box");
        let adm: Vec<&Candidate> = cands.iter().filter(|c| c.admissible).collect();
        match got {
            None => assert!(
                adm.is_empty(),
                "case {case}: no choice although admissible exist"
            ),
            Some(ch) => {
                let c = &cands[ch.index];
                assert!(c.admissible, "case {case}: non-admissible chosen");
                let top = adm.iter().map(|c| c.evidence_level).max().unwrap();
                assert_eq!(
                    c.evidence_level, top,
                    "case {case}: lower evidence level chosen"
                );
                if adm.len() == 1 {
                    assert_eq!(
                        c.id, adm[0].id,
                        "case {case}: the only admissible candidate"
                    );
                }
                // order-independence: the same choice when the input is reversed
                let rev: Vec<Candidate> = cands.iter().rev().cloned().collect();
                let revp: Vec<PoleProfile> = profiles.iter().rev().cloned().collect();
                let g2 = pole_objective(&rev, &revp, &obj, bx).unwrap().unwrap();
                assert_eq!(
                    g2.id, ch.id,
                    "case {case}: choice depends on the input order"
                );
            }
        }
    }
}

/// One way a key solves: needed seeds, relation size, pole order.
#[derive(Clone)]
struct Alt {
    needs: Vec<Offset>,
    rhs: u64,
    pole: PoleOrder,
}

#[derive(Clone, Default)]
struct Oracle {
    alts: BTreeMap<Key, Vec<Alt>>,
}

impl TrialSolver for Oracle {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        Ok(trials
            .iter()
            .map(|t| {
                let have: BTreeSet<&Offset> = t.list.iter().collect();
                t.keys
                    .iter()
                    .map(|k| {
                        if let TrialMode::Probe { .. } = t.mode {
                            return TrialAnswer::Probe {
                                verdict: "NO_RELATION_AT_D0".into(),
                            };
                        }
                        for (i, a) in self.alts.get(k).into_iter().flatten().enumerate() {
                            if a.needs.iter().all(|o| have.contains(o)) {
                                return TrialAnswer::Relation(Box::new(RelationFacts {
                                    relation_digest: sha256_hex(format!("{k:?}/{i}").as_bytes()),
                                    rhs_count: a.rhs,
                                    rhs_sub_count: None,
                                    bytes: None,
                                    trace_rows: None,
                                    seeds_used: None,
                                    pole: a.pole,
                                    minimise: None,
                                }));
                            }
                        }
                        TrialAnswer::NoRelation
                    })
                    .collect()
            })
            .collect())
    }
    fn capabilities(&self) -> EngineCaps {
        EngineCaps::default()
    }
}

fn shell() -> Vec<Offset> {
    let mut v = vec![];
    for a in -1..=1 {
        for b in -1..=1 {
            for c in -1..=1 {
                v.push(vec![a, b, c]);
            }
        }
    }
    v
}

/// A random toy: per key 1..4 alternatives, each needing 1..4 seeds of the shell (the last alternative needs the
/// centre only so that the start list always solves), sizes and pole orders random.
fn toy(r: &mut Mix, keys: usize, poles: &mut dyn FnMut(&mut Mix) -> PoleOrder) -> Oracle {
    let sh = shell();
    let mut o = Oracle::default();
    for k in 0..keys {
        let key: Key = vec![k as i32 + 2, 1, -1];
        let na = 1 + r.below(4) as usize;
        let mut alts = vec![];
        for _ in 0..na {
            let ns = 1 + r.below(4) as usize;
            let needs: Vec<Offset> = (0..ns).map(|_| sh[r.below(27) as usize].clone()).collect();
            alts.push(Alt {
                needs,
                rhs: 5 + r.below(60),
                pole: poles(r),
            });
        }
        o.alts.insert(key, alts);
    }
    o
}

fn random_pole(r: &mut Mix) -> PoleOrder {
    match r.below(5) {
        0 => PoleOrder::Undecided,
        1 => PoleOrder::Singular,
        x => PoleOrder::Order {
            order: (x - 2) as u32,
        },
    }
}

#[test]
fn audit_seedmin_pole_objective_off_is_independent_of_the_poles() {
    let policy = SeedMinPolicy {
        pole_objective: "off".into(),
        ..SeedMinPolicy::default()
    };
    for seed in 0..60u64 {
        // the same toy twice: the structure from one PRNG stream, the poles from another
        let mut ra = Mix(seed);
        let mut rb = Mix(seed);
        let mut pa = Mix(1000 + seed);
        let a = toy(&mut ra, 6, &mut |_r| PoleOrder::regular());
        let b = toy(&mut rb, 6, &mut |_r| random_pole(&mut pa));
        for key in a.alts.keys() {
            let (mut sa, mut sb) = (a.clone(), b.clone());
            let ka = minimise_key(
                key,
                &[0, 0, 0],
                &shell(),
                &policy,
                &mut sa,
                &mut StubSystemPoles,
            )
            .unwrap();
            let kb = minimise_key(
                key,
                &[0, 0, 0],
                &shell(),
                &policy,
                &mut sb,
                &mut StubSystemPoles,
            )
            .unwrap();
            assert_eq!(
                ka.list, kb.list,
                "seed {seed} key {key:?}: list depends on poles with pole_objective off"
            );
            assert_eq!(
                ka.relation.as_ref().map(|r| r.relation_digest.clone()),
                kb.relation.as_ref().map(|r| r.relation_digest.clone()),
                "seed {seed} key {key:?}: relation depends on poles"
            );
            assert_eq!(
                ka.solves, kb.solves,
                "seed {seed} key {key:?}: work depends on poles"
            );
            assert_eq!(ka.stopped, kb.stopped);
        }
    }
}

#[test]
fn audit_seedmin_pole_objective_on_never_unserves_and_never_worsens() {
    for undecided in ["keep_seed", "drop"] {
        // the guarantee of the POLE-FIRST objective (explicit since rule 21, 2026-11-19: the default puts terms first and
        // may trade a pole order for fewer terms, by design)
        let policy = SeedMinPolicy {
            pole_objective: "on".into(),
            on_undecided_regularity: undecided.into(),
            objective: vec!["pole_order".into(), "seeds".into(), "terms".into()],
            ..SeedMinPolicy::default()
        };
        for seed in 0..60u64 {
            let mut r = Mix(seed);
            let mut pr = Mix(7000 + seed);
            let o = toy(&mut r, 6, &mut |_r| random_pole(&mut pr));
            for key in o.alts.keys() {
                let mut s = o.clone();
                let start = s.solve(&[Trial {
                    keys: vec![key.clone()],
                    centre: vec![0, 0, 0],
                    list: shell(),
                    label: "start".into(),
                    mode: TrialMode::Solve,
                    engine_minimise: None,
                }]);
                let served_at_start = matches!(start.unwrap()[0][0], TrialAnswer::Relation(_));
                let km = minimise_key(
                    key,
                    &[0, 0, 0],
                    &shell(),
                    &policy,
                    &mut s,
                    &mut StubSystemPoles,
                )
                .unwrap();
                if served_at_start {
                    let rel = km.relation.as_ref().unwrap_or_else(|| {
                        panic!(
                            "seed {seed} key {key:?} ({undecided}): served at start, unserved after"
                        )
                    });
                    let st = km.start_relation.as_ref().unwrap();
                    if let (Some(a), Some(b)) = (rel.pole.order(), st.pole.order()) {
                        assert!(
                            a <= b,
                            "seed {seed} key {key:?}: pole order worsened {b} -> {a}"
                        );
                    }
                }
            }
        }
    }
}

/// FINDING (MAJOR, latent: no call site passes composed profiles yet): two ALTERNATIVE relations of ONE key have the SAME
/// composed vector on a terminal basis (the reduction onto a basis is unique; POLES_OBJECTIVE 2 A), so their composed
/// profiles always tie and `pole_objective` falls through to SIZE: a locally singular relation (route pole, spurious)
/// beats a regular one of any size. POLES_OBJECTIVE.md (binding, overrides the addenda) keeps the user default "regular
/// at d0 always wins" for relations (smaller intermediate expressions, no cancellations to carry); with the default
/// `max_size_ratio` "unbounded" the regular candidate must win.
#[test]
fn audit_pole_objective_composed_tie_keeps_regular_first() {
    let t: Vec<(Vec<i32>, Option<u32>)> = vec![(vec![1, 1, 1], Some(0)), (vec![2, 1, 1], Some(0))];
    let cands = vec![
        Candidate {
            id: "regular-big".into(),
            admissible: true,
            excluded_by: None,
            evidence_level: 0,
            size: 100,
            pole_order: Some(0),
        },
        Candidate {
            id: "singular-small".into(),
            admissible: true,
            excluded_by: None,
            evidence_level: 0,
            size: 10,
            pole_order: Some(1),
        },
    ];
    let profiles = vec![
        PoleProfile {
            local: vec![0, 0],
            composed: Some(t.clone()),
        },
        PoleProfile {
            local: vec![1, 0],
            composed: Some(t),
        },
    ];
    // USER RULING 2026-11-19 (rule 21): by DEFAULT size comes first, so the smaller singular relation wins
    let ch = pole_objective(&cands, &profiles, &PoleObjective::default(), None)
        .unwrap()
        .unwrap();
    assert_eq!(
        ch.id, "singular-small",
        "chosen {} ({:?})",
        ch.id, ch.reason
    );
    // the EXPLICIT withdrawn order (pole measures before size): the composed tie must not let size decide
    let ch = pole_objective(
        &cands,
        &profiles,
        &PoleObjective::withdrawn_pole_first(),
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(ch.id, "regular-big", "chosen {} ({:?})", ch.id, ch.reason);
}
