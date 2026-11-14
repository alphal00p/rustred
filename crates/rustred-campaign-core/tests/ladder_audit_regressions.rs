//! Regression tests of the decision library for the findings of audit ladder-A (notes/implementer.audit-ladder-A.md;
//! its confirming tests audit_a.rs ported here, inverted so that they FAIL on the audited code 2c82576c and PASS
//! after the fixes) and the mutation-power tests of finding M9 / brief item (i). One test per finding id.

mod ladder_common;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use ladder_common::{ReplayProver, root_item};
use rustred_campaign_core::domain::{CaseDomain, PoleKind, PoleLocus};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, ContainmentCheck, Coverage, CoverageVerdict, Descent, FactSource,
    FirstMatch, KeyProver, NoTables, RepairBudget, RepairFailure, RepairInputs, TreeLimits,
    TreeReport, drive_tree, first_match_with_exclusions, repair_key,
};
use rustred_campaign_core::terminals::{TerminalFlag, check_terminals};
use rustred_campaign_core::worklist::affine::{AffineEq, AffineError};
use rustred_campaign_core::worklist::items::{
    CaseSpec, EvidenceLevel, Face, GapFaceInput, Item, Key, LocusInput, NodeParams,
    PartitionReport, PreferredEntry, PreferredTerminals, items_from_closure_keys,
    items_from_partition,
};
use rustred_campaign_core::worklist::ladder::{
    AttentionReason, CandidateRule, Disposition, ExclusionReason, ExclusionTarget, Interruption,
    ItemState, KeyEvidence, LadderExhaustedRecord, Next, PointFacts, ProofFailure, SolveAnswer,
    SolveOutcome, Tri, judge_points, next_steps,
};
use rustred_campaign_core::worklist::policy::{
    CentreSpec, ClassMatch, PolicyClass, SeedPolicy, SettingSpec, Wave,
};
use rustred_campaign_core::worklist::seed::{AttemptBudget, BudgetClass, seed_space};
use rustred_campaign_core::worklist::supplied::{CandidateSource, SuppliedCandidate};

fn p() -> NodeParams {
    NodeParams::new("P", 6, 3)
}
fn key(a: i32, b: i32) -> Key {
    vec![a, b, 1, 0, 0, 0]
}
fn cmp6() -> NodeComparator {
    NodeComparator::legacy(6, None).unwrap()
}

/// Domain of `case` with the given exception branches (each an AND-list of engine polynomials).
fn dom(p: &NodeParams, case: &CaseSpec, exc: &[&[&str]]) -> CaseDomain {
    let mut d = CaseDomain::free(p.arity, Some(p.lines.clone()));
    let free = case.free();
    for i in 0..p.arity {
        if !free.contains(&i) {
            d.fixed[i] = Some(
                case.fixed
                    .get(&i)
                    .copied()
                    .unwrap_or(if p.lines[i] { 1 } else { 0 }),
            );
        }
    }
    for e in &case.equations {
        d.equations
            .push(Poly::parse_d_free(&e.to_engine(), p.arity).unwrap());
    }
    for b in exc {
        d.exceptions.push(
            b.iter()
                .map(|s| Poly::parse_d_free(s, p.arity).unwrap())
                .collect(),
        );
    }
    d
}

fn rule(d: CaseDomain, tag: &str, rhs: u64) -> CandidateRule {
    CandidateRule {
        domain: d,
        digest: tag.into(),
        rhs_terms: rhs,
        monomials: None,
        max_degree: None,
        bytes: None,
        rows: None,
        tie_break_only: None,
        symmetry_rows: false,
    }
}

/// Facts: regular, complete, rhs = one step down on the first dotted line, except at (rule, key) pairs in `bad`
/// (rhs ABOVE the key: not descending), `unknown` (regularity unknown) and `pole` (regularity fails).
#[derive(Default)]
struct Facts {
    bad: Vec<(String, Key)>,
    unknown: Vec<(String, Key)>,
    pole: Vec<(String, Key)>,
}
impl FactSource for Facts {
    fn facts(&mut self, r: &CandidateRule, k: &[i32]) -> PointFacts {
        let hit = |l: &Vec<(String, Key)>| l.iter().any(|(t, kk)| *t == r.digest && kk.as_slice() == k);
        let mut rhs = k.to_vec();
        if hit(&self.bad) {
            rhs[0] += 3;
            rhs[1] += 3;
        } else if let Some(i) = (0..3).find(|&i| rhs[i] >= 2) {
            rhs[i] -= 1;
        }
        PointFacts {
            regular: if hit(&self.pole) {
                Tri::Fail
            } else if hit(&self.unknown) {
                Tri::Unknown
            } else {
                Tri::Pass
            },
            rhs_top: Some(vec![rhs]),
            exceptions_complete: Tri::Pass,
            note: None,
        }
    }
}

/// Face rule "R" (no exception) from the target setting at dim 2; "C" at every lower dimension (target only).
struct ByDim {
    p: NodeParams,
}
impl CaseSolver for ByDim {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let tag = if req.case.dim() == 2 { "R" } else { "C" };
        let outcome = if req.setting.label().starts_with("target") {
            SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case, &[]), tag, 10)])
        } else {
            SolveOutcome::NoRelation
        };
        SolveAnswer {
            outcome,
            cpu_ms: 5,
            rows: None,
        }
    }
}

/// The rules of a repair outcome as candidates (rebuilt from their written cases; the solvers here write no
/// exceptions for R / C) in rank order, with their exclusions.
fn dispatch_at(p: &NodeParams, out: &rustred_campaign_core::repair::RepairOutcome, k: &[i32]) -> Option<String> {
    let cands: Vec<CandidateRule> = out
        .rules
        .iter()
        .map(|r| rule(dom(p, &r.case, &[]), &r.digest, r.rhs_terms))
        .collect();
    let list: Vec<(&CandidateRule, &[rustred_campaign_core::worklist::ladder::Exclusion])> = cands
        .iter()
        .zip(&out.rules)
        .map(|(c, r)| (c, r.exclusions.as_slice()))
        .collect();
    match first_match_with_exclusions(&list, k) {
        FirstMatch::Rule(i) => Some(out.rules[i].digest.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// C1

#[test]
fn c1_a1_non_descending_point_is_excluded_from_the_accepted_rule() {
    let p = p();
    let cmp = cmp6();
    let k = key(2, 2);
    let mut facts = Facts {
        bad: vec![("R".into(), k.clone())],
        ..Default::default()
    };
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: k.clone(),
        extra_targets: vec![key(3, 3), key(4, 2)],
        tables: &NoTables,
    };
    let out = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut ByDim { p: p.clone() },
        &mut facts,
        &mut ReplayProver,
        None,
    )
    .expect("repair succeeds");
    let r = out.rules.iter().find(|r| r.digest == "R").expect("R accepted");
    assert!(r.case.contains(&k, &p.lines), "k lies in R's written case");
    assert!(
        r.exclusions.iter().any(|e| e.target == ExclusionTarget::Key(k.clone())
            && matches!(e.reason, ExclusionReason::NotDescending { .. })),
        "R must carry the exclusion of k: {:?}",
        r.exclusions
    );
    assert!(!r.credited.contains(&k));
    // dispatch over the returned rules in rank order with exclusions: k goes to the child's rule, never to R
    assert_eq!(dispatch_at(&p, &out, &k).as_deref(), Some("C"));
    for kk in [key(3, 3), key(4, 2)] {
        assert_eq!(dispatch_at(&p, &out, &kk).as_deref(), Some("R"));
    }
    // ... and the order of R and C does not matter at the keys asked about (exclusions both ways)
    let c = out.rules.iter().find(|r| r.digest == "C").unwrap();
    assert!(c.serves_key && out.dispatch.after_tables_in_force);
    for kk in [key(3, 3), key(4, 2)] {
        let cr = rule(dom(&p, &c.case, &[]), "C", 1);
        let cx = c.exclusions.as_slice();
        if cr.domain.applies(&kk).applies() {
            assert!(cx.iter().any(|e| e.target.holds(&kk)));
        }
    }
    assert!(r.rank < c.rank, "parent before its children");
}

struct FailAt {
    tag: String,
    k: Key,
    failure: Option<ProofFailure>,
    level: Option<EvidenceLevel>,
}
impl KeyProver for FailAt {
    fn prove(&mut self, r: &CandidateRule, keys: &[Key], wanted: EvidenceLevel) -> Vec<KeyEvidence> {
        keys.iter()
            .map(|kk| {
                if r.digest == self.tag && *kk == self.k {
                    KeyEvidence {
                        key: kk.clone(),
                        level: self.level,
                        failure: self.failure.clone(),
                    }
                } else {
                    KeyEvidence {
                        key: kk.clone(),
                        level: Some(wanted),
                        failure: None,
                    }
                }
            })
            .collect()
    }
}

#[test]
fn c1_a1b_identity_failed_point_is_excluded_from_the_accepted_rule() {
    let p = p();
    let cmp = cmp6();
    let k = key(2, 2);
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: k.clone(),
        extra_targets: vec![key(3, 3), key(4, 2)],
        tables: &NoTables,
    };
    let mut prover = FailAt {
        tag: "R".into(),
        k: k.clone(),
        failure: Some(ProofFailure::NonMembership),
        level: None,
    };
    let out = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut ByDim { p: p.clone() },
        &mut Facts::default(),
        &mut prover,
        None,
    )
    .expect("repair succeeds");
    let r = out.rules.iter().find(|r| r.digest == "R").expect("R accepted");
    assert!(r.exclusions.iter().any(|e| e.target == ExclusionTarget::Key(k.clone())
        && matches!(
            e.reason,
            ExclusionReason::IdentityFailed {
                failure: ProofFailure::NonMembership,
                ..
            }
        )));
    assert_eq!(dispatch_at(&p, &out, &k).as_deref(), Some("C"));
}

/// A preferred KEY inside a returned rule's domain is excluded (P-4 (b), audit A note).
#[test]
fn c1_preferred_key_inside_a_repair_rule_is_excluded() {
    let mut p = p();
    p.preferred = PreferredTerminals {
        entries: vec![PreferredEntry::Key(key(5, 5))],
        ..Default::default()
    };
    let cmp = cmp6();
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(2, 2),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let out = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut ByDim { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    )
    .unwrap();
    let r = &out.rules[0];
    assert!(r.exclusions.iter().any(|e| e.target == ExclusionTarget::Key(key(5, 5))
        && e.reason == ExclusionReason::PreferredTerminal));
}

/// A rule whose POLE locus vanishes at a gap point is excluded there (a pole BLOCKS first match: without the
/// exclusion the child could never serve the key).
#[test]
fn c1_pole_point_is_excluded_and_the_child_is_reached() {
    let p = p();
    let cmp = cmp6();
    struct PoleSolver {
        p: NodeParams,
    }
    impl CaseSolver for PoleSolver {
        fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            let outcome = if !req.setting.label().starts_with("target") {
                SolveOutcome::NoRelation
            } else if req.case.dim() == 2 {
                let mut d = dom(&self.p, &req.case, &[]);
                d.poles.push(PoleLocus::new(
                    PoleKind::Denominator,
                    &Poly::parse_d_free("n0-n1", self.p.arity).unwrap(),
                ));
                SolveOutcome::Rules(vec![rule(d, "R", 10)])
            } else {
                SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case, &[]), "C", 10)])
            };
            SolveAnswer {
                outcome,
                cpu_ms: 5,
                rows: None,
            }
        }
    }
    let k = key(2, 2);
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: k.clone(),
        extra_targets: vec![key(3, 2)],
        tables: &NoTables,
    };
    let out = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut PoleSolver { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    )
    .expect("repair succeeds");
    let r = out.rules.iter().find(|r| r.digest == "R").unwrap();
    assert!(r.exclusions.iter().any(|e| e.target.holds(&k)), "{:?}", r.exclusions);
    assert!(out.rules.iter().any(|r| r.digest == "C" && r.serves_key));
}

// ---------------------------------------------------------------------------------------------------------------
// C2

struct AllCover {
    p: NodeParams,
}
impl CaseSolver for AllCover {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        SolveAnswer {
            outcome: SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case, &[]), "all", 10)]),
            cpu_ms: 5,
            rows: None,
        }
    }
}

/// Drive one item through `next_steps` with a scripted evidence answer; returns the disposition and the number of
/// evidence rounds.
fn drive_item(
    item: Item,
    p: &NodeParams,
    solver: &mut dyn CaseSolver,
    facts: &mut dyn FactSource,
    mut evidence: impl FnMut(&[Key]) -> Vec<KeyEvidence>,
    max_loops: usize,
) -> (Option<Disposition>, usize) {
    let cmp = cmp6();
    let mut st = ItemState::new(item);
    let mut rounds = 0;
    for _ in 0..max_loops {
        match next_steps(&st, p) {
            Next::Run(list) => {
                let a = &list[0];
                let ans = solver.solve(&CaseRequest::from_attempt(&p.node, a), &a.budget);
                st.record_answer(a, ans);
            }
            Next::NeedVerdicts(r) => {
                for q in r {
                    let c = st.records[&q.attempt_key].candidates()[q.index].clone();
                    let v = judge_points(&c, &q.keys, facts, &cmp);
                    st.record_verdicts(&q.attempt_key, q.index, v);
                }
            }
            Next::NeedEvidence(r) => {
                rounds += 1;
                for q in r {
                    let ev = evidence(&q.keys);
                    st.record_evidence(&q.attempt_key, q.index, ev);
                }
            }
            Next::NeedOffers => st.record_offers("none", vec![]),
            Next::NeedContainment(r) => {
                for q in r {
                    st.record_containment(&q.attempt_key, q.index, Tri::Unknown);
                }
            }
            Next::Done(d) => return (Some(d), rounds),
        }
    }
    (None, rounds)
}

#[test]
fn c2_a2_unknown_evidence_terminates_and_never_credits() {
    let p = p();
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2), key(3, 2)]);
    let (d, rounds) = drive_item(
        item,
        &p,
        &mut AllCover { p: p.clone() },
        &mut Facts::default(),
        |keys| {
            keys.iter()
                .map(|k| KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: None,
                })
                .collect()
        },
        2000,
    );
    let d = d.expect("terminates");
    assert!(rounds <= 64, "evidence asked at most once per candidate: {rounds}");
    // nothing credited: no cover member
    match &d {
        Disposition::Covered { cover } => panic!("covered without evidence: {cover:?}"),
        Disposition::Split { cover, .. } => assert!(cover.members.is_empty()),
        _ => {}
    }
    // an empty evidence answer is "no answer" (not re-requested)
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let (d, rounds) = drive_item(
        item,
        &p,
        &mut AllCover { p: p.clone() },
        &mut Facts::default(),
        |_| vec![],
        2000,
    );
    assert!(d.is_some() && rounds <= 64);
}

/// The item's GLOBAL step bound: an always-operator-interrupting solver ends in ATTENTION (sanity bound, U-5), a
/// driver that keeps answering ends at `max_item_steps`; drive_tree's own step bound stops the run.
#[test]
fn c2_step_bounds_hold_for_operator_interrupts_and_the_tree() {
    struct Interrupt;
    impl CaseSolver for Interrupt {
        fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            SolveAnswer {
                outcome: SolveOutcome::Interrupted {
                    by: Interruption::Operator,
                },
                cpu_ms: 0,
                rows: None,
            }
        }
    }
    let mut p = p();
    p.budgets.max_operator_interrupts = 3;
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let (d, _) = drive_item(item.clone(), &p, &mut Interrupt, &mut Facts::default(), |_| vec![], 100);
    assert!(matches!(
        d,
        Some(Disposition::Attention {
            reason: AttentionReason::OperatorInterrupts { .. },
            ..
        })
    ));
    p.budgets.max_operator_interrupts = u32::MAX;
    p.budgets.max_item_steps = 40;
    let (d, _) = drive_item(item.clone(), &p, &mut Interrupt, &mut Facts::default(), |_| vec![], 1000);
    assert!(matches!(
        d,
        Some(Disposition::Attention {
            reason: AttentionReason::StepBound { .. },
            ..
        })
    ));
    p.budgets.max_item_steps = u64::MAX;
    let rep = drive_tree(
        vec![item],
        &p,
        &cmp6(),
        &mut Interrupt,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits {
            max_steps: 77,
            ..TreeLimits::default()
        },
    );
    assert_eq!(rep.stopped.as_deref(), Some("step limit 77"));
    assert!(rep.steps <= 77);
}

// ---- adversarial traits (C2 property test) ----

/// xorshift64* (deterministic, no dependency).
#[derive(Clone)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Random,
    Erratic,
    AlwaysUnknown,
    Contradictory,
}

struct AdvSolver {
    p: NodeParams,
    rng: Rng,
    mode: Mode,
}
impl CaseSolver for AdvSolver {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let r = &mut self.rng;
        let outcome = match r.below(10) {
            0 => SolveOutcome::NoRelation,
            1 => SolveOutcome::Error("adv".into()),
            2 => SolveOutcome::KilledMemory,
            3 => SolveOutcome::BudgetStop {
                time_decided: r.chance(1, 2),
            },
            4 => SolveOutcome::RefusedHeavy { reason: "adv".into() },
            5 => SolveOutcome::Interrupted {
                by: if r.chance(1, 2) {
                    Interruption::Operator
                } else {
                    Interruption::Infra
                },
            },
            6 if self.mode == Mode::AlwaysUnknown => SolveOutcome::ProfileRefused { reason: "adv".into() },
            _ => {
                let n = r.below(4) as usize;
                let mut rules = Vec::new();
                for j in 0..n {
                    let mut d = dom(&self.p, &req.case, &[]);
                    for s in req.case.free() {
                        if r.chance(1, 3) {
                            let v = if self.p.lines[s] { 2 + r.below(3) as i32 } else { -1 - r.below(2) as i32 };
                            d.exceptions.push(vec![
                                Poly::parse_d_free(&AffineEq::fixed(s, v).to_engine(), self.p.arity).unwrap(),
                            ]);
                        }
                    }
                    if r.chance(1, 6) && req.case.free().len() >= 2 {
                        let f = req.case.free();
                        d.poles.push(PoleLocus::new(
                            PoleKind::Denominator,
                            &Poly::parse_d_free(&format!("n{}-n{}", f[0], f[1]), self.p.arity).unwrap(),
                        ));
                    }
                    rules.push(rule(d, &format!("adv{}-{j}", r.next() % 1000), 1 + r.below(5000)));
                }
                SolveOutcome::Rules(rules)
            }
        };
        SolveAnswer {
            outcome,
            cpu_ms: self.rng.below(10_000),
            rows: None,
        }
    }
}

struct AdvFacts {
    rng: Rng,
    mode: Mode,
}
impl FactSource for AdvFacts {
    fn facts(&mut self, _r: &CandidateRule, k: &[i32]) -> PointFacts {
        let r = &mut self.rng;
        let tri = |r: &mut Rng| match r.below(3) {
            0 => Tri::Pass,
            1 => Tri::Fail,
            _ => Tri::Unknown,
        };
        match self.mode {
            Mode::AlwaysUnknown => PointFacts {
                regular: Tri::Unknown,
                rhs_top: None,
                exceptions_complete: Tri::Unknown,
                note: None,
            },
            _ => {
                let mut down = k.to_vec();
                if let Some(i) = (0..3).find(|&i| down[i] >= 2) {
                    down[i] -= 1;
                }
                let mut up = k.to_vec();
                up[0] += 1;
                let rhs = match r.below(4) {
                    0 => None,
                    1 => Some(vec![up]),
                    _ => Some(vec![down]),
                };
                PointFacts {
                    regular: if r.chance(3, 4) { Tri::Pass } else { tri(r) },
                    rhs_top: rhs,
                    exceptions_complete: if r.chance(3, 4) { Tri::Pass } else { tri(r) },
                    note: None,
                }
            }
        }
    }
}

struct AdvDescent {
    rng: Cell<u64>,
    mode: Mode,
    cmp: NodeComparator,
}
impl Descent for AdvDescent {
    fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        let mut r = Rng(self.rng.get());
        let x = r.next();
        self.rng.set(r.0);
        match self.mode {
            Mode::Erratic => x % 2 == 0,
            Mode::Contradictory => true, // lower(a, b) and lower(b, a) both "true"
            _ => self.cmp.lower(a, b),
        }
    }
}

struct AdvProver {
    rng: Rng,
    mode: Mode,
}
impl KeyProver for AdvProver {
    fn prove(&mut self, _r: &CandidateRule, keys: &[Key], wanted: EvidenceLevel) -> Vec<KeyEvidence> {
        let r = &mut self.rng;
        let mut out = Vec::new();
        for k in keys {
            if self.mode == Mode::AlwaysUnknown {
                out.push(KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: None,
                });
                continue;
            }
            match r.below(7) {
                0 => {}
                1 => out.push(KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: None,
                }),
                2 => out.push(KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: Some(ProofFailure::Undecided("adv".into())),
                }),
                3 => {
                    out.push(KeyEvidence {
                        key: k.clone(),
                        level: Some(wanted),
                        failure: None,
                    });
                    out.push(KeyEvidence {
                        key: k.clone(),
                        level: None,
                        failure: Some(ProofFailure::NonMembership),
                    });
                }
                4 => out.push(KeyEvidence {
                    key: k.clone(),
                    level: Some(EvidenceLevel::A1),
                    failure: None,
                }),
                _ => out.push(KeyEvidence {
                    key: k.clone(),
                    level: Some(wanted),
                    failure: None,
                }),
            }
            if r.chance(1, 10) {
                out.push(KeyEvidence {
                    key: vec![9; k.len()],
                    level: Some(EvidenceLevel::A4),
                    failure: None,
                });
            }
        }
        out
    }
}

struct AdvContain(Rng);
impl ContainmentCheck for AdvContain {
    fn contains(&mut self, _r: &CandidateRule, _old: &[String]) -> Tri {
        match self.0.below(3) {
            0 => Tri::Pass,
            1 => Tri::Fail,
            _ => Tri::Unknown,
        }
    }
}

struct AdvSource {
    p: NodeParams,
    rng: Rng,
}
impl CandidateSource for AdvSource {
    fn name(&self) -> &str {
        "adv"
    }
    fn offer(&mut self, item: &Item, _p: &NodeParams) -> Vec<SuppliedCandidate> {
        if self.rng.chance(2, 3) {
            return vec![];
        }
        vec![SuppliedCandidate {
            candidate: rule(dom(&self.p, &item.case, &[]), "offer", 3),
            record: serde_json::json!({}),
            source_rule: "adv".into(),
            instance: "0".into(),
        }]
    }
}

/// Fail-closed recheck of evidence (independent of the library): every entry for the key passes at `min`.
fn evidence_passes(st: &ItemState, ak: &str, idx: usize, k: &Key, min: EvidenceLevel) -> bool {
    if min <= EvidenceLevel::A1 {
        return true;
    }
    let Some(ev) = st.evidence.get(&(ak.to_string(), idx)) else {
        return false;
    };
    let mine: Vec<&KeyEvidence> = ev.iter().filter(|e| &e.key == k).collect();
    !mine.is_empty()
        && mine
            .iter()
            .all(|e| e.failure.is_none() && e.level.is_some_and(|l| l >= min))
}

/// Safety properties of a finished tree run (independent rechecks).
fn check_tree(rep: &TreeReport, p: &NodeParams) {
    let mut pts: BTreeSet<Key> = BTreeSet::new();
    for r in &rep.items {
        pts.extend(r.state.item.gap_points.iter().cloned());
    }
    let members = rep.accepted_members();
    let mut cands = Vec::new();
    for (idx, m) in &members {
        let st = &rep.items[*idx].state;
        let v = st.verdicts.get(&(m.attempt_key.clone(), m.index)).expect("judged");
        for k in &m.served {
            assert!(m.credited.contains(k), "served but not credited");
        }
        for k in &m.credited {
            let i = st.item.gap_points.iter().position(|g| g == k).expect("credited key is a gap point of its item");
            assert!(v[i].covered(), "credited without a covered verdict");
            assert!(evidence_passes(st, &m.attempt_key, m.index, k, p.min_evidence), "credited without evidence");
        }
        cands.push((rep.rule_of(*idx, m).unwrap(), m.exclusions.as_slice()));
    }
    // order-free safety at every gap point: whatever rule first-match selects is credited there
    for k in &pts {
        for order in [false, true] {
            let mut list = cands.clone();
            let mut ms: Vec<usize> = (0..members.len()).collect();
            if order {
                list.reverse();
                ms.reverse();
            }
            match first_match_with_exclusions(&list, k) {
                FirstMatch::Rule(i) => assert!(members[ms[i]].1.credited.contains(k), "dispatch to an uncredited rule at {k:?}"),
                FirstMatch::Blocked(i) => panic!("blocked by {} at {k:?}", members[ms[i]].1.digest),
                FirstMatch::None => {}
            }
        }
    }
    // terminals only from complete verdict ladders
    for r in &rep.items {
        if let Some(Disposition::TerminalCandidates { exhausted, .. }) = &r.disposition {
            let space: Vec<_> = seed_space(&r.state.item.case, p).into_iter().flatten().collect();
            assert_eq!(exhausted.settings() as usize, space.len());
            for s in &space {
                let k = rustred_campaign_core::worklist::ladder::attempt_key(&r.state.item.id, s);
                assert!(r.state.records.get(&k).is_some_and(|x| x.is_verdict()), "terminal without a verdict");
            }
        }
    }
}

#[test]
fn c2_adversarial_traits_terminate_within_the_bounds_10000_runs() {
    let runs: u64 = std::env::var("LADDER_ADV_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    let modes = [Mode::Random, Mode::Erratic, Mode::AlwaysUnknown, Mode::Contradictory];
    let mut max_steps_seen = 0u64;
    for run in 0..runs {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ (run.wrapping_mul(0x1000_0001_B3)).wrapping_add(1));
        let mode = modes[(run % 4) as usize];
        let mut p = p();
        p.budgets.max_item_steps = 300;
        p.budgets.nonverdict_retries = r.below(3) as u32;
        p.budgets.infra_retries = r.below(3) as u32;
        p.budgets.max_operator_interrupts = r.below(4) as u32;
        p.min_evidence = if r.chance(1, 2) { EvidenceLevel::A2 } else { EvidenceLevel::A3 };
        let sd: Vec<usize> = if r.chance(1, 2) { vec![0] } else { vec![0, 1] };
        let sn: Vec<usize> = if r.chance(1, 3) { vec![3] } else { vec![] };
        let face = Face::new(sd.clone(), sn.clone());
        let mut pts = Vec::new();
        for _ in 0..(1 + r.below(5)) {
            let mut k = vec![1, 1, 1, 0, 0, 0];
            for &s in &sd {
                k[s] = 2 + r.below(3) as i32;
            }
            for &s in &sn {
                k[s] = -1 - r.below(2) as i32;
            }
            pts.push(k);
        }
        let cmp = cmp6();
        let mut solver = AdvSolver { p: p.clone(), rng: Rng(r.next() | 1), mode };
        let mut facts = AdvFacts { rng: Rng(r.next() | 1), mode };
        let descent = AdvDescent { rng: Cell::new(r.next() | 1), mode, cmp };
        let mut prover = AdvProver { rng: Rng(r.next() | 1), mode };
        let limits = TreeLimits {
            max_attempts: 400,
            max_cpu_ms: u64::MAX,
            max_items: 40,
            max_steps: 4000,
        };
        if run % 2 == 0 {
            let mut contain = AdvContain(Rng(r.next() | 1));
            let mut src = AdvSource { p: p.clone(), rng: Rng(r.next() | 1) };
            let root = root_item(&p, face, pts);
            let rep = drive_tree(
                vec![root],
                &p,
                &descent,
                &mut solver,
                &mut facts,
                &mut prover,
                Some(&mut contain),
                Some(&mut src),
                &limits,
            );
            assert!(rep.steps <= limits.max_steps);
            max_steps_seen = max_steps_seen.max(rep.steps);
            for it in &rep.items {
                // one round may record several answers beyond the check: bounded by the requests of that round
                assert!(it.state.steps <= p.budgets.max_item_steps + 64, "item steps {}", it.state.steps);
                assert!(it.disposition.is_some() || rep.stopped.is_some());
            }
            check_tree(&rep, &p);
        } else {
            let k = pts[0].clone();
            let inp = RepairInputs {
                params: &p,
                descent: &descent,
                key: k.clone(),
                extra_targets: pts[1..].to_vec(),
                tables: &NoTables,
            };
            let budget = RepairBudget {
                max_attempts: 400,
                max_cpu_ms: u64::MAX,
                max_items: 40,
                max_steps: 4000,
            };
            match repair_key(&inp, &budget, &mut solver, &mut facts, &mut prover, None) {
                Ok(out) => {
                    let s = out.rules.iter().find(|x| x.serves_key).expect("a serving rule");
                    assert!(s.credited.contains(&k));
                }
                Err(RepairFailure::Terminal { exhausted, .. }) => {
                    assert_eq!(exhausted.key(), k.as_slice());
                }
                Err(_) => {}
            }
        }
    }
    eprintln!("C2: {runs} adversarial runs terminated; max tree steps {max_steps_seen}");
}

// ---------------------------------------------------------------------------------------------------------------
// C3

struct Crash;
impl CaseSolver for Crash {
    fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        SolveAnswer {
            outcome: SolveOutcome::Error("segfault".into()),
            cpu_ms: 1,
            rows: None,
        }
    }
}

#[test]
fn c3_a3_crash_only_ladder_is_attention_never_terminal() {
    let p = p();
    let cmp = cmp6();
    let k = key(3, 2);
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: k.clone(),
        extra_targets: vec![],
        tables: &NoTables,
    };
    match repair_key(&inp, &RepairBudget::default(), &mut Crash, &mut Facts::default(), &mut ReplayProver, None) {
        Err(RepairFailure::Attention {
            reason: AttentionReason::NonVerdict { settings },
            attempts,
            ..
        }) => {
            assert!(!settings.is_empty());
            assert!(attempts.iter().all(|a| a.outcome == "error"));
        }
        Err(RepairFailure::BudgetExhausted { .. }) => {}
        other => panic!("{other:?}"),
    }
    // every non-verdict kind, mixed with verdicts on the other settings: still no terminal
    for bad in [
        SolveOutcome::KilledMemory,
        SolveOutcome::BudgetStop { time_decided: true },
        SolveOutcome::BudgetStop { time_decided: false },
        SolveOutcome::RefusedHeavy { reason: "x".into() },
        SolveOutcome::ProfileRefused { reason: "x".into() },
    ] {
        struct OneBad(SolveOutcome);
        impl CaseSolver for OneBad {
            fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
                let o = if r.setting.depth == 3 && !r.setting.floor {
                    self.0.clone()
                } else {
                    SolveOutcome::NoRelation
                };
                SolveAnswer { outcome: o, cpu_ms: 1, rows: None }
            }
        }
        let r = repair_key(&inp, &RepairBudget::default(), &mut OneBad(bad.clone()), &mut Facts::default(), &mut ReplayProver, None);
        assert!(
            matches!(r, Err(RepairFailure::Attention { .. })),
            "{bad:?}: {r:?}"
        );
    }
    // all verdicts (no relation anywhere): a terminal proposal
    struct Nothing;
    impl CaseSolver for Nothing {
        fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            SolveAnswer { outcome: SolveOutcome::NoRelation, cpu_ms: 1, rows: None }
        }
    }
    assert!(matches!(
        repair_key(&inp, &RepairBudget::default(), &mut Nothing, &mut Facts::default(), &mut ReplayProver, None),
        Err(RepairFailure::Terminal { .. })
    ));
}

/// A non-verdict outcome is retried in the larger (oversized) resource class; operator interrupts never count (U-5).
#[test]
fn c3_nonverdict_retry_in_larger_class_and_operator_interrupts_do_not_count() {
    let mut p = p();
    p.budgets.nonverdict_retries = 1;
    let item = root_item(&p, Face::new(vec![0], vec![]), vec![key(2, 1)]);
    let mut st = ItemState::new(item);
    st.record_offers("none", vec![]);
    let Next::Run(list) = next_steps(&st, &p) else { panic!() };
    let a = list[0].clone();
    assert_ne!(a.budget.class, BudgetClass::Oversized);
    for _ in 0..5 {
        st.record_answer(&a, SolveAnswer { outcome: SolveOutcome::Interrupted { by: Interruption::Operator }, cpu_ms: 0, rows: None });
    }
    st.record_answer(&a, SolveAnswer { outcome: SolveOutcome::KilledMemory, cpu_ms: 1, rows: None });
    let Next::Run(list) = next_steps(&st, &p) else { panic!() };
    let retry = list.iter().find(|x| x.key == a.key).expect("retried");
    assert_eq!(retry.budget.class, BudgetClass::Oversized);
    assert!(!st.records[&a.key].settled(&p.budgets), "operator interrupts did not use up the cap");
    st.record_answer(retry, SolveAnswer { outcome: SolveOutcome::Error("x".into()), cpu_ms: 1, rows: None });
    assert!(st.records[&a.key].settled_without_verdict(&p.budgets));
}

// ---------------------------------------------------------------------------------------------------------------
// M1 (a): the reserve no longer loses to point rules

struct ReserveSolver {
    p: NodeParams,
}
impl CaseSolver for ReserveSolver {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let l = req.setting.label();
        let outcome = match req.case.dim() {
            2 if l == "target/unfl/d2" => SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case, &[&["n0-2"]]), "A", 10)]),
            2 if l == "line:0/fl/d2" => SolveOutcome::Rules(vec![rule(
                dom(&self.p, &req.case, &[&["n0-4"], &["n0-2", "n1-4"]]),
                "B",
                10,
            )]),
            0 => SolveOutcome::Rules(vec![rule(dom(&self.p, &req.case, &[]), "PT", 2000)]),
            _ => SolveOutcome::NoRelation,
        };
        SolveAnswer { outcome, cpu_ms: 5, rows: None }
    }
}

#[test]
fn m1_a5_reserve_beats_point_rules() {
    let p = p();
    let mut grid = Vec::new();
    for a in 2..5 {
        for b in 2..5 {
            grid.push(key(a, b));
        }
    }
    let root = root_item(&p, Face::new(vec![0, 1], vec![]), grid);
    let rep = drive_tree(
        vec![root],
        &p,
        &cmp6(),
        &mut ReserveSolver { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let acc: Vec<(String, Vec<Key>)> = rep.accepted_rules().into_iter().map(|(_, m)| (m.digest, m.served)).collect();
    let points = acc.iter().filter(|(d, _)| d == "PT").count();
    assert!(acc.iter().any(|(d, _)| d == "B"), "{acc:?}");
    assert_eq!(points, 1, "{acc:?}");
    assert_eq!(acc.len(), 3, "A + B + one point rule: {acc:?}");
    check_tree(&rep, &p);
}

// ---------------------------------------------------------------------------------------------------------------
// M2 (b): conflicting evidence, failure wins

#[test]
fn m2_a2b_conflicting_evidence_failure_wins() {
    let p = p();
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let (d, _) = drive_item(
        item,
        &p,
        &mut AllCover { p: p.clone() },
        &mut Facts::default(),
        |keys| {
            let mut ev = Vec::new();
            for k in keys {
                ev.push(KeyEvidence { key: k.clone(), level: None, failure: Some(ProofFailure::NonMembership) });
                ev.push(KeyEvidence { key: k.clone(), level: Some(EvidenceLevel::A3), failure: None });
            }
            ev
        },
        5000,
    );
    let d = d.unwrap();
    assert!(!matches!(d, Disposition::Covered { .. }), "{d:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// M3 (c): preferred top sector, bound

#[test]
fn m3_a6_preferred_top_sector_is_ignored_and_preferences_are_bounded() {
    let mut p = p();
    p.preferred = PreferredTerminals {
        entries: vec![PreferredEntry::Sector { lines: vec![0, 1, 2] }],
        ..Default::default()
    };
    let keys: Vec<Key> = (2..40).map(|a| key(a, 2)).collect();
    let items = items_from_closure_keys(&keys, &p).unwrap();
    assert_eq!(items.iter().map(|i| i.gap_points.len()).sum::<usize>(), keys.len());
    assert!(p.preferred.warnings(&p.lines).iter().any(|w| w.contains("not a proper subsector")));
    let terms: Vec<(Key, Option<rustred_campaign_core::worklist::ladder::LadderExhausted>)> =
        keys.iter().map(|k| (k.clone(), None)).collect();
    assert!(check_terminals(&terms, 1, &NoTables, &p.preferred, &p.lines)
        .iter()
        .any(|f| matches!(f, TerminalFlag::OverBound { .. })));
    // explicit keys: at most max_preferred honoured, the surplus "not applied" (stays a gap point)
    p.preferred = PreferredTerminals {
        entries: keys.iter().map(|k| PreferredEntry::Key(k.clone())).collect(),
        max_preferred: Some(3),
        ..Default::default()
    };
    let mut items = vec![root_item(&p, Face::new(vec![0], vec![]), keys.clone())];
    let rep = p.preferred.strip(&mut items, &p.lines, p.terminals_max);
    assert_eq!(rep.kept_preferred.len(), 3);
    assert_eq!(rep.not_applied.len(), keys.len() - 3);
    assert_eq!(items[0].gap_points.len(), keys.len() - 3);
}

// ---------------------------------------------------------------------------------------------------------------
// M4 (d): undecided tables at a terminal

#[test]
fn m4_a9_undecided_terminal_is_flagged() {
    struct Und;
    impl Coverage for Und {
        fn verdict(&self, _k: &[i32]) -> CoverageVerdict {
            CoverageVerdict::Undecided { why: "on pole".into() }
        }
    }
    let p = p();
    let flags = check_terminals(&[(key(2, 2), None)], 10, &Und, &PreferredTerminals::default(), &p.lines);
    assert!(matches!(flags.as_slice(), [TerminalFlag::UndecidedTerminal { .. }]), "{flags:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// M5 (e): forgeable proof objects

#[test]
fn m5_a3b_token_record_must_be_reverified() {
    // the JSON form deserialises only into the UNVERIFIED record; verifying it against an item that never ran fails
    let rec: LadderExhaustedRecord = serde_json::from_str(
        r#"{"item":"x","key":[9,9,1,0,0,0],"attempts":0,"settings":0,"depth3_tried":true,"depth3_unfloored_tried":true,"parent":null}"#,
    )
    .unwrap();
    let p = p();
    let st = ItemState::new(root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(9, 9)]));
    assert!(rec.verify(&st, &p).is_err());
    // a genuine token round-trips through its record against the same state
    struct Nothing;
    impl CaseSolver for Nothing {
        fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            SolveAnswer { outcome: SolveOutcome::NoRelation, cpu_ms: 1, rows: None }
        }
    }
    let item = root_item(&p, Face::new(vec![], vec![]), vec![vec![1, 1, 1, 0, 0, 0]]);
    let rep = drive_tree(vec![item], &p, &cmp6(), &mut Nothing, &mut Facts::default(), &mut ReplayProver, None, None, &TreeLimits::default());
    let (_, tok) = rep.terminal_candidates().into_iter().next().expect("terminal");
    let rec = tok.to_record();
    let text = serde_json::to_string(&rec).unwrap();
    let back: LadderExhaustedRecord = serde_json::from_str(&text).unwrap();
    assert_eq!(back.verify(&rep.items[0].state, &p).unwrap(), tok);
    let mut forged = back.clone();
    forged.settings += 1;
    assert!(forged.verify(&rep.items[0].state, &p).is_err());
}

// ---------------------------------------------------------------------------------------------------------------
// M6 (f): full attempt ended before any box item

#[test]
fn m6_box_item_needs_the_full_attempt_ended() {
    use rustred_campaign_core::worklist::items::{FullAttemptEnded, FullAttemptEvidence, UserBox, WorklistError, box_restricted_item};
    let mut p = p();
    p.user_box = Some(UserBox { id: "B".into(), given_by: "user".into() });
    p.full_attempt.fixer_rounds = 2;
    let ev = FullAttemptEvidence { node: "P".into(), cpu_ms: 0, fixer_rounds: 1, open: vec!["x".into()] };
    assert!(matches!(FullAttemptEnded::from_evidence(&p, ev.clone()), Err(WorklistError::FullAttemptNotEnded(_))));
    let ended = FullAttemptEnded::from_evidence(&p, FullAttemptEvidence { fixer_rounds: 2, ..ev }).unwrap();
    assert!(box_restricted_item(CaseSpec::face_case(Face::new(vec![0], vec![])), vec![], &p, &ended).is_ok());
}

// ---------------------------------------------------------------------------------------------------------------
// M7 (g): a spec policy replaces the space

#[test]
fn m7_a8_policy_replaces_the_builtin_space() {
    let mut p = p();
    let class = |replace: bool, depths: &[u8]| PolicyClass {
        matcher: ClassMatch { node_class: "*".into(), face_type: "*".into() },
        waves: depths
            .iter()
            .map(|&d| Wave { depth: d, settings: vec![SettingSpec { centre: CentreSpec::Target, floor: None, tight: None }] })
            .collect(),
        tight: false,
        budgets: None,
        replace_builtin: replace,
    };
    p.policy = SeedPolicy {
        schema: rustred_campaign_core::worklist::policy::SEED_POLICY_SCHEMA.into(),
        name: "only-target".into(),
        source: "test".into(),
        classes: vec![class(true, &[2])],
        evidence: vec![],
    };
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    let w = seed_space(&case, &p);
    assert_eq!(w.iter().map(|x| x.len()).sum::<usize>(), 1, "{w:?}");
    p.policy.classes = vec![class(true, &[4, 2])];
    let w = seed_space(&case, &p);
    assert_eq!(w.iter().map(|x| x[0].depth).collect::<Vec<_>>(), vec![4, 2]);
    let text = serde_json::to_string(&p.policy).unwrap();
    assert!(SeedPolicy::from_json(&text).is_ok(), "depth 4 accepted");
    // default (no replace): the built-in space is still appended (completeness rule)
    p.policy.classes = vec![class(false, &[2])];
    assert!(seed_space(&case, &p).iter().map(|x| x.len()).sum::<usize>() > 1);
}

// ---------------------------------------------------------------------------------------------------------------
// M8 (h): point loci go through the face case

#[test]
fn m8_point_locus_points_are_gap_points_of_the_face_item() {
    let p = p();
    let face = Face::new(vec![0, 1], vec![]);
    let rep = PartitionReport {
        faces: vec![GapFaceInput {
            face: face.clone(),
            generic: false,
            points: vec![],
            loci: vec![LocusInput {
                equations: vec![AffineEq::parse("n0-3", 6).unwrap(), AffineEq::parse("n1-2", 6).unwrap()],
                points: vec![key(3, 2)],
                proof: "exact".into(),
            }],
            out_of_box: false,
            proof: "exact".into(),
        }],
    };
    let (items, _) = items_from_partition(&rep, &p).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].case, CaseSpec::face_case(face));
    assert_eq!(items[0].gap_points, vec![key(3, 2)]);
}

// ---------------------------------------------------------------------------------------------------------------
// M9 (i): tests with power. Each fails under the named mutation of ladder.rs (checked by hand, see the notes).

/// One target rule on face {0,1}; gap points (2,2), (3,2); facts / prover as given; returns whether (2,2) is
/// CREDITED by the cover (member credited set) and the disposition.
fn credit_of_22(facts: &mut Facts, prover: &mut dyn KeyProver, p: &NodeParams) -> (bool, Disposition) {
    let item = root_item(p, Face::new(vec![0, 1], vec![]), vec![key(2, 2), key(3, 2)]);
    let rep = drive_tree(
        vec![item],
        p,
        &cmp6(),
        &mut ByDim { p: p.clone() },
        facts,
        prover,
        None,
        None,
        &TreeLimits { max_items: 1, ..TreeLimits::default() },
    );
    let d = rep.items[0].disposition.clone().unwrap();
    let credited = match &d {
        Disposition::Covered { cover } | Disposition::Split { cover, .. } => {
            cover.members.iter().any(|m| m.digest == "R" && m.credited.contains(&key(2, 2)))
        }
        _ => false,
    };
    (credited, d)
}

#[test]
fn m9_mutation_skip_the_evidence_check() {
    let p = p();
    let mut prover = FailAt { tag: "R".into(), k: key(2, 2), failure: Some(ProofFailure::Fail), level: None };
    let (c, d) = credit_of_22(&mut Facts::default(), &mut prover, &p);
    assert!(!c, "{d:?}");
}

#[test]
fn m9_mutation_credit_unknown_coverage() {
    let p = p();
    let mut f = Facts { unknown: vec![("R".into(), key(2, 2))], ..Default::default() };
    let (c, d) = credit_of_22(&mut f, &mut ReplayProver, &p);
    assert!(!c, "{d:?}");
}

#[test]
fn m9_mutation_credit_a_non_descending_point() {
    let p = p();
    let mut f = Facts { bad: vec![("R".into(), key(2, 2))], ..Default::default() };
    let (c, d) = credit_of_22(&mut f, &mut ReplayProver, &p);
    assert!(!c, "{d:?}");
}

#[test]
fn m9_mutation_ignore_a_pole() {
    let p = p();
    let mut f = Facts { pole: vec![("R".into(), key(2, 2))], ..Default::default() };
    let (c, d) = credit_of_22(&mut f, &mut ReplayProver, &p);
    assert!(!c, "{d:?}");
    // the whole candidate is rejected (D-L2: a hidden pole rejects the candidate)
    match d {
        Disposition::Split { cover, .. } => assert!(cover.members.iter().all(|m| m.digest != "R")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn m9_mutation_accept_below_the_required_level() {
    let mut p = p();
    p.min_evidence = EvidenceLevel::A3;
    let mut prover = FailAt { tag: "R".into(), k: key(2, 2), failure: None, level: Some(EvidenceLevel::A2) };
    let (c, d) = credit_of_22(&mut Facts::default(), &mut prover, &p);
    assert!(!c, "{d:?}");
}

#[test]
fn m9_mutation_skip_the_exhaustion_requirement() {
    // a point item whose ONE deep setting crashed: no terminal (would be one without the verdict requirement)
    let p = p();
    struct DeepCrash;
    impl CaseSolver for DeepCrash {
        fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            let o = if r.setting.depth == 3 { SolveOutcome::Error("x".into()) } else { SolveOutcome::NoRelation };
            SolveAnswer { outcome: o, cpu_ms: 1, rows: None }
        }
    }
    let item = root_item(&p, Face::new(vec![], vec![]), vec![vec![1, 1, 1, 0, 0, 0]]);
    let rep = drive_tree(vec![item], &p, &cmp6(), &mut DeepCrash, &mut Facts::default(), &mut ReplayProver, None, None, &TreeLimits::default());
    assert!(rep.terminal_candidates().is_empty());
    assert!(matches!(
        rep.items[0].disposition,
        Some(Disposition::Attention { reason: AttentionReason::NonVerdict { .. }, .. })
    ));
}

// ---------------------------------------------------------------------------------------------------------------
// minor: affine overflow is reported, not truncated

#[test]
fn minor_a7_affine_overflow_is_reported() {
    assert_eq!(
        AffineEq::try_new(vec![(0, i64::MAX), (0, i64::MAX)], 1),
        Err(AffineError::Overflow)
    );
    assert!(AffineEq::new(vec![(0, i64::MAX), (0, i64::MAX)], 1).is_none());
    let n = AffineEq::new(vec![(3, -2), (1, -4)], 6).unwrap();
    assert_eq!((n.coeffs.clone(), n.constant), (vec![(1, 2), (3, 1)], -3));
    let _ = BTreeMap::<u8, u8>::new();
}
