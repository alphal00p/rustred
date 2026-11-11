//! rr-5l-api: the case-level queue API (initial cases, cover rules, guard routes).

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::input::{Compiler, Limits, LoweringLimits};
use crate::order::{CompiledOrder, OrderDescriptor};
use crate::sector::{Mask, zero};
use crate::solver::tests::{sunset, tadpole};
use crate::solver::{SectorConfig, SourceSystem};

fn zero_sectors<const N: usize>(family: &crate::family::IntegralFamily) -> Vec<[bool; N]> {
    let analyzer = zero::Analyzer::try_unrestricted(family).unwrap();
    (0..1_usize << N)
        .map(|bits| std::array::from_fn(|axis| bits & (1 << axis) != 0))
        .filter(|sector: &[bool; N]| {
            matches!(
                analyzer.analyze(&Mask::try_new(*sector).unwrap()).unwrap(),
                zero::Decision::ProvedZero(_)
            )
        })
        .collect()
}

/// (case, target, rhs, exceptions) of every rule, comparable across solves.
fn records<const N: usize>(rules: &[SectorRule<N>]) -> Vec<String> {
    rules
        .iter()
        .map(|r| format!("{}|{:?}", identity(r), r.exceptions))
        .collect()
}

/// (case, target, rhs): the identity of a rule, whatever route gave its exceptions.
fn identity<const N: usize>(r: &SectorRule<N>) -> String {
    format!(
        "{:?}|{:?}|{:?}",
        r.candidate.case, r.candidate.target, r.candidate.rhs
    )
}

fn is_subsequence(part: &[String], whole: &[String]) -> bool {
    let mut rest = whole.iter();
    part.iter().all(|item| rest.any(|w| w == item))
}

/// Every queue state of a solve: (pending, numerical, rules found so far).
type States<const N: usize> = Vec<(Vec<Case<N>>, Vec<CoordinateCase<N>>, usize)>;

fn solve_queue<const N: usize>(
    solver: &SectorSolver<'_, N>,
    queue: &CaseQueue<N>,
) -> (CaseQueueSolution<N>, States<N>) {
    let mut states = Vec::new();
    let solved = solver
        .solve_case_queue_deferring(
            SectorSolveOptions::default(),
            queue,
            |_| {},
            |event| {
                if let CaseQueueEvent::QueueState {
                    pending,
                    numerical,
                    rules,
                } = event
                {
                    states.push((pending.to_vec(), numerical.to_vec(), rules));
                }
            },
        )
        .unwrap();
    (solved, states)
}

/// Re-enter a solved queue at the middle state: initial = its pending and numerical cases,
/// cover = the rules found before it. The piece may prune more (a pending case is checked
/// against every earlier rule), never less: its rules are a subsequence of the rest of the
/// full solve and the residuals agree.
fn check_split<const N: usize>(
    solver: &SectorSolver<'_, N>,
    full: &CaseQueueSolution<N>,
    states: &States<N>,
    guard_route: GuardRoute,
) {
    let full_rules = &full.solved.solution.rules;
    assert!(states.len() >= 3, "a queue long enough to split");
    let (pending, numerical, k) = states[states.len() / 2].clone();
    assert!(k > 0);
    let mut initial = pending;
    initial.extend(numerical.into_iter().map(Case::from));
    let piece_queue = CaseQueue {
        initial: Some(initial),
        cover: full_rules[..k].iter().map(CoverRule::from).collect(),
        guard_route,
        ..CaseQueue::default()
    };
    let (piece, _) = solve_queue(solver, &piece_queue);
    // The content route may give the same exceptional set as other branches: compare
    // exceptions only on the native route.
    let view = |rules: &[SectorRule<N>]| -> Vec<String> {
        if guard_route == GuardRoute::Native {
            records(rules)
        } else {
            rules.iter().map(identity).collect()
        }
    };
    let rest = view(&full_rules[k..]);
    let got = view(&piece.solved.solution.rules);
    assert!(!got.is_empty());
    assert!(
        is_subsequence(&got, &rest),
        "piece rules are the rest of the full solve"
    );
    assert_eq!(
        piece.solved.solution.finite_residuals,
        full.solved.solution.finite_residuals
    );
    assert!(piece.solved.deferred.is_empty());
    // Without the cover the same piece re-solves cases the earlier rules own: never fewer rules.
    let uncovered = CaseQueue {
        cover: Vec::new(),
        ..piece_queue
    };
    let (alone, _) = solve_queue(solver, &uncovered);
    assert!(alone.solved.solution.rules.len() >= got.len());
    assert_eq!(alone.queue.cover_pruned, 0);
}

#[test]
fn the_default_case_queue_is_the_deferring_solve() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    let solver = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let reference = solver
        .solve_sector_deferring(SectorSolveOptions::default())
        .unwrap();
    let (queued, states) = solve_queue(&solver, &CaseQueue::default());
    assert_eq!(
        records(&queued.solved.solution.rules),
        records(&reference.solution.rules)
    );
    assert_eq!(
        queued.solved.solution.finite_residuals,
        reference.solution.finite_residuals
    );
    assert_eq!(queued.solved.rule_parents, reference.rule_parents);
    assert_eq!(queued.queue.initial_queued, 1);
    assert_eq!(queued.queue.cover_pruned, 0);
    assert_eq!(queued.queue.content_routes, 0);
    // One queue state per symbolic case, the first holding exactly the chart's case.
    assert_eq!(states.len(), queued.solved.solution.stats.symbolic_cases);
    assert_eq!(states[0], (vec![Case::generic()], Vec::new(), 0));
}

#[test]
fn a_split_queue_with_earlier_rules_as_cover_reproduces_the_rest() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    let solver = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let (full, states) = solve_queue(&solver, &CaseQueue::default());
    check_split(&solver, &full, &states, GuardRoute::Native);
}

#[test]
fn a_cover_rule_prunes_exactly_the_cases_it_covers() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let candidate = solver
        .solve_case(CoordinateCase::generic(), SearchOptions::default())
        .unwrap();
    let (rule, _) = solver.finish_rule(candidate).unwrap();
    let covered = CaseQueue {
        initial: Some(vec![CoordinateCase::new([Some(2)]).unwrap().into()]),
        cover: vec![CoverRule::from(&rule)],
        ..CaseQueue::default()
    };
    let (solved, states) = solve_queue(&solver, &covered);
    assert!(solved.solved.solution.rules.is_empty());
    assert!(states.is_empty());
    assert_eq!(
        (
            solved.queue.initial_queued,
            solved.queue.initial_dropped,
            solved.queue.cover_pruned
        ),
        (0, 1, 1)
    );
    // The exceptional face n0 = 1 is not covered: it is kept (a fixed point: numerical).
    let face = CaseQueue {
        initial: Some(vec![CoordinateCase::new([Some(1)]).unwrap().into()]),
        ..covered
    };
    let (solved, _) = solve_queue(&solver, &face);
    assert_eq!(
        (solved.queue.initial_queued, solved.queue.cover_pruned),
        (1, 0)
    );
    assert_eq!(solved.solved.solution.stats.numerical_cases, 1);
}

#[test]
fn initial_cases_outside_the_sector_are_refused() {
    let system = SourceSystem::<1>::from_family(&tadpole()).unwrap();
    let solver = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    let queue = CaseQueue {
        initial: Some(vec![CoordinateCase::new([Some(0)]).unwrap().into()]),
        ..CaseQueue::default()
    };
    assert!(matches!(
        solver.solve_case_queue_deferring(SectorSolveOptions::default(), &queue, |_| {}, |_| {}),
        Err(SectorSolveError::Search {
            source: SolverError::InvalidInput(_),
            ..
        })
    ));
}

#[test]
fn guard_routes_close_the_sector_with_the_native_residuals() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    let solver = SectorSolver::new(&system, [true; 3], SectorConfig::default()).unwrap();
    let (native, _) = solve_queue(&solver, &CaseQueue::default());
    let timed = |seconds| CaseQueue {
        guard_route: GuardRoute::NativeThenContent {
            native_timeout: Duration::from_secs_f64(seconds),
        },
        ..CaseQueue::default()
    };
    // A generous native try is the native solve, rule for rule.
    let (generous, _) = solve_queue(&solver, &timed(120.0));
    assert_eq!(
        records(&generous.solved.solution.rules),
        records(&native.solved.solution.rules)
    );
    assert_eq!(generous.queue.content_routes, 0);
    for queue in [
        CaseQueue {
            guard_route: GuardRoute::Content,
            ..CaseQueue::default()
        },
        timed(0.0),
    ] {
        let (routed, _) = solve_queue(&solver, &queue);
        assert_eq!(
            routed.solved.solution.finite_residuals,
            native.solved.solution.finite_residuals
        );
        assert!(routed.solved.deferred.is_empty());
    }
    let (content, _) = solve_queue(
        &solver,
        &CaseQueue {
            guard_route: GuardRoute::Content,
            ..CaseQueue::default()
        },
    );
    assert_eq!(
        content.queue.content_routes,
        content.solved.solution.stats.symbolic_cases
    );
}

/// The two-sided (eps) path: the 2L eps family T001c (power shifts on D1 and D5), one chart.
#[test]
fn the_two_sided_queue_reenters_and_routes_guards() {
    const N: usize = 5;
    let text = include_str!("../../../tests/support/eps_families/T001c.sym");
    let family = Compiler::new(Limits::default())
        .unwrap()
        .compile_compact(text, None)
        .unwrap()
        .into_lowered(LoweringLimits::default())
        .unwrap()
        .into_family();
    let mut system = SourceSystem::<N>::from_family_with_capacity(&family, true).unwrap();
    let axes = system.set_two_sided_eps_axes(&family).unwrap();
    assert_eq!(axes, [true, false, false, false, true]);
    let program = CompiledOrder::compile(
        OrderDescriptor::spired_uncut_two_sided(axes.to_vec()),
        Default::default(),
    )
    .unwrap();
    let config = SectorConfig {
        zero_sectors: Arc::from(zero_sectors::<N>(&family)),
        integral_order: Some(program),
        ..Default::default()
    };
    // Chart: D1 and D5 in n >= 1, every line active.
    let solver = SectorSolver::new(&system, [true; N], config).unwrap();
    let reference = solver
        .solve_sector_deferring_refined(SectorSolveOptions::default(), CaseRefinement::all())
        .unwrap();
    let queue = CaseQueue {
        refinement: CaseRefinement::all(),
        ..CaseQueue::default()
    };
    let (full, states) = solve_queue(&solver, &queue);
    assert_eq!(
        records(&full.solved.solution.rules),
        records(&reference.solution.rules)
    );
    assert_eq!(
        full.solved.solution.finite_residuals,
        reference.solution.finite_residuals
    );
    check_split(&solver, &full, &states, GuardRoute::Native);
    check_split(
        &solver,
        &full,
        &states,
        GuardRoute::NativeThenContent {
            native_timeout: Duration::ZERO,
        },
    );
    let (content, _) = solve_queue(
        &solver,
        &CaseQueue {
            guard_route: GuardRoute::Content,
            ..queue
        },
    );
    assert_eq!(
        content.solved.solution.finite_residuals,
        reference.solution.finite_residuals
    );
    assert!(content.solved.deferred.is_empty());
}

#[test]
fn per_case_numeric_endpoints_use_the_case_search_and_keep_the_residuals() {
    let system = SourceSystem::<3>::from_family(&sunset()).unwrap();
    for backend in [
        crate::solver::SymbolicExactBackend::Sparse,
        crate::solver::SymbolicExactBackend::SparseTargetOnlyFactorized,
    ] {
        let config = SectorConfig {
            symbolic_exact_backend: backend,
            ..Default::default()
        };
        let solver = SectorSolver::new(&system, [true; 3], config).unwrap();
        let (shared, _) = solve_queue(&solver, &CaseQueue::default());
        let (per_case, _) = solve_queue(
            &solver,
            &CaseQueue {
                numeric_endpoints: NumericEndpoints::PerCase,
                ..CaseQueue::default()
            },
        );
        let symbolic = shared.solved.solution.stats.symbolic_cases;
        assert_eq!(
            records(&per_case.solved.solution.rules[..symbolic]),
            records(&shared.solved.solution.rules[..symbolic])
        );
        assert_eq!(
            per_case.solved.solution.stats.numerical_cases,
            shared.solved.solution.stats.numerical_cases
        );
        assert_eq!(
            per_case.solved.solution.finite_residuals,
            shared.solved.solution.finite_residuals
        );
    }
}
