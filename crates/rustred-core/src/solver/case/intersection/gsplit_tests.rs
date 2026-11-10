//! Opt-in exact refinement ([`CaseRefinement`]: factor-first, saturation and
//! G-split) of the case-intersection engine.
//!
//! - Off (`CaseRefinement::default()`) is the established engine: identical
//!   cases, resolutions, deferred regions, failures and counters.
//! - On, every result stays exact on integer points (nothing lost, nothing
//!   invented), checked independently of the engine on boxes, on the captured
//!   T167 refusals (p1, p9, p16, original order) and on planted random inputs.
//! - On the captured p1 chart the coefficient-zero faces become ordinary cases
//!   and only the typed graph remainder stays deferred (Astra's 372 in-box
//!   points of the B5 unresolved branch).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use symbolica::utils::resources::{ResourceBudget, ResourceLimits};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::Case;
use super::defer_tests::{
    Lcg, T167, assert_checks, check_points, coordinate, core, equations, grid, in_region, on_chart,
    planted, row_through, value, vanishes,
};
use super::t167_captured_inputs::{CapturedT167, T167_ORIG, T167_P1_PF0};
use super::t167_gsplit_captured::{T167_P1_BOX_POINTS, T167_P9_B5, T167_P16_B5};
use super::t167_p1_b5_captured::T167_P1_B5;
use super::{
    CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionResult,
    CaseIntersectionStats, CaseRefinement, UnsupportedBranchPolicy, native,
};

const ALL: CaseRefinement = CaseRefinement::all();
const OFF: CaseRefinement = CaseRefinement {
    factor_first: false,
    saturate: false,
    graph_split: false,
};
const DEFER: UnsupportedBranchPolicy = UnsupportedBranchPolicy::Defer;
const REFUSE: UnsupportedBranchPolicy = UnsupportedBranchPolicy::Refuse;

type Outcome = Result<CaseIntersectionResult<20>, CaseIntersectionError<20>>;

fn untimed(stats: &CaseIntersectionStats) -> CaseIntersectionStats {
    CaseIntersectionStats {
        restriction_time: Duration::ZERO,
        admission_time: Duration::ZERO,
        normalization_time: Duration::ZERO,
        factorization_time: Duration::ZERO,
        union_time: Duration::ZERO,
        ..stats.clone()
    }
}

fn case_text(case: &Case<20>) -> String {
    let coupled = case.affine().map_or_else(Vec::new, |affine| {
        affine.equations().iter().map(ToString::to_string).collect()
    });
    let fixed: Vec<String> = case
        .fixed()
        .iter()
        .enumerate()
        .filter_map(|(axis, value)| value.map(|value| format!("n{axis}={value}")))
        .collect();
    format!("fixed {fixed:?} coupled {coupled:?}")
}

fn describe(label: &str, elapsed: Duration, result: &CaseIntersectionResult<20>) {
    println!(
        "{label}: {elapsed:?} cases={} deferred={} resolutions={:?}",
        result.cases.len(),
        result.deferred.len(),
        result
            .resolutions
            .iter()
            .map(|resolution| resolution.reason)
            .collect::<Vec<_>>()
    );
    println!("{label} refinement {:?}", result.refinement);
    println!("{label} stats {:?}", result.stats);
    for case in &result.cases {
        println!("{label} case {}", case_text(case));
    }
    for region in &result.deferred {
        println!(
            "{label} region chart {} conjunction {:?} exclusions {:?}",
            case_text(&region.chart),
            region
                .conjunction
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            region
                .exclusions
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        );
        for dependent in region.dependents.iter() {
            println!(
                "{label} region dependent n{} = ({}) / ({})",
                dependent.axis, dependent.numerator, dependent.denominator
            );
        }
    }
}

fn run(
    t: &T167,
    parent: &Case<20>,
    conjunction: &[CoefficientPolynomial],
    policy: UnsupportedBranchPolicy,
    refinement: CaseRefinement,
) -> (Duration, Outcome) {
    let started = Instant::now();
    let outcome = parent.intersect_many_refined(
        conjunction,
        &t.indices,
        &t.sector,
        CaseIntersectionLimits::default(),
        None,
        policy,
        refinement,
    );
    (started.elapsed(), outcome)
}

fn same_outcome(left: &Outcome, right: &Outcome) {
    match (left, right) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left.cases, right.cases);
            assert_eq!(left.resolutions, right.resolutions);
            assert_eq!(left.deferred, right.deferred);
            assert_eq!(left.max_numerator_rank, right.max_numerator_rank);
            assert_eq!(untimed(&left.stats), untimed(&right.stats));
            assert_eq!(left.refinement, right.refinement);
        }
        (Err(left), Err(right)) => {
            assert_eq!(left.failure, right.failure);
            assert_eq!(left.unresolved_parent, right.unresolved_parent);
            assert_eq!(left.unresolved_conjunction, right.unresolved_conjunction);
            assert_eq!(untimed(&left.stats), untimed(&right.stats));
        }
        (left, right) => panic!("outcomes differ: {left:?} vs {right:?}"),
    }
}

/// Every resolved case lies in the parent and in the ORIGINAL locus, every
/// deferred region in the parent, with affine (or no) exclusions.
fn assert_sound(
    t: &T167,
    parent: &Case<20>,
    original: &[CoefficientPolynomial],
    result: &CaseIntersectionResult<20>,
) {
    for case in &result.cases {
        assert!(parent.contains(case).unwrap());
        assert!(
            native::restrict(case, original, &t.indices)
                .unwrap()
                .is_empty(),
            "case off the locus: {}",
            case_text(case)
        );
    }
    for region in &result.deferred {
        assert!(parent.contains(&region.chart).unwrap());
    }
}

fn point_of(parent: &Case<20>, free: &[usize], values: &[i64]) -> [i64; 20] {
    let mut point = [0_i64; 20];
    for (axis, value) in parent.fixed().iter().enumerate() {
        point[axis] = value.map_or(0, i64::from);
    }
    for (slot, &axis) in free.iter().enumerate() {
        point[axis] = values[slot];
    }
    point
}

fn free_axes(t: &T167, parent: &Case<20>) -> Vec<usize> {
    (0..20)
        .filter(|&axis| t.sector[axis] && parent.fixed()[axis].is_none())
        .collect()
}

/// The chart of the face `equalities` inside `parent`.
fn face(t: &T167, parent: &Case<20>, equalities: &[&str]) -> Case<20> {
    parent
        .intersect(&equations(&t.context, equalities), &t.indices, &t.sector)
        .unwrap()
        .expect("nonempty face")
}

/// (a) Off is the established engine on every captured input, both policies.
#[test]
fn refinement_off_is_the_established_engine() {
    for fixture in [&T167_P1_B5, &T167_P1_PF0, &T167_P9_B5, &T167_P16_B5] {
        let t = T167::new(fixture);
        let parent = t.chart(fixture.original_fixed, fixture.original_equalities);
        let original = equations(&t.context, fixture.original_conjunction);
        for policy in [REFUSE, DEFER] {
            let established = parent.intersect_many_with_policy(
                &original,
                &t.indices,
                &t.sector,
                CaseIntersectionLimits::default(),
                None,
                policy,
            );
            let (_, off) = run(&t, &parent, &original, policy, OFF);
            same_outcome(&established, &off);
            if let Ok(result) = &off {
                assert!(
                    result
                        .deferred
                        .iter()
                        .all(|region| region.exclusions.is_empty() && region.dependents.is_empty())
                );
            }
        }
    }
}

/// (b) The captured p1 chart (B5 refusal): G-split leaves exactly one typed
/// graph remainder deferred (n0 and n2 dependent, denominators built from
/// a = n6-1 and L = n4-n6+n9-2). The coefficient-zero faces are ordinary
/// resolved cases. Exact on Astra's 836 in-box original zeros and on the box
/// 1..=5 of the six free coordinates; counts the moved U points.
#[test]
fn t167_p1_gsplit_defers_only_the_graph_remainder() {
    let fixture = &T167_P1_B5;
    let t = T167::new(fixture);
    let parent = t.chart(fixture.original_fixed, fixture.original_equalities);
    let original = equations(&t.context, fixture.original_conjunction);
    let (elapsed, outcome) = run(&t, &parent, &original, DEFER, ALL);
    let result = outcome.unwrap();
    describe("T167_P1_GSPLIT", elapsed, &result);
    assert_sound(&t, &parent, &original, &result);
    // Astra's box points: every original zero is covered; count U points
    // that now lie in an ordinary case and in no deferred region.
    let (mut covered, mut moved, mut kept) = (0, 0, Vec::new());
    for (values, on_u, branch) in T167_P1_BOX_POINTS {
        let point: [i64; 20] = std::array::from_fn(|axis| i64::from(values[axis]));
        assert!(on_chart(&parent, &point) && vanishes(&original, &t.indices, &point));
        let in_case = result.cases.iter().any(|case| on_chart(case, &point));
        let in_deferred = result
            .deferred
            .iter()
            .any(|region| in_region(region, &t.indices, &point));
        assert!(in_case || in_deferred, "box point {point:?} lost");
        covered += 1;
        if *on_u {
            if in_case && !in_deferred {
                moved += 1;
            } else {
                kept.push((point, *branch));
            }
        }
    }
    println!(
        "T167_P1_GSPLIT box: original zeros {covered}, U points {}, moved to ordinary cases {moved}, still deferred {kept:?}",
        T167_P1_BOX_POINTS
            .iter()
            .filter(|(_, on_u, _)| *on_u)
            .count()
    );
    // Exactness on the box 1..=5 of the six free coordinates.
    let free = free_axes(&t, &parent);
    assert_eq!(free, vec![0, 2, 3, 4, 6, 9]);
    let points = grid([1; 6], [5; 6]).map(|values| point_of(&parent, &free, &values));
    let count = check_points(&parent, &original, &result, &t.indices, &t.sector, points);
    println!("T167_P1_GSPLIT box 1..=5 {count:?}");
    // The four affine coefficient-zero faces are ordinary cases.
    for equalities in [
        &["n6-1", "n4-n9"][..],
        &["n6-1", "n4-2", "n3-n9+1"][..],
        &["n6-1", "n9-1", "n3-n4+2"][..],
        &["n6-2", "n4-1", "n9-3", "n3-n0-n2-1"][..],
    ] {
        let face = face(&t, &parent, equalities);
        assert!(
            native::restrict(&face, &original, &t.indices)
                .unwrap()
                .is_empty()
        );
        assert!(
            result
                .cases
                .iter()
                .any(|case| case.contains(&face).unwrap()),
            "face {equalities:?} not an ordinary case"
        );
    }
    // Exactly one deferred region: the graph remainder, n0 and n2 dependent.
    assert_eq!(result.deferred.len(), 1);
    let region = &result.deferred[0];
    let mut axes: Vec<usize> = region.dependents.iter().map(|d| d.axis).collect();
    axes.sort_unstable();
    assert_eq!(axes, vec![0, 2]);
    assert!(region.exclusions.iter().all(native::is_affine));
    assert_eq!(moved, 370);
    assert_eq!(kept.len(), 2);
    assert!(kept.iter().all(|(_, branch)| *branch == 2));
}

/// The captured unresolved branch alone (B5 GB basis on the same chart).
#[test]
fn t167_p1_unresolved_branch_alone_is_split_the_same_way() {
    let fixture = &T167_P1_B5;
    let t = T167::new(fixture);
    let chart = t.chart(fixture.unresolved_fixed, fixture.unresolved_equalities);
    let branch = equations(&t.context, fixture.unresolved_conjunction);
    let (elapsed, outcome) = run(&t, &chart, &branch, DEFER, ALL);
    let result = outcome.unwrap();
    describe("T167_P1_BRANCH_GSPLIT", elapsed, &result);
    assert_sound(&t, &chart, &branch, &result);
    let free = free_axes(&t, &chart);
    let points = grid([1; 6], [5; 6]).map(|values| point_of(&chart, &free, &values));
    let count = check_points(&chart, &branch, &result, &t.indices, &t.sector, points);
    println!("T167_P1_BRANCH_GSPLIT box 1..=5 {count:?}");
    assert!(
        result
            .deferred
            .iter()
            .all(|region| !region.dependents.is_empty())
    );
}

fn captured_gsplit(label: &str, fixture: &CapturedT167, high: i64) {
    let t = T167::new(fixture);
    let parent = t.chart(fixture.original_fixed, fixture.original_equalities);
    let original = equations(&t.context, fixture.original_conjunction);
    let (elapsed, refused) = run(&t, &parent, &original, DEFER, OFF);
    let refused = refused.unwrap();
    println!(
        "{label} defer-only: {elapsed:?} cases={} deferred={}",
        refused.cases.len(),
        refused.deferred.len()
    );
    let (elapsed, outcome) = run(&t, &parent, &original, DEFER, ALL);
    let result = outcome.unwrap();
    describe(label, elapsed, &result);
    assert_sound(&t, &parent, &original, &result);
    let free = free_axes(&t, &parent);
    let low = vec![1_i64; free.len()];
    let highs = vec![high; free.len()];
    let total: usize = (0..free.len()).map(|_| high as usize).product();
    let points = (0..total).map(|mut ordinal| {
        let values: Vec<i64> = (0..free.len())
            .map(|slot| {
                let width = (highs[slot] - low[slot] + 1) as usize;
                let value = low[slot] + (ordinal % width) as i64;
                ordinal /= width;
                value
            })
            .collect();
        point_of(&parent, &free, &values)
    });
    let count = check_points(&parent, &original, &result, &t.indices, &t.sector, points);
    let defer_only_points = (0..total).map(|mut ordinal| {
        let values: Vec<i64> = (0..free.len())
            .map(|slot| {
                let width = (highs[slot] - low[slot] + 1) as usize;
                let value = low[slot] + (ordinal % width) as i64;
                ordinal /= width;
                value
            })
            .collect();
        point_of(&parent, &free, &values)
    });
    let before = check_points(
        &parent,
        &original,
        &refused,
        &t.indices,
        &t.sector,
        defer_only_points,
    );
    println!("{label} box 1..={high} on {free:?}: defer-only {before:?}; gsplit {count:?}");
    assert!(count.only_in_regions <= before.only_in_regions);
    for region in &result.deferred {
        assert!(region.exclusions.iter().all(native::is_affine));
    }
}

/// (c) p9 and p16 B5 refusals: exact under G-split, never more deferred box
/// points than with S4 alone.
#[test]
fn t167_p9_gsplit_is_exact() {
    captured_gsplit("T167_P9_GSPLIT", &T167_P9_B5, 4);
}

#[test]
fn t167_p16_gsplit_is_exact() {
    captured_gsplit("T167_P16_GSPLIT", &T167_P16_B5, 5);
}

/// (d) The original order (`--ignored`, the established engine needs hours of
/// F4 here): with the refinement the complete intersection finishes under
/// both policies, every case lies in the locus, the five affine faces F1..F5
/// of the diagnosis are covered, and nothing is deferred (the quadric graph
/// remainders lie on charts with n1 = -2 n0, which miss the sector).
#[test]
#[ignore = "release only: heavy original-order intersection (opt-in refinement)"]
fn t167_original_order_refined_resolves_the_five_faces() {
    let t = T167::new(&T167_ORIG);
    let parent = t.chart(T167_ORIG.original_fixed, T167_ORIG.original_equalities);
    let original = equations(&t.context, T167_ORIG.original_conjunction);
    let (elapsed, deferring) = run(&t, &parent, &original, DEFER, ALL);
    let deferring = deferring.unwrap();
    describe("T167_ORIG_GSPLIT_DEFER", elapsed, &deferring);
    let (elapsed, outcome) = run(&t, &parent, &original, REFUSE, ALL);
    let result = outcome.unwrap();
    describe("T167_ORIG_GSPLIT", elapsed, &result);
    assert_sound(&t, &parent, &original, &result);
    assert_eq!(deferring.cases, result.cases);
    assert!(deferring.deferred.is_empty());
    for (name, equalities) in [
        ("F1", ["n4-1", "n5-1"]),
        ("F2", ["n4-1", "n5-n6"]),
        ("F3", ["n2-1", "n4-1"]),
        ("F4", ["n6-1", "n4-n5"]),
        ("F5", ["n2-1", "n5-n6"]),
    ] {
        let face = face(&t, &parent, &equalities);
        assert!(
            native::restrict(&face, &original, &t.indices)
                .unwrap()
                .is_empty()
        );
        let covered = result
            .cases
            .iter()
            .any(|case| case.contains(&face).unwrap());
        println!("T167_ORIG_GSPLIT {name} {equalities:?} covered by one case: {covered}");
        assert!(covered, "{name} not covered");
    }
    assert!(result.deferred.is_empty());
    assert_eq!(result.cases.len(), 5);
    assert!(result.refinement.sector_empty_branches > 0);
}

/// (e) Planted points: with every refinement step on, every intersection
/// stays exact on the box [-3,5]^4 (nothing lost, nothing invented, no
/// resolved or deferred point off the locus, exclusions respected). Inputs
/// include reducible equations (factor-first, saturation by sibling factors)
/// and equations linear in one coordinate with an affine coefficient
/// (G-split).
#[test]
#[allow(clippy::result_large_err)]
fn refinement_never_loses_or_invents_a_point() {
    let context = CoefficientContext::new(["d", "a", "b", "c", "e"]);
    let indices = [1, 2, 3, 4];
    let mut random = Lcg(0x6511_7e5a_7a7e);
    let (mut ok, mut refused, mut timeouts) = (0, 0, 0);
    let (mut splits, mut graph, mut saturations, mut deferred, mut dropped) = (0, 0, 0, 0, 0);
    for iteration in 0..240 {
        let point: [i64; 4] = std::array::from_fn(|_| random.range(-3, 5));
        let sector: [bool; 4] = std::array::from_fn(|axis| point[axis] >= 1);
        let mut rows = Vec::new();
        for _ in 0..random.range(0, 1) {
            rows.extend(row_through(&mut random, point));
        }
        let mut fixed = [None; 4];
        if random.range(0, 3) == 0 {
            let axis = random.range(0, 3) as usize;
            fixed[axis] = Some(point[axis] as i16);
        }
        let row_refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let parent = coordinate(fixed)
            .intersect(&equations(&context, &row_refs), &indices, &sector)
            .unwrap()
            .expect("the chart contains the planted point");
        let variables = random.range(2, 4) as usize;
        let first = planted(&mut random, point, variables);
        let second = planted(&mut random, point, variables);
        // Linear in `a` with an affine coefficient through the point's (b, c).
        let shift_b = point[1] + random.range(-1, 1);
        let shift_c = random.range(-2, 2);
        let linear = format!(
            "((b-({shift_b}))+({shift_c})*(c-({})))*(a-({}))+({second})",
            point[2], point[0]
        );
        let obstruction =
            ["a^2-3*b^2-2", "a^2+b^2-3*c^2-3", "b*c-2*a-1"][random.range(0, 2) as usize];
        let affine = format!("(b-({}))", point[1] + random.range(0, 1));
        let conjunction: Vec<String> = match random.range(0, 4) {
            0 => vec![
                format!("{affine}*({first})"),
                format!("{affine}*({second})"),
            ],
            1 => vec![linear, first],
            2 => vec![format!("{affine}*({obstruction})*({first})"), second],
            3 => vec![linear, format!("({first})*({obstruction})")],
            _ => vec![first, second],
        };
        let conjunction_refs: Vec<&str> = conjunction.iter().map(String::as_str).collect();
        let conjunction = equations(&context, &conjunction_refs);
        let (sender, receiver) = mpsc::channel();
        {
            let (parent, conjunction) = (parent.clone(), conjunction.clone());
            std::thread::spawn(move || {
                let budget = ResourceBudget::new(ResourceLimits {
                    max_process_rss_bytes: None,
                    max_elapsed: Some(Duration::from_secs(10)),
                });
                let result = budget.run(|| {
                    parent.intersect_many_refined(
                        &conjunction,
                        &indices,
                        &sector,
                        CaseIntersectionLimits::default().with_diagnostics(),
                        None,
                        DEFER,
                        ALL,
                    )
                });
                let _ = sender.send(result);
            });
        }
        let Ok(result) = receiver.recv_timeout(Duration::from_secs(30)) else {
            eprintln!(
                "GSPLIT_PLANTED_TIMEOUT iteration {iteration}: {conjunction_refs:?} {rows:?}"
            );
            timeouts += 1;
            continue;
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                assert!(
                    !matches!(
                        error.failure,
                        super::CaseIntersectionFailure::UnsupportedGeometry
                            | super::CaseIntersectionFailure::RepeatedState
                            | super::CaseIntersectionFailure::InvalidInput(_)
                            | super::CaseIntersectionFailure::NativeAlgebra
                    ),
                    "iteration {iteration}: refined Defer returned {:?} on {conjunction_refs:?} {rows:?}",
                    error.failure
                );
                refused += 1;
                continue;
            }
        };
        ok += 1;
        splits += result.refinement.factor_first_splits;
        graph += result.refinement.graph_splits;
        saturations += result.refinement.saturations;
        deferred += result.deferred.len();
        dropped += result.refinement.excluded_cases_dropped;
        let in_case = result.cases.iter().any(|case| on_chart(case, &point));
        let in_deferred = result
            .deferred
            .iter()
            .any(|region| in_region(region, &indices, &point));
        assert!(
            in_case || in_deferred,
            "iteration {iteration}: planted point {point:?} lost: {conjunction_refs:?} {rows:?} {result:?}"
        );
        for case in &result.cases {
            assert!(parent.contains(case).unwrap());
            assert!(
                native::restrict(case, &conjunction, &indices)
                    .unwrap()
                    .is_empty(),
                "iteration {iteration}: case off the locus"
            );
        }
        for region in &result.deferred {
            for exclusion in region.exclusions.iter() {
                assert!(exclusion.nterms() > 0);
            }
            let _ = value(&region.conjunction[0], &indices, &point);
        }
        check_points(
            &parent,
            &conjunction,
            &result,
            &indices,
            &sector,
            grid([-3; 4], [5; 4]),
        );
    }
    println!(
        "GSPLIT_PLANTED ok={ok} refused={refused} timeouts={timeouts} factor_first_splits={splits} \
         graph_splits={graph} saturations={saturations} deferred={deferred} dropped_cases={dropped}"
    );
    assert_eq!(ok + refused + timeouts, 240);
    assert!(ok > 200 && splits > 0 && graph > 0 && saturations > 0);
}

/// Refinement under `Defer` skips the unbounded Lex F4 fallback (stage 3). A
/// branch that the established engine sends into Lex F4 (two irreducible
/// Pell quadrics in disjoint variables: no factor, no linear coordinate, no
/// exact step applies) is then deferred, and its region is the same full
/// record as the established S4 deferral: reason, parent, chart,
/// conjunction, S1/S2 checks, no exclusions, no dependents. Conservative:
/// its points lie in no resolved case (never claimed covered).
#[test]
fn a_skipped_lex_branch_is_deferred_with_the_full_record() {
    let context = CoefficientContext::new(["x", "y", "z", "w"]);
    let indices = [0, 1, 2, 3];
    let sector = [true; 4];
    let generic = Case::<4>::generic();
    let conjunction = equations(&context, &["x^2-2*y^2-1", "z^2-3*w^2-1"]);
    let solve = |refinement| {
        generic
            .intersect_many_refined(
                &conjunction,
                &indices,
                &sector,
                CaseIntersectionLimits::default(),
                None,
                DEFER,
                refinement,
            )
            .unwrap()
    };
    native::take_lex_calls();
    let established = solve(OFF);
    assert!(
        native::take_lex_calls() > 0,
        "the established engine must enter Lex F4 on this branch"
    );
    assert_eq!(established.refinement.lex_skipped, 0);
    let refined = solve(ALL);
    assert_eq!(
        native::take_lex_calls(),
        0,
        "refinement under Defer never enters Lex F4"
    );
    assert!(
        refined.refinement.lex_skipped >= 1,
        "{:?}",
        refined.refinement
    );
    assert!(refined.cases.is_empty() && established.cases.is_empty());
    assert_eq!(refined.deferred.len(), 1);
    let region = &refined.deferred[0];
    assert_eq!(
        core(region),
        (
            CaseIntersectionFailure::UnsupportedGeometry,
            generic.clone(),
            generic.clone(),
            native::restrict(&generic, &conjunction, &indices).unwrap(),
            Vec::new(),
            Vec::new(),
        )
    );
    assert_checks(region);
    // The same record as the established deferral (B7), checks included.
    assert_eq!(refined.deferred, established.deferred);
    // Nothing lost or invented; every locus point is only in the region.
    for point in [[3, 2, 2, 1], [17, 12, 7, 4], [99, 70, 26, 15]] {
        assert!(in_region(region, &indices, &point), "{point:?}");
    }
    for point in [[3, 2, 2, 2], [4, 2, 2, 1]] {
        assert!(!in_region(region, &indices, &point), "{point:?}");
    }
    for result in [&refined, &established] {
        let count = check_points(
            &generic,
            &conjunction,
            result,
            &indices,
            &sector,
            grid([1; 4], [20, 15, 10, 6]),
        );
        assert_eq!(count.locus, 2 * 2); // (3,2), (17,12) x (2,1), (7,4)
        assert_eq!(count.only_in_regions, count.locus);
        assert_eq!(count.in_cases, 0);
    }
    println!(
        "LEX_SKIP_DEFER refined {:?} established checks {:?}",
        refined.refinement, established.deferred[0].checks
    );
}
