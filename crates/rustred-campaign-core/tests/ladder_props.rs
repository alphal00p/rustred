//! Property tests of the ladder state machine: termination counters (random answer sequences never exceed them, no
//! ladder cycle), determinism (same answers -> same disposition; a batch driver answering whole waves and the
//! sequential driver agree when the economy extra budget does not cut the sequential run).

mod ladder_common;

use std::collections::BTreeMap;

use ladder_common::*;
use proptest::prelude::*;
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::repair::{CaseRequest, CaseSolver, TreeLimits, drive_tree};
use rustred_campaign_core::worklist::affine::AffineEq;
use rustred_campaign_core::worklist::items::{Face, Item, NodeParams, fnv1a128};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, Interruption, ItemState, Next, SolveAnswer, SolveOutcome,
    judge_points, next_steps, sub_counter,
};
use rustred_campaign_core::worklist::seed::AttemptBudget;

/// Answers as a pure function of (seed, attempt key, attempt number): no relation, a rule excepting a random subset
/// of the case's points (as single-slot planes), an interrupt (operator or infra), an error.
struct RandomSolver {
    seed: u64,
    params: NodeParams,
    calls: BTreeMap<String, u32>,
}

impl RandomSolver {
    fn h(&self, s: &str, k: u32) -> u64 {
        (fnv1a128(format!("{}|{s}|{k}", self.seed).as_bytes()) & 0xffff_ffff_ffff) as u64
    }
}

impl CaseSolver for RandomSolver {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let n = self.calls.entry(req.attempt_key.clone()).or_insert(0);
        *n += 1;
        let n = *n;
        let h = self.h(&req.attempt_key, n);
        let outcome = match h % 10 {
            0..=2 => SolveOutcome::NoRelation,
            3 if n < 3 => SolveOutcome::Interrupted {
                by: Interruption::Operator,
            },
            4 if n < 3 => SolveOutcome::Interrupted {
                by: Interruption::Infra,
            },
            5 => SolveOutcome::Error("planted".into()),
            _ => {
                let p = &self.params;
                let mut d = CaseDomain::free(p.arity, Some(p.lines.clone()));
                let free: Vec<usize> = req.case.free();
                for i in 0..p.arity {
                    if !free.contains(&i) {
                        d.fixed[i] =
                            Some(req.case.fixed.get(&i).copied().unwrap_or(if p.lines[i] {
                                1
                            } else {
                                0
                            }));
                    }
                }
                for e in &req.case.equations {
                    d.equations
                        .push(Poly::parse_d_free(&e.to_engine(), p.arity).unwrap());
                }
                // exceptions: a random slot at a random small value
                for (j, &s) in free.iter().enumerate() {
                    if (h >> (8 + j)) & 1 == 1 {
                        let v = if p.lines[s] {
                            2 + ((h >> (16 + j)) & 1) as i32
                        } else {
                            -1 - ((h >> (20 + j)) & 1) as i32
                        };
                        d.exceptions.push(vec![
                            Poly::parse_d_free(&AffineEq::fixed(s, v).to_engine(), p.arity)
                                .unwrap(),
                        ]);
                    }
                }
                SolveOutcome::Rules(vec![CandidateRule {
                    domain: d,
                    digest: format!("r{h:x}"),
                    rhs_terms: 100 + h % 1000,
                    monomials: None,
                    max_degree: None,
                    bytes: None,
                    rows: None,
                    tie_break_only: None,
                    symmetry_rows: false, witness: rustred_campaign_core::cover::WitnessStatus::Complete,
                }])
            }
        };
        SolveAnswer {
            outcome,
            cpu_ms: 1 + h % 5000,
            rows: None,
        }
    }
}

fn params() -> NodeParams {
    let mut p = NodeParams::new("P", 8, 4);
    p.budgets.economy_extra_factor = 1.0e12;
    p
}

fn face_points(face: &Face, p: &NodeParams) -> Vec<Vec<i32>> {
    let mut pts = Vec::new();
    let slots = face.slots();
    let mut idx = vec![0i32; slots.len()];
    loop {
        let mut k: Vec<i32> = (0..p.arity)
            .map(|i| if p.lines[i] { 1 } else { 0 })
            .collect();
        for (j, &s) in slots.iter().enumerate() {
            k[s] = if p.lines[s] { 2 + idx[j] } else { -1 - idx[j] };
        }
        pts.push(k);
        let mut j = 0;
        while j < idx.len() {
            idx[j] += 1;
            if idx[j] < 3 {
                break;
            }
            idx[j] = 0;
            j += 1;
        }
        if j == idx.len() {
            break;
        }
    }
    pts
}

/// Batch driver of ONE item: answers every attempt of a Run at once.
fn batch_one(item: Item, p: &NodeParams, solver: &mut RandomSolver) -> (ItemState, Disposition) {
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 4,
        line_priority: (0..4).collect(),
    };
    let mut st = ItemState::new(item);
    for _ in 0..100_000 {
        match next_steps(&st, p) {
            Next::Run(list) => {
                for a in &list {
                    let ans = solver.solve(&CaseRequest::from_attempt(&p.node, a), &a.budget);
                    st.record_answer(a, ans);
                }
            }
            Next::NeedVerdicts(r) => {
                for q in r {
                    let c = st.records[&q.attempt_key].candidates()[q.index].clone();
                    let v = judge_points(&c, &q.keys, &mut facts, &cmp);
                    st.record_verdicts(&q.attempt_key, q.index, v);
                }
            }
            Next::NeedEvidence(r) => {
                for q in r {
                    let c = st.records[&q.attempt_key].candidates()[q.index].clone();
                    let ev = rustred_campaign_core::repair::KeyProver::prove(
                        &mut ReplayProver,
                        &c,
                        &q.keys,
                        q.level,
                    );
                    st.record_evidence(&q.attempt_key, q.index, ev);
                }
            }
            Next::NeedContainment(_) => unreachable!(),
            Next::NeedOffers => st.record_offers("none", vec![]),
            Next::Done(d) => return (st, d),
        }
    }
    panic!("no disposition within 100,000 steps");
}

fn summary(d: &Disposition) -> String {
    match d {
        Disposition::Covered { cover } => format!(
            "covered {:?}",
            cover.members.iter().map(|m| &m.digest).collect::<Vec<_>>()
        ),
        Disposition::Split {
            cover, children, ..
        } => format!(
            "split {:?} {:?}",
            cover.members.iter().map(|m| &m.digest).collect::<Vec<_>>(),
            children.iter().map(|c| &c.id).collect::<Vec<_>>()
        ),
        Disposition::TerminalCandidates { keys, .. } => format!("terminal {keys:?}"),
        Disposition::Attention { why, .. } => format!("attention {why}"),
        Disposition::OptimiseKept { why } => format!("kept {why}"),
        Disposition::RecipeRule { rule, .. } => format!("recipe {}", rule.recipe_digest),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, .. ProptestConfig::default() })]

    #[test]
    fn counters_bound_attempts_and_trees_terminate(seed in any::<u64>(), sd in 1usize..3, sn in 0usize..2) {
        let p = params();
        let face = Face::new((0..sd).collect(), (4..4 + sn).collect());
        let pts = face_points(&face, &p);
        let root = root_item(&p, face, pts);
        let mut solver = RandomSolver { seed, params: p.clone(), calls: BTreeMap::new() };
        let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
        let mut facts = ScriptedFacts { cmp: &cmp, n_lines: 4, line_priority: (0..4).collect() };
        let rep = drive_tree(vec![root.clone()], &p, &cmp, &mut solver, &mut facts, &mut ReplayProver, None, None, &TreeLimits::default());
        prop_assert!(rep.stopped.is_none());
        for r in &rep.items {
            // seed-search sub-counter: every setting answered at most once (interrupts excluded)
            prop_assert!(r.state.attempts_made() <= sub_counter(&r.state, &p));
            // dimension strictly decreases along the tree: depth <= root dimension
            prop_assert!(r.state.item.depth <= root.root_dim);
            prop_assert!(r.state.item.case.dim() + r.state.item.depth <= root.root_dim);
            prop_assert!(r.disposition.is_some());
        }
        // interrupts never exceed the infra cap without attention; operator interrupts are uncapped and not attempts
        let total_final: usize = rep.items.iter().map(|r| r.state.attempts_made()).sum();
        prop_assert!(total_final <= rep.attempts.len());
    }

    #[test]
    fn same_answers_same_disposition_batch_equals_sequential(seed in any::<u64>(), sd in 1usize..3, sn in 0usize..2) {
        let p = params();
        let face = Face::new((0..sd).collect(), (4..4 + sn).collect());
        let pts = face_points(&face, &p);
        let root = root_item(&p, face, pts);
        let mut s1 = RandomSolver { seed, params: p.clone(), calls: BTreeMap::new() };
        let mut s2 = RandomSolver { seed, params: p.clone(), calls: BTreeMap::new() };
        let (_, d1) = batch_one(root.clone(), &p, &mut s1);
        let (_, d1b) = batch_one(root.clone(), &p, &mut s2);
        prop_assert_eq!(summary(&d1), summary(&d1b));
        let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
        let mut facts = ScriptedFacts { cmp: &cmp, n_lines: 4, line_priority: (0..4).collect() };
        let mut s3 = RandomSolver { seed, params: p.clone(), calls: BTreeMap::new() };
        let lim = TreeLimits { max_items: 1, ..TreeLimits::default() };
        let rep = drive_tree(vec![root], &p, &cmp, &mut s3, &mut facts, &mut ReplayProver, None, None, &lim);
        let d3 = rep.items[0].disposition.clone().unwrap();
        // the sequential driver appends RESERVE members to a split's cover after the children's subtrees (none
        // processed here); the primary decision must be identical
        match (&d1, &d3) {
            (Disposition::Split { cover: c1, children: k1, .. }, Disposition::Split { cover: c3, children: k3, .. }) => {
                let a: Vec<&String> = c1.members.iter().map(|m| &m.digest).collect();
                let b: Vec<&String> = c3.members.iter().map(|m| &m.digest).take(a.len()).collect();
                prop_assert_eq!(a, b);
                prop_assert_eq!(
                    k1.iter().map(|c| &c.id).collect::<Vec<_>>(),
                    k3.iter().map(|c| &c.id).collect::<Vec<_>>()
                );
            }
            _ => prop_assert_eq!(summary(&d1), summary(&d3)),
        }
    }
}
