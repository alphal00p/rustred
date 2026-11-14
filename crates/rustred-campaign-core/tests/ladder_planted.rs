//! Planted tests of the ladder, terminals, economy, explain and repair_key (brief item 8; R29 (b), R31 (b), R33, R35,
//! U-9, U-13).

mod ladder_common;

use std::collections::BTreeMap;

use ladder_common::*;
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::economy::{
    CoverageState, RetireError, Retirement, RuleMeasure, SupersedeReason,
};
use rustred_campaign_core::explain::explain;
use rustred_campaign_core::poly::Poly;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, ContainmentCheck, Coverage, CoverageVerdict, NoTables, RepairBudget,
    RepairFailure, RepairInputs, TreeLimits, drive_tree, repair_key,
};
use rustred_campaign_core::terminals::{
    FallbackSet, TerminalFlag, check_terminals, compare_with_reference,
};
use rustred_campaign_core::worklist::affine::AffineEq;
use rustred_campaign_core::worklist::items::{
    CaseSpec, Face, Item, MasterCounts, NodeParams, OldRule, UserBox, WorklistError,
    box_restricted_item,
};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, ItemState, SolveAnswer, SolveOutcome, Tri,
};
use rustred_campaign_core::worklist::seed::AttemptBudget;

/// Answers by setting label (any case); unlisted settings: no relation.
struct LabelSolver {
    params: NodeParams,
    /// label -> list of exception points (each a key) of the returned rule, with rhs terms
    answers: BTreeMap<String, (Vec<Vec<i32>>, u64)>,
    calls: Vec<String>,
}

impl LabelSolver {
    fn rule(&self, case: &CaseSpec, except: &[Vec<i32>], rhs: u64, tag: &str) -> CandidateRule {
        let p = &self.params;
        let mut d = CaseDomain::free(p.arity, Some(p.lines.clone()));
        let free = case.free();
        for i in 0..p.arity {
            if !free.contains(&i) {
                d.fixed[i] =
                    Some(
                        case.fixed
                            .get(&i)
                            .copied()
                            .unwrap_or(if p.lines[i] { 1 } else { 0 }),
                    );
            }
        }
        for k in except {
            d.exceptions.push(
                free.iter()
                    .map(|&s| {
                        Poly::parse_d_free(&AffineEq::fixed(s, k[s]).to_engine(), p.arity).unwrap()
                    })
                    .collect(),
            );
        }
        CandidateRule {
            domain: d,
            digest: tag.to_string(),
            rhs_terms: rhs,
            monomials: None,
            max_degree: None,
            bytes: None,
            rows: None,
            tie_break_only: None,
            symmetry_rows: false, witness: rustred_campaign_core::cover::WitnessStatus::Complete,
        }
    }
}

impl CaseSolver for LabelSolver {
    fn solve(&mut self, req: &CaseRequest, _b: &AttemptBudget) -> SolveAnswer {
        let l = req.setting.label();
        self.calls
            .push(format!("{} {}", req.case.canonical_text(), l));
        let outcome = match self.answers.get(&l) {
            Some((exc, rhs)) if req.case.dim() > 0 => {
                SolveOutcome::Rules(vec![self.rule(&req.case, exc, *rhs, &l)])
            }
            _ => SolveOutcome::NoRelation,
        };
        SolveAnswer {
            outcome,
            cpu_ms: 10,
            rows: None,
        }
    }
}

fn p2() -> NodeParams {
    NodeParams::new("P", 6, 3)
}

fn key(a: i32, b: i32) -> Vec<i32> {
    vec![a, b, 1, 0, 0, 0]
}

fn grid() -> Vec<Vec<i32>> {
    let mut v = Vec::new();
    for a in 2..5 {
        for b in 2..5 {
            v.push(key(a, b));
        }
    }
    v
}

fn run(
    solver: &mut LabelSolver,
    p: &NodeParams,
) -> (rustred_campaign_core::repair::TreeReport, Outcome) {
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 3,
        line_priority: (0..3).collect(),
    };
    let root = root_item(p, Face::new(vec![0, 1], vec![]), grid());
    let rep = drive_tree(
        vec![root],
        p,
        &cmp,
        solver,
        &mut facts,
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    );
    let o = outcome(&rep);
    (rep, o)
}

#[test]
fn two_case_cover_is_never_kept_when_a_one_case_cover_exists() {
    let p = p2();
    let half_a: Vec<Vec<i32>> = grid().into_iter().filter(|k| k[0] == 2).collect();
    let half_b: Vec<Vec<i32>> = grid().into_iter().filter(|k| k[0] != 2).collect();
    // depth 2: two complementary centres (a two-case cover); depth 3: ONE centre covering everything (dearer)
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([
            ("line:0/fl/d2".to_string(), (half_a, 100)),
            ("line:1/fl/d2".to_string(), (half_b, 100)),
            ("line:0/fl/d3".to_string(), (vec![], 5000)),
        ]),
        calls: vec![],
    };
    let (_, o) = run(&mut s, &p);
    assert_eq!(o.cases, 1, "{:?}", o.rules);
    assert!(o.rules[0].1.ends_with("/d3"));
    // and when both a one-case and a two-case cover are in the SAME wave, the one-case cover wins
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([
            (
                "line:0/fl/d2".to_string(),
                (grid().into_iter().filter(|k| k[0] == 2).collect(), 10),
            ),
            (
                "line:1/fl/d2".to_string(),
                (grid().into_iter().filter(|k| k[0] != 2).collect(), 10),
            ),
            ("union_lines(0,1)/unfl/d2".to_string(), (vec![], 9000)),
        ]),
        calls: vec![],
    };
    let (_, o) = run(&mut s, &p);
    assert_eq!(o.cases, 1, "{:?}", o.rules);
    assert!(o.rules[0].1.starts_with("union_lines"));
}

#[test]
fn a_candidate_covering_none_of_its_gap_points_is_rejected() {
    let p = p2();
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([("target/unfl/d2".to_string(), (grid(), 1))]),
        calls: vec![],
    };
    let (rep, o) = run(&mut s, &p);
    assert_eq!(o.cases, 0);
    let st = &rep.items[0].state;
    match rep.items[0].disposition.as_ref().unwrap() {
        Disposition::Split {
            cover, children, ..
        } => {
            assert!(cover.members.is_empty());
            // its exception branches (all points) still inform the children
            assert!(!children.is_empty());
        }
        d => panic!("{d:?}"),
    }
    let e = explain(st, &p);
    assert!(e.to_text().contains("covers 0/9"), "{}", e.to_text());
    assert_eq!(e.state, "split");
}

#[test]
fn hidden_pole_rejects_the_candidate() {
    use rustred_campaign_core::repair::FactSource;
    use rustred_campaign_core::worklist::ladder::{PointFacts, PointVerdict, judge_point};
    struct Pole;
    impl FactSource for Pole {
        fn facts(&mut self, _r: &CandidateRule, k: &[i32]) -> PointFacts {
            PointFacts {
                regular: if k[0] == 3 { Tri::Fail } else { Tri::Pass },
                rhs_top: Some(vec![]),
                exceptions_complete: Tri::Pass,
                note: None,
            }
        }
    }
    let p = p2();
    let s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::new(),
        calls: vec![],
    };
    let r = s.rule(
        &CaseSpec::face_case(Face::new(vec![0, 1], vec![])),
        &[],
        1,
        "x",
    );
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut f = Pole;
    let v = judge_point(&r, &key(3, 2), &f.facts(&r, &key(3, 2)), &cmp);
    assert_eq!(v, PointVerdict::HiddenPole);
    let v = judge_point(&r, &key(2, 2), &f.facts(&r, &key(2, 2)), &cmp);
    assert_eq!(v, PointVerdict::Covered);
    // an ascending RHS key is NotDescending
    let pf = PointFacts {
        regular: Tri::Pass,
        rhs_top: Some(vec![key(5, 5)]),
        exceptions_complete: Tri::Pass,
        note: None,
    };
    assert!(matches!(
        judge_point(&r, &key(2, 2), &pf, &cmp),
        PointVerdict::NotDescending { .. }
    ));
}

#[test]
fn box_restricted_item_needs_a_user_box_r35() {
    use rustred_campaign_core::worklist::items::{FullAttemptEnded, FullAttemptEvidence};
    let mut p = p2();
    let c = CaseSpec::face_case(Face::new(vec![0], vec![]));
    let ev = |cpu_ms: u64, rounds: u32, open: Vec<String>| FullAttemptEvidence {
        node: "P".into(),
        cpu_ms,
        fixer_rounds: rounds,
        open,
    };
    // R35 / U-6: the full attempt must have ended (budget used up, something still open)
    assert!(matches!(
        FullAttemptEnded::from_evidence(&p, ev(1000, 1, vec!["face".into()])),
        Err(WorklistError::FullAttemptNotEnded(_))
    ));
    assert!(matches!(
        FullAttemptEnded::from_evidence(&p, ev(u64::MAX, 9, vec![])),
        Err(WorklistError::FullAttemptNotEnded(_))
    ));
    let ended = FullAttemptEnded::from_evidence(&p, ev(10, 5, vec!["face".into()])).unwrap();
    assert_eq!(
        box_restricted_item(c.clone(), vec![key(2, 1)], &p, &ended).unwrap_err(),
        WorklistError::NoUserBox
    );
    p.user_box = Some(UserBox {
        id: "R*".into(),
        given_by: "tool".into(),
    });
    assert!(matches!(
        box_restricted_item(c.clone(), vec![], &p, &ended),
        Err(WorklistError::BoxNotUserGiven(_))
    ));
    p.user_box = Some(UserBox {
        id: "R*".into(),
        given_by: "user".into(),
    });
    let it = box_restricted_item(c, vec![key(2, 1)], &p, &ended).unwrap();
    assert_eq!(it.box_id.as_deref(), Some("R*"));
}

#[test]
fn terminal_bound_flagged_master_count_is_information_only_u9() {
    let lines = vec![true, true, true, false, false, false];
    let flags = check_terminals(
        &[(key(2, 1), None), (key(1, 2), None), (key(1, 1), None)],
        2,
        &NoTables,
        &Default::default(),
        &lines,
    );
    assert!(flags.contains(&TerminalFlag::OverBound { count: 3, bound: 2 }));
    // a reference master count different from the campaign's is a note, never a flag (U-9)
    let r = compare_with_reference(
        3,
        &MasterCounts {
            ibp: Some(7),
            with_symmetry: Some(3),
            source: "ref".into(),
        },
        "reference",
    );
    assert!(r.note.contains("information only"));
    let mut f = FallbackSet::new(1);
    assert!(f.is_empty());
    struct All;
    impl Coverage for All {
        fn verdict(&self, _k: &[i32]) -> CoverageVerdict {
            CoverageVerdict::Covered { rule: "r".into() }
        }
    }
    let mut f2 = FallbackSet::with_explicit(5, vec![key(1, 1)]).unwrap();
    assert_eq!(f2.remove_covered(&All).len(), 1);
    assert!(f2.is_empty());
    let _ = &mut f;
}

#[test]
fn full_node_retirement_needs_a_containment_proof_r33() {
    let m = |t: u64| RuleMeasure {
        cases: 1,
        rhs_terms: t,
        monomials: t,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 1,
    };
    let old = vec![("a".to_string(), m(100)), ("b".to_string(), m(100))];
    let e = Retirement::new(
        "N",
        CoverageState::Full,
        &old,
        ("c", &m(50)),
        SupersedeReason::FaceRetiresPoints,
        None,
    );
    assert_eq!(e.unwrap_err(), RetireError::FullNodeNeedsProof);
    // a ContainmentProof cannot be built outside the crate (audit A M5); the positive path is the unit test
    // economy::tests::full_node_retirement_needs_a_checked_proof_and_records_reverify
    let one = vec![("a".to_string(), m(100))];
    assert_eq!(
        Retirement::new(
            "N",
            CoverageState::Boxed,
            &one,
            ("c", &m(500)),
            SupersedeReason::CheaperSameOrLargerDomain,
            None
        )
        .unwrap_err(),
        RetireError::NotCheaper
    );
}

#[test]
fn optimise_item_replaces_only_with_containment_and_a_strictly_better_measure_u13() {
    let p = p2();
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([
            ("line:0/fl/d2".to_string(), (vec![], 50)),
            ("line:1/fl/d2".to_string(), (vec![], 40)),
        ]),
        calls: vec![],
    };
    struct Contain(&'static str);
    impl ContainmentCheck for Contain {
        fn contains(&mut self, r: &CandidateRule, _old: &[String]) -> Tri {
            if r.digest.starts_with(self.0) {
                Tri::Pass
            } else {
                Tri::Fail
            }
        }
    }
    let old = vec![OldRule {
        digest: "old".into(),
        rhs_terms: 60,
        monomials: None,
        max_degree: None,
    }];
    let it = Item::optimise(
        "P",
        CaseSpec::face_case(Face::new(vec![0, 1], vec![])),
        grid(),
        old.clone(),
    );
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 3,
        line_priority: (0..3).collect(),
    };
    let mut c = Contain("line:0");
    let rep = drive_tree(
        vec![it.clone()],
        &p,
        &cmp,
        &mut s,
        &mut facts,
        &mut ReplayProver,
        Some(&mut c),
        None,
        &TreeLimits::default(),
    );
    match rep.items[0].disposition.as_ref().unwrap() {
        Disposition::Covered { cover } => assert_eq!(cover.members[0].digest, "line:0/fl/d2"),
        d => panic!("{d:?}"),
    }
    // nothing contains the old domain: the old rule stays
    let mut c = Contain("none");
    let rep = drive_tree(
        vec![it],
        &p,
        &cmp,
        &mut s,
        &mut facts,
        &mut ReplayProver,
        Some(&mut c),
        None,
        &TreeLimits::default(),
    );
    assert!(matches!(
        rep.items[0].disposition,
        Some(Disposition::OptimiseKept { .. })
    ));
}

#[test]
fn repair_key_typed_outcomes() {
    let p = p2();
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 3,
        line_priority: (0..3).collect(),
    };
    // covered by the tables in force: nothing to do
    struct All;
    impl Coverage for All {
        fn verdict(&self, _k: &[i32]) -> CoverageVerdict {
            CoverageVerdict::Covered { rule: "t#1".into() }
        }
    }
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::new(),
        calls: vec![],
    };
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(3, 2),
        extra_targets: vec![],
        tables: &All,
    };
    let o = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut s,
        &mut facts,
        &mut ReplayProver,
        None,
    )
    .unwrap();
    assert!(o.already_covered && s.calls.is_empty());
    // uncovered: the face case of the key is solved
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([("line:1/fl/d2".to_string(), (vec![], 10))]),
        calls: vec![],
    };
    let inp = RepairInputs {
        params: &p,
        descent: &cmp,
        key: key(3, 2),
        extra_targets: vec![],
        tables: &NoTables,
    };
    let o = repair_key(
        &inp,
        &RepairBudget::default(),
        &mut s,
        &mut facts,
        &mut ReplayProver,
        None,
    )
    .unwrap();
    assert!(
        o.rules
            .iter()
            .any(|r| r.serves_key && r.setting == "line:1/fl/d2")
    );
    // nothing anywhere: a terminal proposal with an exhausted ladder (depth 3 unfloored tried)
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::new(),
        calls: vec![],
    };
    match repair_key(
        &inp,
        &RepairBudget::default(),
        &mut s,
        &mut facts,
        &mut ReplayProver,
        None,
    ) {
        Err(RepairFailure::Terminal { exhausted, .. }) => {
            assert!(exhausted.depth3_unfloored_tried())
        }
        other => panic!("{other:?}"),
    }
    // budget
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::new(),
        calls: vec![],
    };
    let b = RepairBudget {
        max_attempts: 2,
        ..RepairBudget::default()
    };
    assert!(matches!(
        repair_key(&inp, &b, &mut s, &mut facts, &mut ReplayProver, None),
        Err(RepairFailure::BudgetExhausted { .. })
    ));
    // a key outside the top-sector chart
    let bad = RepairInputs {
        params: &p,
        descent: &cmp,
        key: vec![0, 1, 1, 0, 0, 0],
        extra_targets: vec![],
        tables: &NoTables,
    };
    assert!(matches!(
        repair_key(
            &bad,
            &RepairBudget::default(),
            &mut s,
            &mut facts,
            &mut ReplayProver,
            None
        ),
        Err(RepairFailure::InvalidKey { .. })
    ));
    let _ = ItemState::new(
        rustred_campaign_core::worklist::items::repair_item(&key(2, 2), &[], &p).unwrap(),
    );
}

#[test]
fn wave0_offers_are_judged_like_searched_candidates_r43() {
    use rustred_campaign_core::worklist::items::NodeParams as NP;
    use rustred_campaign_core::worklist::supplied::{CandidateSource, SuppliedCandidate};
    struct Offer {
        good: bool,
    }
    impl CandidateSource for Offer {
        fn name(&self) -> &str {
            "scripted"
        }
        fn offer(&mut self, item: &Item, params: &NP) -> Vec<SuppliedCandidate> {
            let s = LabelSolver {
                params: params.clone(),
                answers: BTreeMap::new(),
                calls: vec![],
            };
            // a good offer covers everything; a bad one excepts every gap point (covers none): dropped
            let exc = if self.good {
                vec![]
            } else {
                item.gap_points.clone()
            };
            vec![SuppliedCandidate {
                candidate: s.rule(&item.case, &exc, 4, "supplied:x"),
                record: serde_json::json!({}),
                source_rule: "x".into(),
                instance: "0".into(),
            }]
        }
    }
    let p = p2();
    let cmp = rustred_campaign_core::order::NodeComparator::legacy(p.arity, None).unwrap();
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 3,
        line_priority: (0..3).collect(),
    };
    let root = root_item(&p, Face::new(vec![0, 1], vec![]), grid());
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([("line:0/fl/d2".to_string(), (vec![], 900))]),
        calls: vec![],
    };
    let mut good = Offer { good: true };
    let rep = drive_tree(
        vec![root.clone()],
        &p,
        &cmp,
        &mut s,
        &mut facts,
        &mut ReplayProver,
        None,
        Some(&mut good),
        &TreeLimits::default(),
    );
    match rep.items[0].disposition.as_ref().unwrap() {
        Disposition::Covered { cover } => assert_eq!(cover.members[0].digest, "supplied:x"),
        d => panic!("{d:?}"),
    }
    assert!(
        s.calls.is_empty(),
        "no search after a covering offer: {:?}",
        s.calls
    );
    let mut bad = Offer { good: false };
    let mut s = LabelSolver {
        params: p.clone(),
        answers: BTreeMap::from([("line:0/fl/d2".to_string(), (vec![], 900))]),
        calls: vec![],
    };
    let rep = drive_tree(
        vec![root],
        &p,
        &cmp,
        &mut s,
        &mut facts,
        &mut ReplayProver,
        None,
        Some(&mut bad),
        &TreeLimits::default(),
    );
    match rep.items[0].disposition.as_ref().unwrap() {
        Disposition::Covered { cover } => assert_eq!(cover.members[0].digest, "line:0/fl/d2"),
        d => panic!("{d:?}"),
    }
}
