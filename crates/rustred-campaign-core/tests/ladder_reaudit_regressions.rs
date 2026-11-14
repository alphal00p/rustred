//! Regression tests of lane ladderfix2 for the re-audits of the repair ladder (notes/implementer.reaudit-ladder-A.md
//! and notes/implementer.reaudit-ladder-B.md). Each auditor reproduction is kept with its name (`r1`..`r10` of
//! re-audit A, the B findings by id) and now asserts the FIXED behaviour: every one of these tests failed at
//! b5ba6b33 (the auditors' confirming tests asserted the faulty behaviour there).

mod ladder_common;

use std::collections::{BTreeMap, BTreeSet};

use ladder_common::{ReplayProver, root_item};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::economy::EconomyWeights;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, FactSource, FirstMatch, NoTables, PartitionShadowCheck, RepairBudget,
    RepairFailure, RepairInputs, ShadowCheck, TreeLimits, WinnerCheck, drive_tree,
    drive_tree_with, first_match_with_exclusions, repair_key, seal_tree,
};
use rustred_campaign_core::terminals::{TerminalContext, TerminalFlag, check_terminals};
use rustred_campaign_core::worklist::items::{
    CaseSpec, EvidenceLevel, Face, FullAttemptEnded, FullAttemptEvidence, Item, Key, NodeParams,
    PreferredEntry, PreferredTerminals, ProverCapabilities,
};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, DropReason, Exclusion, ExclusionReason, ExclusionTarget, Next,
    PointFacts, ShadowVerdict, SolveAnswer, SolveOutcome, Tri,
};
use rustred_campaign_core::worklist::orbits::{GroupElement, SymmetryGroup};
use rustred_campaign_core::worklist::recipe::{
    KeyRelation, Recipe, RecipeEconomy, RecipeFailure, accept_recipe, default_recipes,
};
use rustred_campaign_core::worklist::seed::{AttemptBudget, SeedSetting, attempt_budget, seed_space};

fn p() -> NodeParams {
    NodeParams::new("P", 6, 3)
}
fn key(a: i32, b: i32) -> Key {
    vec![a, b, 1, 0, 0, 0]
}
fn cmp6() -> NodeComparator {
    NodeComparator::legacy(6, None).unwrap()
}
fn dom(p: &NodeParams, case: &CaseSpec) -> CaseDomain {
    case.case_domain(&p.lines).expect("domain")
}
fn rule(d: CaseDomain, tag: &str, rhs: u64, sym: bool) -> CandidateRule {
    CandidateRule {
        domain: d,
        digest: tag.into(),
        rhs_terms: rhs,
        monomials: None,
        max_degree: None,
        bytes: None,
        rows: None,
        tie_break_only: None,
        symmetry_rows: sym,
        witness: rustred_campaign_core::cover::WitnessStatus::Complete,
    }
}
/// Regular, complete, rhs one step down; NOT descending at the (rule tag, key) pairs in `bad`.
#[derive(Default)]
struct Facts {
    bad: Vec<(String, Key)>,
    calls: u64,
}
impl FactSource for Facts {
    fn facts(&mut self, r: &CandidateRule, k: &[i32]) -> PointFacts {
        self.calls += 1;
        let mut rhs = k.to_vec();
        if self.bad.iter().any(|(t, kk)| *t == r.digest && kk.as_slice() == k) {
            rhs[0] += 3;
        } else if let Some(i) = (0..3).find(|&i| rhs[i] >= 2) {
            rhs[i] -= 1;
        }
        PointFacts {
            regular: Tri::Pass,
            rhs_top: Some(vec![rhs]),
            exceptions_complete: Tri::Pass,
            note: None,
        }
    }
}
fn swap01() -> SymmetryGroup {
    SymmetryGroup {
        arity: 6,
        elements: vec![
            GroupElement {
                slot_image: (0..6).map(Some).collect(),
            },
            GroupElement {
                slot_image: vec![Some(1), Some(0), Some(2), Some(3), Some(4), Some(5)],
            },
        ],
        verified: true,
        source: "test data".into(),
    }
}
fn ans(o: SolveOutcome, cpu: u64) -> SolveAnswer {
    SolveAnswer {
        outcome: o,
        cpu_ms: cpu,
        rows: None,
    }
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r1 / brief item 5: the INFORMATION-only "symmetry rows at the target alone" attempt (depth 0, never
// acceptable without a capability) does NOT gate the terminal rung: an engine refusing depth 0 still gives a terminal.

struct RefuseDepth0;
impl CaseSolver for RefuseDepth0 {
    fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let o = if r.setting.symmetry_rows && r.setting.depth == 0 {
            SolveOutcome::Error("max-depth 0 refused".into())
        } else {
            SolveOutcome::NoRelation
        };
        ans(o, 1)
    }
}

#[test]
fn r1_information_only_symmetry_attempt_never_blocks_a_terminal() {
    let cmp = cmp6();
    let point = vec![1, 1, 1, 0, 0, 0];
    let mut p1 = p();
    p1.symmetry = Some(swap01());
    let case = CaseSpec::face_case(Face::new(vec![], vec![]));
    let sp = seed_space(&case, &p1);
    assert!(sp[0].iter().any(|s| s.symmetry_rows && s.depth == 0), "info attempt kept");
    let inp = RepairInputs {
        params: &p1,
        descent: &cmp,
        key: point.clone(),
        extra_targets: vec![],
        tables: &NoTables,
    };
    match repair_key(
        &inp,
        &RepairBudget::default(),
        &mut RefuseDepth0,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    ) {
        Err(RepairFailure::Terminal { exhausted, .. }) => {
            assert!(
                exhausted
                    .information()
                    .iter()
                    .any(|(l, o)| l.contains("/sym") && o == "error"),
                "{:?}",
                exhausted.information()
            );
            assert!(!exhausted.awaiting_certificate_route());
        }
        other => panic!("expected a terminal proposal, got {other:?}"),
    }
}

/// re-audit A r1b / brief item 7: a symmetry-only relation gives a terminal PROPOSAL marked "awaiting a certificate
/// route" (never accepted, never a failure), with the relation's digest in the token and a terminal flag.
#[test]
fn r1b_symmetry_only_relation_marks_the_terminal_awaiting_a_certificate_route() {
    struct SymFinds {
        p: NodeParams,
    }
    impl CaseSolver for SymFinds {
        fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            let o = if r.setting.symmetry_rows {
                SolveOutcome::Rules(vec![rule(dom(&self.p, &r.case), "sym", 3, true)])
            } else {
                SolveOutcome::NoRelation
            };
            ans(o, 1)
        }
    }
    let cmp = cmp6();
    let mut p1 = p();
    p1.symmetry = Some(swap01());
    let point = vec![1, 1, 1, 0, 0, 0];
    let inp = RepairInputs {
        params: &p1,
        descent: &cmp,
        key: point.clone(),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let mut facts = Facts::default();
    match repair_key(
        &inp,
        &RepairBudget::default(),
        &mut SymFinds { p: p1.clone() },
        &mut facts,
        &mut ReplayProver,
        None,
    ) {
        Err(RepairFailure::Terminal { exhausted, .. }) => {
            assert!(exhausted.awaiting_certificate_route());
            assert_eq!(exhausted.uncertified_relations()[0].1, "sym");
            let rec = serde_json::to_string(&exhausted.to_record()).unwrap();
            assert!(rec.contains("sym"), "{rec}");
            let flags = check_terminals(
                &[(point.clone(), Some(exhausted.clone()))],
                10,
                &NoTables,
                &PreferredTerminals::default(),
                &TerminalContext {
                    lines: &p1.lines,
                    honoured: &BTreeSet::new(),
                    explicit: &BTreeSet::new(),
                },
            );
            assert!(flags.iter().any(|f| matches!(f, TerminalFlag::AwaitingCertificateRoute { .. })));
            assert!(flags.iter().all(|f| !f.is_blocking()), "{flags:?}");
        }
        other => panic!("{other:?}"),
    }
    // capability check BEFORE any work (re-audit B M1 / brief item 2): the uncertifiable candidate was never judged
    assert_eq!(facts.calls, 0, "no facts were fetched for an uncertifiable candidate");
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r2 / M2: the retry "in the larger resource class" has a LARGER work counter.

#[test]
fn r2_oversized_retry_has_a_larger_work_counter() {
    let mut p = p();
    p.ibp_rows_per_seed = 100;
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    for s in seed_space(&case, &p).iter().flatten() {
        let std = attempt_budget(s, &case, &p, false, false);
        let big = attempt_budget(s, &case, &p, false, true);
        assert_eq!(big.max_rows, std.max_rows * p.budgets.oversized_work_factor, "{}", s.label());
        assert!(big.max_rows > std.max_rows);
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Brief item 6 (re-audit A ruling 4.1): a WORK-COUNTER budget stop in the largest class counts as "tried within
// budget", is FLAGGED in the token / terminal check, and a run with a LARGER budget retries exactly those settings
// (the stored record does not shadow the retry).

/// Stops by work counter whenever the attempt's counter is below `need`, else finds nothing.
struct WorkLimited {
    need: u64,
    calls: Vec<(String, u64)>,
}
impl CaseSolver for WorkLimited {
    fn solve(&mut self, r: &CaseRequest, b: &AttemptBudget) -> SolveAnswer {
        self.calls.push((r.setting.label(), b.max_rows));
        let o = if !r.setting.symmetry_rows && r.setting.label().starts_with("target") && b.max_rows < self.need {
            SolveOutcome::BudgetStop {
                time_decided: false,
            }
        } else {
            SolveOutcome::NoRelation
        };
        ans(o, 1)
    }
}

#[test]
fn item6_work_counter_stop_is_tried_within_budget_and_retried_with_a_larger_budget() {
    let cmp = cmp6();
    let mut p = p();
    p.ibp_rows_per_seed = 10;
    let point = vec![1, 1, 1, 0, 0, 0];
    let item = root_item(&p, Face::new(vec![], vec![]), vec![point.clone()]);
    let mut solver = WorkLimited {
        need: u64::MAX,
        calls: vec![],
    };
    let rep = drive_tree(
        vec![item.clone()],
        &p,
        &cmp,
        &mut solver,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let terms = rep.terminal_candidates();
    assert_eq!(terms.len(), 1, "{:?}", rep.items[0].disposition);
    let tok = &terms[0].1;
    assert!(tok.within_budget(), "flagged");
    assert!(tok.budget_limited().iter().any(|(l, _)| l.starts_with("target")));
    let flags = check_terminals(
        &[(point.clone(), Some(tok.clone()))],
        10,
        &NoTables,
        &PreferredTerminals::default(),
        &TerminalContext {
            lines: &p.lines,
            honoured: &BTreeSet::new(),
            explicit: &BTreeSet::new(),
        },
    );
    assert!(flags.iter().any(|f| matches!(f, TerminalFlag::WithinBudget { .. })));
    // the token re-verifies against the recorded state
    let st = rep.items[0].state.clone();
    assert!(tok.to_record().verify(&st, &p).is_ok());
    // a larger budget: the recorded work-counter stop is OPEN again (not shadowed): the same state now asks to re-run
    // exactly that setting, in the oversized class with the larger counter
    let mut p2 = p.clone();
    p2.budgets.oversized_work_factor *= 10;
    match rustred_campaign_core::worklist::ladder::next_steps(&st, &p2) {
        Next::Run(list) => {
            assert_eq!(list.len(), 1, "{list:?}");
            assert!(list[0].setting.label().starts_with("target"));
            assert_eq!(list[0].budget.class, rustred_campaign_core::worklist::seed::BudgetClass::Oversized);
        }
        other => panic!("{other:?}"),
    }
    // and with the larger budget the solver finishes the shells: a plain (not within-budget) terminal
    let big = attempt_budget(&SeedSetting::target(2), &item.case, &p2, true, true).max_rows;
    let mut solver2 = WorkLimited {
        need: big,
        calls: vec![],
    };
    let rep2 = drive_tree(
        vec![item],
        &p2,
        &cmp,
        &mut solver2,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let t2 = rep2.terminal_candidates();
    assert_eq!(t2.len(), 1);
    assert!(!t2[0].1.within_budget());
    // a WALL-CLOCK stop stays a non-verdict outcome (never a terminal)
    struct Wall;
    impl CaseSolver for Wall {
        fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            ans(SolveOutcome::BudgetStop { time_decided: true }, 1)
        }
    }
    let item3 = root_item(&p, Face::new(vec![], vec![]), vec![point]);
    let rep3 = drive_tree(
        vec![item3],
        &p,
        &cmp,
        &mut Wall,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    assert!(rep3.terminal_candidates().is_empty());
    assert!(matches!(rep3.items[0].disposition, Some(Disposition::Attention { .. })));
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r3 / brief item 10: the containment proof is unforgeable: no public constructor takes a verdict.

#[test]
fn r3_containment_proof_has_no_constructor_from_a_verdict() {
    let root = env!("CARGO_MANIFEST_DIR");
    let assemble = std::fs::read_to_string(format!("{root}/src/assemble/mod.rs")).unwrap();
    assert!(!assemble.contains("pub fn containment_proof"), "the verdict-taking constructor is gone");
    assert!(!assemble.contains("ContainmentProof::"), "assemble mints no proof");
    let economy = std::fs::read_to_string(format!("{root}/src/economy.rs")).unwrap();
    // the only non-test construction site is the checking function
    let lit = economy.matches("Ok(ContainmentProof {").count();
    assert_eq!(lit, 1);
    assert!(economy.contains("#[cfg(test)]\n    pub(crate) fn for_tests("));
    assert!(!economy.contains("pub(crate) fn checked("));
    // and the checking function refuses a table set with a gap (a domain that covers nothing of the chart)
    let f = rustred_campaign_core::key::NodeFrame::without_header_lines_first(2, 2);
    let mut gap = rustred_campaign_core::partition::DomainList::new(2);
    let mut d = CaseDomain::free(2, Some(vec![true, true]));
    d.fixed[0] = Some(7);
    gap.push(rustred_campaign_core::cover::RuleRef::bare(0, 0, 0), d);
    assert!(
        rustred_campaign_core::economy::prove_coverage_preserved(
            &gap,
            &gap,
            &f,
            &Default::default(),
            &rustred_campaign_core::partition::DispatchOrderStatement::SameOrder
        )
        .is_err()
    );
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r4 / M4 (brief items 2, 9): recipe acceptance checks the node identity and the symmetry capability
// BEFORE any solve, and requires a determinism id.

struct RecipeOk {
    calls: u64,
    det: Option<String>,
}
impl CaseSolver for RecipeOk {
    fn solve(&mut self, _r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        ans(SolveOutcome::NoRelation, 0)
    }
    fn solve_recipe_at_key(&mut self, _r: &Recipe, k: &[i32]) -> Result<KeyRelation, RecipeFailure> {
        self.calls += 1;
        let mut rhs = k.to_vec();
        rhs[0] -= 1;
        Ok(KeyRelation {
            key: k.to_vec(),
            rhs_keys: vec![rhs],
            relation_digest: "d".into(),
            regular: Tri::Pass,
            proof_ok: true,
            determinism_id: self.det.clone(),
            cost_ms: 1,
            recipe_digest: Some("e".into()),
            binary_digest: None,
        })
    }
}

fn ended_for(p: &NodeParams) -> FullAttemptEnded {
    FullAttemptEnded::from_evidence(
        p,
        FullAttemptEvidence {
            node: p.node.clone(),
            cpu_ms: u64::MAX / 2,
            fixer_rounds: 99,
            open: vec!["x".into()],
        },
    )
    .unwrap()
}

#[test]
fn r4_recipe_acceptance_checks_node_identity_and_capability_before_any_solve() {
    let cmp = cmp6();
    let pa = NodeParams::new("A", 6, 3);
    let mut pb = NodeParams::new("B", 6, 3);
    pb.min_evidence = EvidenceLevel::A3;
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    let keys = [key(3, 2), key(2, 4)];
    // (a) a token of ANOTHER node: nothing solved, nothing certified
    let plain = Recipe::from_setting("r", &SeedSetting::target(2), &case, &pb, "legacy");
    let mut s = RecipeOk { calls: 0, det: Some("det".into()) };
    let rr = accept_recipe(&plain, &keys, &mut s, &cmp, &pb, &ended_for(&pa));
    assert_eq!(rr.certified_count(), 0);
    assert_eq!(s.calls, 0);
    assert!(rr.failed().iter().all(|(_, f)| matches!(f, RecipeFailure::Unsupported { .. })));
    // (b) symmetry rows without a certificate route: refused before any solve
    let mut sym = SeedSetting::target(2);
    sym.symmetry_rows = true;
    let recipe = Recipe::from_setting("r", &sym, &case, &pb, "legacy");
    assert!(!pb.capabilities.can(true, pb.min_evidence));
    let mut s = RecipeOk { calls: 0, det: Some("det".into()) };
    let rr = accept_recipe(&recipe, &keys, &mut s, &cmp, &pb, &ended_for(&pb));
    assert_eq!((rr.certified_count(), s.calls), (0, 0));
    // with the capability the same recipe certifies
    let mut pc = pb.clone();
    pc.capabilities = ProverCapabilities {
        plain: Some(EvidenceLevel::A4),
        symmetry_rows: Some(EvidenceLevel::A3),
    };
    let mut s = RecipeOk { calls: 0, det: Some("det".into()) };
    let rr = accept_recipe(&recipe, &keys, &mut s, &cmp, &pc, &ended_for(&pc));
    assert_eq!(rr.certified_count(), 2);
    // (c) answers WITHOUT a determinism id certify nothing (re-audit B m5)
    let mut s = RecipeOk { calls: 0, det: None };
    let rr = accept_recipe(&plain, &keys, &mut s, &cmp, &pb, &ended_for(&pb));
    assert_eq!(rr.certified_count(), 0);
    // (d) a recipe of another node (provenance) is refused
    let foreign = Recipe::from_setting("r", &SeedSetting::target(2), &case, &pa, "legacy");
    let mut s = RecipeOk { calls: 0, det: Some("det".into()) };
    let rr = accept_recipe(&foreign, &keys, &mut s, &cmp, &pb, &ended_for(&pb));
    assert_eq!((rr.certified_count(), s.calls), (0, 0));
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r5 / r6 (M5; brief item 4): the economy extra budget (U-13 factor) and the economy weights are READ.

struct FirstWins {
    p: NodeParams,
}
impl CaseSolver for FirstWins {
    fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let target = r.setting.label().starts_with("target");
        ans(
            if target {
                SolveOutcome::Rules(vec![rule(dom(&self.p, &r.case), "T", 50, false)])
            } else {
                SolveOutcome::NoRelation
            },
            if target { 1 } else { 1000 },
        )
    }
}

#[test]
fn r6_economy_extra_budget_is_enforced_by_work_counters() {
    let cmp = cmp6();
    let run = |p: &NodeParams| {
        let item = root_item(p, Face::new(vec![0, 1], vec![]), vec![key(2, 2), key(3, 4)]);
        drive_tree(
            vec![item],
            p,
            &cmp,
            &mut FirstWins { p: p.clone() },
            &mut Facts::default(),
            &mut ReplayProver,
            None,
            None,
            &TreeLimits::default(),
        )
    };
    let wave = seed_space(&CaseSpec::face_case(Face::new(vec![0, 1], vec![])), &p())[0].len();
    let mut zero = p();
    zero.budgets.economy_extra_factor = 0.0;
    let mut two = p();
    two.budgets.economy_extra_factor = 2.0;
    let mut all = p();
    all.budgets.economy_extra_factor = f64::INFINITY;
    let (r0, r2, ra) = (run(&zero), run(&two), run(&all));
    assert_eq!(r0.attempts.len(), 1, "factor 0: the first acceptable setting only");
    assert!(r2.attempts.len() > 1 && r2.attempts.len() < wave, "{} of {wave}", r2.attempts.len());
    assert_eq!(ra.attempts.len(), wave, "infinite factor: the whole wave");
    for r in [&r0, &r2, &ra] {
        assert!(matches!(r.items[0].disposition, Some(Disposition::Covered { .. })));
    }
    // the cap is decided by WORK COUNTERS, never by the reported CPU: the same run with other CPU figures is identical
    struct SlowFirst(NodeParams);
    impl CaseSolver for SlowFirst {
        fn solve(&mut self, r: &CaseRequest, b: &AttemptBudget) -> SolveAnswer {
            let mut a = FirstWins { p: self.0.clone() }.solve(r, b);
            a.cpu_ms = 1_000_000 - a.cpu_ms;
            a
        }
    }
    let item = root_item(&two, Face::new(vec![0, 1], vec![]), vec![key(2, 2), key(3, 4)]);
    let r2b = drive_tree(
        vec![item],
        &two,
        &cmp,
        &mut SlowFirst(two.clone()),
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    assert_eq!(
        r2.attempts.iter().map(|a| &a.setting).collect::<Vec<_>>(),
        r2b.attempts.iter().map(|a| &a.setting).collect::<Vec<_>>()
    );
    // a PARALLEL driver that answers the WHOLE wave (in reverse order) decides the same as the sequential one
    let item = root_item(&two, Face::new(vec![0, 1], vec![]), vec![key(2, 2), key(3, 4)]);
    let mut st = rustred_campaign_core::worklist::ladder::ItemState::new(item);
    let mut fw = FirstWins { p: two.clone() };
    let mut facts = Facts::default();
    let disp = loop {
        match rustred_campaign_core::worklist::ladder::next_steps(&st, &two) {
            Next::NeedShadowing => st.record_shadowing(ShadowVerdict::NotShadowed),
            Next::NeedOffers => st.record_offers("none", vec![]),
            Next::Run(_) => {
                // speculative: every setting of the wave, newest first
                let all: Vec<SeedSetting> = seed_space(&st.item.case, &two)[0].clone();
                for s in all.iter().rev() {
                    let a = rustred_campaign_core::worklist::ladder::Attempt {
                        item: st.item.id.clone(),
                        key: rustred_campaign_core::worklist::ladder::attempt_key(&st.item.id, s),
                        case: st.item.case.clone(),
                        setting: s.clone(),
                        rung: rustred_campaign_core::worklist::ladder::Rung::SeedSearch,
                        wave: 0,
                        budget: attempt_budget(s, &st.item.case, &two, false, false),
                        seed_args: vec![],
                        attempt_no: 1,
                    };
                    if !st.records.contains_key(&a.key) {
                        let an = fw.solve(&CaseRequest::from_attempt("P", &a), &a.budget);
                        st.record_answer(&a, an);
                    }
                }
            }
            Next::NeedVerdicts(r) => {
                for q in r {
                    let c = st.records[&q.attempt_key].candidates()[q.index].clone();
                    let v = rustred_campaign_core::worklist::ladder::judge_points(&c, &q.keys, &mut facts, &cmp);
                    st.record_verdicts(&q.attempt_key, q.index, v);
                }
            }
            Next::NeedEvidence(r) => {
                for q in r {
                    let ev = q
                        .keys
                        .iter()
                        .map(|k| rustred_campaign_core::worklist::ladder::KeyEvidence {
                            key: k.clone(),
                            level: Some(q.level),
                            failure: None,
                        })
                        .collect();
                    st.record_evidence(&q.attempt_key, q.index, ev);
                }
            }
            Next::NeedContainment(_) => unreachable!(),
            Next::Done(d) => break d,
        }
    };
    assert_eq!(
        format!("{:?}", disp),
        format!("{:?}", r2.items[0].disposition.clone().unwrap())
    );
}

#[test]
fn r5_economy_weights_decide_among_one_case_covers() {
    // two one-case candidates in the same answer: A small in RHS terms but huge in monomials, B the other way
    struct Two(NodeParams);
    impl CaseSolver for Two {
        fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            if !r.setting.label().starts_with("target") {
                return ans(SolveOutcome::NoRelation, 1);
            }
            let mut a = rule(dom(&self.0, &r.case), "A", 100, false);
            a.monomials = Some(1_000_000);
            let mut b = rule(dom(&self.0, &r.case), "B", 200, false);
            b.monomials = Some(10);
            ans(SolveOutcome::Rules(vec![a, b]), 1)
        }
    }
    let cmp = cmp6();
    let pick = |p: &NodeParams| {
        let item = root_item(p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
        let rep = drive_tree(
            vec![item],
            p,
            &cmp,
            &mut Two(p.clone()),
            &mut Facts::default(),
            &mut ReplayProver,
            None,
            None,
            &TreeLimits::default(),
        );
        match &rep.items[0].disposition {
            Some(Disposition::Covered { cover }) => cover.members[0].digest.clone(),
            other => panic!("{other:?}"),
        }
    };
    let mut terms_only = p();
    terms_only.economy = EconomyWeights {
        case: 2000.0,
        rhs_term: 1.0,
        monomial: 0.0,
        spread: 0.0,
    };
    assert_eq!(pick(&terms_only), "A", "RHS terms only: the smaller rule");
    let mut mono = p();
    mono.economy = EconomyWeights {
        case: 2000.0,
        rhs_term: 1.0,
        monomial: 1.0,
        spread: 0.0,
    };
    assert_eq!(pick(&mono), "B", "monomials weigh: the other rule");
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r7 (positive check kept): every permutation of the returned rules serves every asked key by a credited
// rule.

struct Layered {
    p: NodeParams,
}
impl CaseSolver for Layered {
    fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let tag = match r.case.dim() {
            2 => "R".to_string(),
            d => format!("D{d}:{}", r.case.canonical_text()),
        };
        ans(
            if r.setting.label().starts_with("target") {
                SolveOutcome::Rules(vec![rule(dom(&self.p, &r.case), &tag, 10, false)])
            } else {
                SolveOutcome::NoRelation
            },
            1,
        )
    }
}

#[test]
fn r7_every_permutation_serves_every_asked_key_by_a_credited_rule() {
    let p = p();
    let cmp = cmp6();
    let k = key(2, 2);
    let extra = vec![key(3, 2), key(3, 3), key(2, 3), key(4, 2), key(2, 5)];
    let mut facts = Facts {
        bad: vec![("R".into(), key(2, 2)), ("R".into(), key(3, 2))],
        calls: 0,
    };
    let c3 = CaseSpec::face_case(Face::new(vec![0, 1], vec![]))
        .constrained(&[(0, 3)], &[])
        .unwrap();
    facts.bad.push((format!("D1:{}", c3.canonical_text()), key(3, 2)));
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: k.clone(),
        extra_targets: extra.clone(),
        tables: &NoTables,
    };
    let out = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut Layered { p: p.clone() },
        &mut facts,
        &mut ReplayProver,
        None,
    )
    .expect("repair succeeds");
    assert!(out.rules.len() >= 3, "{:?}", out.rules);
    let cands: Vec<CandidateRule> = out
        .rules
        .iter()
        .map(|r| rule(dom(&p, &r.case), &r.digest, r.rhs_terms, false))
        .collect();
    let mut asked = vec![k.clone()];
    asked.extend(extra);
    assert_eq!(out.usable_at.iter().collect::<BTreeSet<_>>(), asked.iter().collect::<BTreeSet<_>>());
    fn perms(n: usize) -> Vec<Vec<usize>> {
        if n == 0 {
            return vec![vec![]];
        }
        let mut out = Vec::new();
        for p in perms(n - 1) {
            for i in 0..=p.len() {
                let mut q = p.clone();
                q.insert(i, n - 1);
                out.push(q);
            }
        }
        out
    }
    for perm in perms(out.rules.len()) {
        let list: Vec<(&CandidateRule, &[Exclusion])> = perm
            .iter()
            .map(|&i| (&cands[i], out.rules[i].exclusions.as_slice()))
            .collect();
        for a in &asked {
            match first_match_with_exclusions(&list, a) {
                FirstMatch::Rule(j) => assert!(out.rules[perm[j]].credited.contains(a)),
                other => panic!("{perm:?} at {a:?}: {other:?}"),
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r8 / m1: rules dispatched together are sealed in ONE call (the documented contract): two trees sealed
// together are order-free at every gap point of both.

#[test]
fn r8_rules_dispatched_together_are_sealed_together() {
    let p = p();
    let cmp = cmp6();
    let t1 = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let k2 = vec![1, 2, 1, 0, 0, 0];
    let t2 = root_item(&p, Face::new(vec![1], vec![]), vec![k2.clone()]);
    let mut all = Vec::new();
    for t in [t1, t2] {
        all.extend(
            drive_tree(
                vec![t],
                &p,
                &cmp,
                &mut Layered { p: p.clone() },
                &mut Facts::default(),
                &mut ReplayProver,
                None,
                None,
                &TreeLimits::default(),
            )
            .items,
        );
    }
    seal_tree(&mut all, &[], &p);
    let mut rules = Vec::new();
    for it in &all {
        if let Some(Disposition::Covered { cover }) = &it.disposition {
            for m in &cover.members {
                rules.push((it.state.records[&m.attempt_key].candidates()[m.index].clone(), m.clone()));
            }
        }
    }
    for order in [vec![0usize, 1], vec![1, 0]] {
        let list: Vec<(&CandidateRule, &[Exclusion])> = order
            .iter()
            .map(|&i| (&rules[i].0, rules[i].1.exclusions.as_slice()))
            .collect();
        for k in [key(2, 2), k2.clone()] {
            match first_match_with_exclusions(&list, &k) {
                FirstMatch::Rule(j) => assert!(rules[order[j]].1.credited.contains(&k)),
                other => panic!("{other:?}"),
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r9 / m2: a declared terminal WITHOUT a token (not preferred, not explicit) is flagged.

#[test]
fn r9_terminal_without_token_is_flagged() {
    let p = p();
    let flags = check_terminals(
        &[(key(7, 7), None)],
        100,
        &NoTables,
        &PreferredTerminals::default(),
        &TerminalContext {
            lines: &p.lines,
            honoured: &BTreeSet::new(),
            explicit: &BTreeSet::new(),
        },
    );
    assert_eq!(flags, vec![TerminalFlag::MissingToken { key: key(7, 7) }]);
    // an explicit spec key needs no token
    let flags = check_terminals(
        &[(key(7, 7), None)],
        100,
        &NoTables,
        &PreferredTerminals::default(),
        &TerminalContext {
            lines: &p.lines,
            honoured: &BTreeSet::new(),
            explicit: &[key(7, 7)].into_iter().collect(),
        },
    );
    assert!(flags.is_empty());
    // a FallbackSet read back never re-admits a key "added by rule" without its token
    let rec = rustred_campaign_core::terminals::FallbackSetRecord {
        bound: 10,
        keys: vec![key(7, 7)],
        explicit: vec![],
    };
    assert!(rec.verify(&[]).is_err());
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A r10 / m3: preferred keys beyond `max_preferred` are NOT honoured by seal_tree or repair_key.

#[test]
fn r10_preferred_bound_applies_in_seal_tree_and_repair_key() {
    let mut p = p();
    p.preferred = PreferredTerminals {
        entries: vec![PreferredEntry::Key(key(5, 5)), PreferredEntry::Key(key(6, 6))],
        count_against_bound: false,
        max_preferred: Some(0),
    };
    let cmp = cmp6();
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(6, 6),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let r = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut Layered { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    );
    assert!(!matches!(r, Err(RepairFailure::PreferredTerminal { .. })), "{r:?}");
    let inp2 = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(2, 2),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let out = repair_key(
        &inp2,
        &RepairBudget::default(),
        &mut Layered { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    )
    .unwrap();
    assert!(
        !out.rules[0]
            .exclusions
            .iter()
            .any(|e| e.reason == ExclusionReason::PreferredTerminal),
        "{:?}",
        out.rules[0].exclusions
    );
    // within the bound the preference applies
    p.preferred.max_preferred = Some(1);
    let inp3 = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(5, 5),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let r = repair_key(
        &inp3,
        &RepairBudget::default(),
        &mut Layered { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
    );
    assert!(matches!(r, Err(RepairFailure::PreferredTerminal { .. })), "{r:?}");
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A section 5 (c): the mutation "first_match_with_exclusions treats Blocked / Undecided as not selected" was
// caught by nothing. A pole at the key BLOCKS first match: a later rule never serves there.

#[test]
fn mutation_c_first_match_blocked_is_not_skipped() {
    let p = p();
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    let mut d = dom(&p, &case);
    d.poles.push(rustred_campaign_core::domain::PoleLocus {
        coeffs: vec![rustred_campaign_core::poly::Poly::parse_d_free("n0-2", 6).unwrap()],
        kind: rustred_campaign_core::domain::PoleKind::Witness,
    });
    let blocked = rule(d, "pole", 1, false);
    let later = rule(dom(&p, &case), "later", 1, false);
    let list: Vec<(&CandidateRule, &[Exclusion])> = vec![(&blocked, &[]), (&later, &[])];
    assert_eq!(first_match_with_exclusions(&list, &key(2, 3)), FirstMatch::Blocked(0));
    assert_eq!(first_match_with_exclusions(&list, &key(3, 3)), FirstMatch::Rule(0));
    // with an exclusion at the key the later rule serves
    let ex = vec![Exclusion {
        target: ExclusionTarget::Key(key(2, 3)),
        reason: ExclusionReason::OnPole,
    }];
    let list: Vec<(&CandidateRule, &[Exclusion])> = vec![(&blocked, &ex), (&later, &[])];
    assert_eq!(first_match_with_exclusions(&list, &key(2, 3)), FirstMatch::Rule(1));
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit B C1 (brief item 1, small form; the model-scale reproduction is in tests/ladder_audit_b_model.rs): a run
// STOPPED by a limit seals its rules at the gap points of the roots it never processed, and lists them for a retry.

#[test]
fn reaudit_b_c1_stopped_run_excludes_rules_at_unprocessed_roots() {
    let p = p();
    let cmp = cmp6();
    let r1 = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let k2 = vec![1, 2, 1, 0, 0, 0];
    let r2 = root_item(&p, Face::new(vec![1], vec![]), vec![k2.clone()]);
    let rep = drive_tree(
        vec![r1, r2.clone()],
        &p,
        &cmp,
        &mut Layered { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits {
            max_items: 1,
            ..TreeLimits::default()
        },
    );
    assert!(rep.stopped.is_some());
    assert_eq!(rep.unprocessed.len(), 1);
    assert_eq!(rep.unprocessed[0].id, r2.id);
    let (i, m) = rep.accepted_members().remove(0);
    let c = rep.rule_of(i, &m).unwrap();
    assert_ne!(
        c.domain.applies(&k2).stage(),
        rustred_campaign_core::domain::Stage::NotSelected,
        "the rule's written domain contains the unprocessed gap point"
    );
    assert!(m.exclusions.iter().any(|e| e.target == ExclusionTarget::Key(k2.clone())
        && matches!(e.reason, ExclusionReason::NotProcessed { .. })));
    let list: Vec<(&CandidateRule, &[Exclusion])> = vec![(c, m.exclusions.as_slice())];
    assert_eq!(first_match_with_exclusions(&list, &k2), FirstMatch::None);
    // the winner-diff stage also knows the run stopped
    assert!(rep.pending_winner_diff().run(&[], &NoTables, &mut PassAll).from_stopped_run());
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit B M1 (brief item 2): at min_evidence A1 a rule WITH symmetry rows still needs a stated capability; the
// symmetry-first seed space is used only when capable.

#[test]
fn reaudit_b_m1_capability_gate_holds_at_a1() {
    let mut p = p();
    p.min_evidence = EvidenceLevel::A1;
    p.symmetry = Some(swap01());
    assert!(!p.capabilities.can(true, EvidenceLevel::A1));
    assert!(!p.capabilities.can(true, EvidenceLevel::A0));
    assert!(p.capabilities.can(false, EvidenceLevel::A1));
    let face = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    assert!(seed_space(&face, &p).iter().flatten().all(|s| !s.symmetry_rows), "not capable: plain space");
    struct SymOnly(NodeParams);
    impl CaseSolver for SymOnly {
        fn solve(&mut self, r: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
            ans(SolveOutcome::Rules(vec![rule(dom(&self.0, &r.case), "s", 3, true)]), 1)
        }
    }
    let cmp = cmp6();
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let mut facts = Facts::default();
    let rep = drive_tree(
        vec![item],
        &p,
        &cmp,
        &mut SymOnly(p.clone()),
        &mut facts,
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    assert!(rep.accepted_members().is_empty(), "never accepted at A1 without the capability");
    assert_eq!(facts.calls, 0, "never judged");
    // capable: the symmetry-first space and the rule is accepted
    p.capabilities.symmetry_rows = Some(EvidenceLevel::A2);
    assert!(seed_space(&face, &p)[0][0].symmetry_rows);
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit B m1: check_terminals takes the HONOURED set fixed at strip time (relative to the gap keys), never
// relative to the terminal list.

#[test]
fn reaudit_b_m1_check_terminals_honoured_reference_set() {
    let lines = vec![true, true, false];
    let mut p = NodeParams::with_lines("toy", lines.clone());
    p.terminals_max = 1;
    let a: Key = vec![2, 1, 0];
    let b: Key = vec![3, 1, 0];
    p.preferred.entries = vec![PreferredEntry::Key(a.clone()), PreferredEntry::Key(b.clone())];
    let at_strip = p.preferred.honoured(&[a.clone(), b.clone()], &lines, p.terminals_max);
    assert!(at_strip.contains(&a) && !at_strip.contains(&b));
    struct CoverB(Key);
    impl rustred_campaign_core::repair::Coverage for CoverB {
        fn verdict(&self, key: &[i32]) -> rustred_campaign_core::repair::CoverageVerdict {
            if key == self.0.as_slice() {
                rustred_campaign_core::repair::CoverageVerdict::Covered { rule: "tbl#1".into() }
            } else {
                rustred_campaign_core::repair::CoverageVerdict::Uncovered
            }
        }
    }
    let flags = check_terminals(
        &[(b.clone(), None)],
        p.terminals_max,
        &CoverB(b.clone()),
        &p.preferred,
        &TerminalContext {
            lines: &lines,
            honoured: &at_strip,
            explicit: &BTreeSet::new(),
        },
    );
    assert!(flags.iter().any(|f| matches!(f, TerminalFlag::CoveredTerminal { .. })), "{flags:?}");
}

// ------------------------------------------------------------------------------------------------------------------
// Brief item 8 / re-audit B m3: the winner-diff stage is MANDATORY: the only exportable form is `WinnerChecked`; a
// rule winning at a key never asked about is excluded there when the per-key check does not pass.

struct PassAll;
impl WinnerCheck for PassAll {
    fn check(&mut self, _r: &CandidateRule, _k: &[i32]) -> Tri {
        Tri::Pass
    }
}
struct FailAt(Key);
impl WinnerCheck for FailAt {
    fn check(&mut self, _r: &CandidateRule, k: &[i32]) -> Tri {
        if k == self.0.as_slice() { Tri::Fail } else { Tri::Unknown }
    }
}

#[test]
fn item8_winner_diff_stage_is_mandatory_and_excludes_failed_winners() {
    let p = p();
    let cmp = cmp6();
    let item = root_item(&p, Face::new(vec![0, 1], vec![]), vec![key(2, 2)]);
    let rep = drive_tree(
        vec![item],
        &p,
        &cmp,
        &mut Layered { p: p.clone() },
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    assert!(rep.dispatch.winner_diff_required);
    let range = vec![key(2, 2), key(3, 3), key(4, 2)];
    let checked = rep.pending_winner_diff().run(&range, &NoTables, &mut FailAt(key(3, 3)));
    let c = checked.counts();
    assert_eq!((c.keys, c.credited), (3, 1));
    assert_eq!(c.excluded, 2, "{c:?}: (3,3) failed, (4,2) unknown");
    let list = checked.dispatch_list();
    assert!(matches!(first_match_with_exclusions(&list, &key(2, 2)), FirstMatch::Rule(0)));
    assert_eq!(first_match_with_exclusions(&list, &key(3, 3)), FirstMatch::None);
    assert!(checked.rules()[0].exclusions().iter().any(|e| matches!(e.reason, ExclusionReason::WinnerCheckFailed { .. })));
    assert_eq!(checked.range(), &range[..]);
    let ok = rep.pending_winner_diff().run(&range, &NoTables, &mut PassAll);
    assert_eq!((ok.counts().checked, ok.counts().excluded), (2, 0));
    // no public constructor of the checked form: fields private, built only by PendingWinnerDiff::run
    let src = std::fs::read_to_string(format!("{}/src/repair.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert_eq!(src.matches("WinnerChecked {\n            rules,").count(), 1);
    assert!(src.contains("pub struct WinnerChecked {\n    rules: Vec<NewRule>,"));
    assert!(src.contains("pub struct PendingWinnerDiff {\n    rules: Vec<NewRule>,"));
}

// ------------------------------------------------------------------------------------------------------------------
// Addition (b) (R31 evidence 04:37Z): the SHADOWING test runs on a new case BEFORE any search; a shadowed or empty
// case is dropped with a typed reason.

#[test]
fn shadowing_test_before_search_drops_shadowed_and_empty_cases() {
    let p = p();
    let cmp = cmp6();
    struct Stub(BTreeMap<String, ShadowVerdict>);
    impl ShadowCheck for Stub {
        fn check(&mut self, item: &Item, _e: &[(&CandidateRule, &[Exclusion])]) -> ShadowVerdict {
            self.0
                .get(&item.case.canonical_text())
                .cloned()
                .unwrap_or(ShadowVerdict::NotShadowed)
        }
    }
    struct Count(u64, NodeParams);
    impl CaseSolver for Count {
        fn solve(&mut self, r: &CaseRequest, b: &AttemptBudget) -> SolveAnswer {
            self.0 += 1;
            Layered { p: self.1.clone() }.solve(r, b)
        }
    }
    let shadowed = root_item(&p, Face::new(vec![0], vec![]), vec![vec![2, 1, 1, 0, 0, 0]]);
    let normal = root_item(&p, Face::new(vec![1], vec![]), vec![vec![1, 2, 1, 0, 0, 0]]);
    let mut stub = Stub(
        [(
            shadowed.case.canonical_text(),
            ShadowVerdict::Shadowed {
                certificate: "test".into(),
            },
        )]
        .into_iter()
        .collect(),
    );
    let mut solver = Count(0, p.clone());
    let rep = drive_tree_with(
        vec![shadowed.clone(), normal],
        &p,
        &cmp,
        &mut solver,
        &mut Facts::default(),
        &mut ReplayProver,
        None,
        None,
        Some(&mut stub),
        &TreeLimits::default(),
    );
    assert!(matches!(
        &rep.items[0].disposition,
        Some(Disposition::Dropped { reason: DropReason::Shadowed { .. }, keys }) if keys == &shadowed.gap_points
    ));
    assert!(matches!(rep.items[1].disposition, Some(Disposition::Covered { .. })));
    assert!(rep.attempts.iter().all(|a| a.item != shadowed.id), "no search for the dropped case");
    // the partition-module implementation: a table set covering the whole chart shadows every case
    let frame = rustred_campaign_core::key::NodeFrame::without_header_lines_first(6, 3);
    let mut tables = rustred_campaign_core::partition::DomainList::new(6);
    tables.push(
        rustred_campaign_core::cover::RuleRef::bare(0, 0, 0),
        CaseDomain::free(6, Some(p.lines.clone())),
    );
    let mut real = PartitionShadowCheck {
        tables: &tables,
        frame: &frame,
        budget: Default::default(),
    };
    assert!(matches!(real.check(&shadowed, &[]), ShadowVerdict::Shadowed { .. }));
    let none = rustred_campaign_core::partition::DomainList::new(6);
    let mut real0 = PartitionShadowCheck {
        tables: &none,
        frame: &frame,
        budget: Default::default(),
    };
    assert_eq!(real0.check(&shadowed, &[]), ShadowVerdict::NotShadowed);
    // an empty case domain (a free line slot forced below the chart by an equation): EmptyDomain
    let mut empty = shadowed.clone();
    empty.case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]))
        .constrained(
            &[],
            &[rustred_campaign_core::worklist::affine::AffineEq::new(vec![(0, 1), (1, 1)], 1).unwrap()],
        )
        .unwrap();
    assert!(matches!(real0.check(&empty, &[]), ShadowVerdict::EmptyDomain { .. }), "{:?}", real0.check(&empty, &[]));
}

// ------------------------------------------------------------------------------------------------------------------
// Addition (a) / brief item 9: the recipe IS the engine's file `rustred.recipe.v1`: field names, explicit floor slots,
// unknown fields refused, the engine's example parses (with a ball generator).

#[test]
fn recipe_is_the_engine_file_format() {
    let p = p();
    let case = CaseSpec::face_case(Face::new(vec![0, 1], vec![]))
        .constrained(&[(0, 2)], &[])
        .unwrap();
    let s = SeedSetting {
        offsets: vec![BTreeMap::from([(1usize, -1i32)])],
        floor: true,
        ..SeedSetting::target(2)
    };
    let r = Recipe::from_setting("toy/x", &s, &case, &p, "legacy");
    let v = serde_json::to_value(&r).unwrap();
    for f in ["schema", "id", "arity", "chart", "domain", "seeds", "symmetry_rows", "elimination", "provenance"] {
        assert!(v.get(f).is_some(), "{f}");
    }
    assert_eq!(v["schema"], "rustred.recipe.v1");
    assert_eq!(v["chart"], "111000");
    assert_eq!(v["seeds"]["unit"], "harder");
    assert_eq!(v["seeds"]["generator"]["ball"]["radius"], 2);
    assert_eq!(v["seeds"]["generator"]["ball"]["down"], serde_json::json!([1]));
    // the floor NAMES its slots: the free dotted line 1 and the line 0 fixed at 2 (R48 addendum 2)
    assert_eq!(v["seeds"]["min_offset"], serde_json::json!({"0": -1, "1": -1}));
    assert_eq!(v["seeds"]["sector_slots"], serde_json::json!([0, 2, 3, 4, 5]));
    assert_eq!(v["elimination"]["convention"], "rustred.recipe-elim.v1");
    assert_eq!(v["provenance"]["node"], "P");
    let back = Recipe::parse(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
    let mut bad = v.clone();
    bad["group_digest"] = serde_json::json!("x");
    assert!(Recipe::parse(&bad.to_string()).is_err(), "unknown fields are refused");
    let engine = r#"{ "schema": "rustred.recipe.v1", "id": "T/face-12/x6", "arity": 6, "chart": "111000",
      "domain": { "fixed": [null, 1, 1, 0, 0, null], "equations": ["n0+n5-1"], "exceptions": [["n0-4"]],
                  "bounds": {"min": [null, null, null, null, null, -3], "max": [null, null, null, null, null, null]} },
      "seeds": { "unit": "harder", "centres": [[-1, 0, 0, 0, 0, 0]], "generator": {"shells": ["S0", "S1", "Sm1"]},
                 "min_offset": {"0": -1}, "max_offset": {}, "layer_cap": null, "sector_filter": true,
                 "sector_slots": [1, 2, 3, 4] },
      "symmetry_rows": false, "elimination": { "convention": "rustred.recipe-elim.v1" } }"#;
    let e = Recipe::parse(engine).unwrap();
    assert!(e.in_domain(&[2, 1, 1, 0, 0, -1]));
    assert!(!e.in_domain(&[3, 1, 1, 0, 0, 0]), "n0 + n5 - 1 = 2");
    assert!(!e.in_domain(&[5, 1, 1, 0, 0, -4]), "below the bound");
    assert!(!e.in_domain(&[4, 1, 1, 0, 0, -3]), "on the exception branch n0 = 4");
    assert!(!e.in_domain(&[2, 2, 1, 0, 0, -1]), "a fixed slot differs");
}

// ------------------------------------------------------------------------------------------------------------------
// Addition (c) (R49, R48 addenda 3 (a) and 4): the default recipe sequence and the recipe economy measure.

#[test]
fn default_recipe_and_recipe_economy_measure() {
    let mut p = p();
    let k = vec![2, 3, 1, -1, 0, 0];
    let rs = default_recipes(&k, &p, "legacy");
    assert_eq!(rs.len(), 2, "no capable group: line centre + union");
    assert_eq!(rs[0].seeds.centres, vec![vec![0, -1, 0, 0, 0, 0]], "the key's LAST dotted line");
    assert_eq!(rs[0].seeds.min_offset, BTreeMap::from([(0, -1), (1, -1)]));
    assert_eq!(rs[1].seeds.centres.len(), 3, "one centre per off-floor slot");
    for r in &rs {
        assert!(r.in_domain(&k) && r.in_domain(&[4, 2, 1, -3, 0, 0]));
        assert!(r.validate().is_ok());
    }
    p.symmetry = Some(swap01());
    p.capabilities.symmetry_rows = Some(EvidenceLevel::A4);
    let rs = default_recipes(&k, &p, "legacy");
    assert_eq!(rs.len(), 3);
    assert!(rs[0].symmetry_rows, "symmetry rows at the key first when certifiable");
    // closure size and used rules weigh ABOVE the number of recipes
    let per_face = RecipeEconomy {
        closure_keys: 471,
        used_rules: 300,
        recipes: 17,
        solve_cost_ms: 292_000,
    };
    let one_default = RecipeEconomy {
        closure_keys: 3720,
        used_rules: 900,
        recipes: 2,
        solve_cost_ms: 500_000,
    };
    assert!(per_face < one_default);
}

// ------------------------------------------------------------------------------------------------------------------
// re-audit A m8 / M7: the symmetry-rows seed dimension is spec DATA (`SeedPolicy::symmetry_dimension`).

#[test]
fn m8_symmetry_dimension_is_policy_data() {
    let mut p = p();
    p.symmetry = Some(swap01());
    p.capabilities.symmetry_rows = Some(EvidenceLevel::A4);
    let point = CaseSpec::face_case(Face::new(vec![], vec![]));
    let face = CaseSpec::face_case(Face::new(vec![0, 1], vec![]));
    assert!(seed_space(&face, &p).iter().flatten().any(|s| s.symmetry_rows));
    p.policy.symmetry_dimension = false;
    for c in [&point, &face] {
        assert!(seed_space(c, &p).iter().flatten().all(|s| !s.symmetry_rows));
    }
}
