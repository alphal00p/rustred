//! S4 (opt-in "defer unsupported"): an AND branch that refinement, S1 and S2
//! leave unresolved becomes an exact [`DeferredRegion`] instead of failing the
//! whole intersection; `Refuse` (the default) is the unchanged atomic refusal.
//!
//! Exactness is checked on integer points, independently of the engine: every
//! resolved case and every deferred region lies in parent AND locus (nothing is
//! invented), and every point of parent AND locus lies in a resolved case or a
//! deferred region (nothing is lost). Planted points of a deferred region lie in
//! no resolved case (no rule is derived for them through the union).

use std::sync::Arc;
use std::time::{Duration, Instant};

use symbolica::prelude::Integer;
use symbolica::utils::resources::{ResourceBudget, ResourceLimits};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::engine::settle_stuck_branch_for_test;
use super::t167_captured_inputs::CapturedT167;
use super::t167_p1_b5_captured::T167_P1_B5;
use super::{
    CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits,
    CaseIntersectionResult, CaseResolutionReason, DeferredRegion, UnsupportedBranchPolicy, native,
};

const DEFER: UnsupportedBranchPolicy = UnsupportedBranchPolicy::Defer;
const REFUSE: UnsupportedBranchPolicy = UnsupportedBranchPolicy::Refuse;

pub(super) fn equations(
    context: &CoefficientContext,
    values: &[&str],
) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

/// Exact value of an integer polynomial in the index variables at an integer
/// point (i128 arithmetic; test-sized values only).
pub(super) fn value<const N: usize>(
    polynomial: &CoefficientPolynomial,
    indices: &[usize; N],
    point: &[i64; N],
) -> i128 {
    let nvars = polynomial.nvars();
    let mut total = 0_i128;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        let mut product = i128::from(coefficient.to_i64().expect("small coefficient"));
        for (variable, &power) in polynomial.exponents[term * nvars..(term + 1) * nvars]
            .iter()
            .enumerate()
        {
            if power == 0 {
                continue;
            }
            let axis = indices
                .iter()
                .position(|&index| index == variable)
                .expect("only index variables occur");
            product *= i128::from(point[axis]).pow(u32::from(power));
        }
        total += product;
    }
    total
}

pub(super) fn vanishes<const N: usize>(
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    point: &[i64; N],
) -> bool {
    conjunction
        .iter()
        .all(|polynomial| value(polynomial, indices, point) == 0)
}

/// Exact membership in a chart: fixed coordinates and every primitive row.
pub(super) fn on_chart<const N: usize>(chart: &Case<N>, point: &[i64; N]) -> bool {
    let fixed = chart
        .fixed()
        .iter()
        .zip(point)
        .all(|(value, &coordinate)| value.is_none_or(|value| i64::from(value) == coordinate));
    let rows = chart.affine().is_none_or(|affine| {
        affine.primitive_matrix().row_iter().all(|row| {
            let entry = |value: &Integer| i128::from(value.to_i64().expect("small chart row"));
            let lhs: i128 = (0..N)
                .map(|axis| entry(&row[axis]) * i128::from(point[axis]))
                .sum();
            lhs == entry(&row[N])
        })
    });
    fixed && rows
}

/// Membership through the public predicate list only, cross-checked against
/// the region's chart and conjunction.
pub(super) fn in_region<const N: usize>(
    region: &DeferredRegion<N>,
    indices: &[usize; N],
    point: &[i64; N],
) -> bool {
    let by_predicates = vanishes(&region.predicates(indices).unwrap(), indices, point);
    let by_parts = on_chart(&region.chart, point) && vanishes(&region.conjunction, indices, point);
    assert_eq!(by_predicates, by_parts, "predicates disagree at {point:?}");
    // Opt-in refinement only (empty otherwise): every exclusion must be nonzero.
    by_predicates
        && region
            .exclusions
            .iter()
            .all(|exclusion| value(exclusion, indices, point) != 0)
}

pub(super) fn in_sector<const N: usize>(sector: &[bool; N], point: &[i64; N]) -> bool {
    point
        .iter()
        .zip(sector)
        .all(|(&coordinate, &active)| (coordinate >= 1) == active)
}

#[derive(Debug, Default)]
pub(super) struct BoxCount {
    pub(super) sector_points: usize,
    pub(super) locus: usize,
    pub(super) in_cases: usize,
    pub(super) in_regions: usize,
    pub(super) only_in_regions: usize,
}

/// Exactness of a successful (possibly deferred) result on `points` within
/// the sector, against the ORIGINAL parent and conjunction.
pub(super) fn check_points<const N: usize>(
    parent: &Case<N>,
    original: &[CoefficientPolynomial],
    result: &CaseIntersectionResult<N>,
    indices: &[usize; N],
    sector: &[bool; N],
    points: impl Iterator<Item = [i64; N]>,
) -> BoxCount {
    let mut count = BoxCount::default();
    for point in points.filter(|point| in_sector(sector, point)) {
        count.sector_points += 1;
        let locus = on_chart(parent, &point) && vanishes(original, indices, &point);
        let case = result.cases.iter().any(|case| on_chart(case, &point));
        let region = result
            .deferred
            .iter()
            .any(|region| in_region(region, indices, &point));
        assert!(
            !case || locus,
            "resolved case point {point:?} off the locus"
        );
        assert!(!region || locus, "deferred point {point:?} off the locus");
        assert!(!locus || case || region, "locus point {point:?} lost");
        count.locus += usize::from(locus);
        count.in_cases += usize::from(case);
        count.in_regions += usize::from(region);
        count.only_in_regions += usize::from(region && !case);
    }
    count
}

pub(super) fn grid<const N: usize>(
    low: [i64; N],
    high: [i64; N],
) -> impl Iterator<Item = [i64; N]> {
    let total: usize = (0..N)
        .map(|axis| (high[axis] - low[axis] + 1) as usize)
        .product();
    (0..total).map(move |mut ordinal| {
        std::array::from_fn(|axis| {
            let width = (high[axis] - low[axis] + 1) as usize;
            let coordinate = low[axis] + (ordinal % width) as i64;
            ordinal /= width;
            coordinate
        })
    })
}

pub(super) fn coordinate<const N: usize>(fixed: [Option<i16>; N]) -> Case<N> {
    CoordinateCase::new(fixed).unwrap().into()
}

#[test]
fn refuse_policy_is_the_unchanged_refusal_and_defer_changes_nothing_resolvable() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    for input in [&["x^2-2*y^2-1"][..], &["(z-1)*(x^2-2*y^2-1)"][..]] {
        let conjunction = equations(&context, input);
        let old = generic
            .intersect_many(&conjunction, &indices, &sector, Default::default())
            .unwrap_err();
        let new = generic
            .intersect_many_with_policy(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                None,
                REFUSE,
            )
            .unwrap_err();
        assert_eq!(new.failure, CaseIntersectionFailure::UnsupportedGeometry);
        assert_eq!(new.failure, old.failure);
        assert_eq!(new.unresolved_parent, old.unresolved_parent);
        assert_eq!(new.unresolved_conjunction, old.unresolved_conjunction);
        assert_eq!(new.original_parent, old.original_parent);
        assert_eq!(new.original_conjunction, old.original_conjunction);
        let ranked = generic
            .intersect_many_with_policy(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                Some(4),
                REFUSE,
            )
            .unwrap_err();
        let old_ranked = generic
            .intersect_many_with_max_numerator_rank(
                &conjunction,
                &indices,
                &sector,
                Default::default(),
                4,
            )
            .unwrap_err();
        assert_eq!(ranked.failure, old_ranked.failure);
        assert_eq!(ranked.unresolved_parent, old_ranked.unresolved_parent);
    }
    // Resolvable inputs (S1 lift, S2, plain factors): identical under all three.
    for input in [
        &["(x-1)*(y^2-2*z^2-1)", "(x-1)*(x^2+1)"][..],
        &["x^2-3*y^2-2"][..],
        &["(x-1)*(y-2)", "z-3"][..],
    ] {
        let conjunction = equations(&context, input);
        let old = generic
            .intersect_many(&conjunction, &indices, &sector, Default::default())
            .unwrap();
        for policy in [REFUSE, DEFER] {
            let new = generic
                .intersect_many_with_policy(
                    &conjunction,
                    &indices,
                    &sector,
                    Default::default(),
                    None,
                    policy,
                )
                .unwrap();
            assert_eq!(new.cases, old.cases, "{input:?} {policy:?}");
            assert_eq!(new.resolutions, old.resolutions);
            assert!(new.deferred.is_empty());
            assert!(old.deferred.is_empty());
        }
    }
}

#[test]
fn a_stuck_pell_branch_is_deferred_exactly() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    let conjunction = equations(&context, &["x^2-2*y^2-1"]);
    let result = generic
        .intersect_many_with_policy(
            &conjunction,
            &indices,
            &sector,
            Default::default(),
            None,
            DEFER,
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert_eq!(
        result.deferred,
        vec![DeferredRegion {
            reason: CaseIntersectionFailure::UnsupportedGeometry,
            parent: generic.clone(),
            chart: generic.clone(),
            conjunction: native::restrict(&generic, &conjunction, &indices)
                .unwrap()
                .into(),
            exclusions: Arc::from([]),
            dependents: Arc::from([]),
        }]
    );
    let region = &result.deferred[0];
    // Planted Pell points (any z) are in the region; near misses are not.
    for point in [[3, 2, 7], [17, 12, 1], [99, 70, 2], [577, 408, 5]] {
        assert!(in_region(region, &indices, &point), "{point:?}");
        assert!(!result.cases.iter().any(|case| on_chart(case, &point)));
    }
    for point in [[2, 1, 1], [4, 3, 1], [1, 1, 1]] {
        assert!(!in_region(region, &indices, &point), "{point:?}");
    }
    let count = check_points(
        &generic,
        &conjunction,
        &result,
        &indices,
        &sector,
        grid([1, 1, 1], [100, 72, 2]),
    );
    assert_eq!(count.locus, 2 * 3); // (3,2), (17,12), (99,70) at z = 1, 2
    assert_eq!(count.only_in_regions, count.locus);
    println!("S4_PELL {count:?}");
}

#[test]
fn deferred_siblings_leave_the_resolved_cases_exact() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    let conjunction = equations(&context, &["(z-1)*(x^2-2*y^2-1)"]);
    let result = generic
        .intersect_many_with_policy(
            &conjunction,
            &indices,
            &sector,
            Default::default(),
            None,
            DEFER,
        )
        .unwrap();
    let z_is_one = coordinate([None, None, Some(1)]);
    assert_eq!(result.cases, vec![z_is_one]);
    assert_eq!(result.deferred.len(), 1);
    let region = &result.deferred[0];
    assert_eq!(region.reason, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(region.parent, generic);
    assert!(generic.contains(&region.chart).unwrap());
    assert_eq!(
        region.conjunction.as_ref(),
        equations(&context, &["x^2-2*y^2-1"]).as_slice()
    );
    // A planted Pell point off z = 1 is only deferred; z = 1 points are cases.
    assert!(in_region(region, &indices, &[3, 2, 5]));
    assert!(!result.cases.iter().any(|case| on_chart(case, &[3, 2, 5])));
    assert!(!in_region(region, &indices, &[4, 4, 1]));
    assert!(on_chart(&result.cases[0], &[4, 4, 1]));
    let count = check_points(
        &generic,
        &conjunction,
        &result,
        &indices,
        &sector,
        grid([1, 1, 1], [20, 14, 4]),
    );
    assert!(count.only_in_regions == 2 * 3 && count.in_cases == 20 * 14);
    println!("S4_SIBLINGS {count:?}");
    // Refuse still fails atomically on the same input.
    assert!(
        generic
            .intersect_many(&conjunction, &indices, &sector, Default::default())
            .is_err()
    );
}

#[test]
fn defer_settles_both_refusals_only_after_s1_and_s2() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    let original = equations(&context, &["(x-1)*(y^2-2*z^2-1)", "(x-1)*(x-2)"]);
    let branch = equations(&context, &["y^2-2*z^2-1"]);
    let x_is = |value| coordinate([Some(value), None, None]);
    for failure in [
        CaseIntersectionFailure::UnsupportedGeometry,
        CaseIntersectionFailure::RepeatedState,
    ] {
        // x = 2: neither S1 nor S2 applies. Refuse keeps the refusal, Defer
        // records exactly the branch.
        let refused = settle_stuck_branch_for_test(
            &generic,
            &original,
            &x_is(2),
            &branch,
            &indices,
            &sector,
            REFUSE,
            failure.clone(),
        )
        .unwrap_err();
        assert_eq!(refused, failure);
        let (resolved, resolutions, deferred) = settle_stuck_branch_for_test(
            &generic,
            &original,
            &x_is(2),
            &branch,
            &indices,
            &sector,
            DEFER,
            failure.clone(),
        )
        .unwrap();
        assert!(resolved.is_empty() && resolutions.is_empty());
        assert_eq!(
            deferred,
            vec![DeferredRegion {
                reason: failure.clone(),
                parent: generic.clone(),
                chart: x_is(2),
                conjunction: native::restrict(&x_is(2), &branch, &indices)
                    .unwrap()
                    .into(),
                exclusions: Arc::from([]),
                dependents: Arc::from([]),
            }]
        );
        assert!(in_region(&deferred[0], &indices, &[2, 3, 2]));
        assert!(!in_region(&deferred[0], &indices, &[1, 3, 2]));
        // x = 1: S1 lifts first; nothing is deferred.
        let (resolved, resolutions, deferred) = settle_stuck_branch_for_test(
            &generic,
            &original,
            &x_is(1),
            &branch,
            &indices,
            &sector,
            DEFER,
            failure.clone(),
        )
        .unwrap();
        assert_eq!(resolved, vec![x_is(1)]);
        assert_eq!(resolutions[0].reason, CaseResolutionReason::Lifted);
        assert!(deferred.is_empty());
        // A modular-empty branch is dropped by S2, never deferred.
        let empty = equations(&context, &["x^2-3*y^2-2"]);
        let (resolved, resolutions, deferred) = settle_stuck_branch_for_test(
            &generic, &empty, &generic, &empty, &indices, &sector, DEFER, failure,
        )
        .unwrap();
        assert!(resolved.is_empty() && deferred.is_empty());
        assert_eq!(
            resolutions[0].reason,
            CaseResolutionReason::ModularEmpty { prime: 3 }
        );
    }
}

#[test]
fn defer_never_hides_budget_failures() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let conjunction = equations(&context, &["(z-1)*(x^2-2*y^2-1)"]);
    let limits = CaseIntersectionLimits {
        diagnostics_enabled: true,
        max_work_items: 1,
        ..Default::default()
    };
    let error = Case::<3>::generic()
        .intersect_many_with_policy(&conjunction, &[0, 1, 2], &[true; 3], limits, None, DEFER)
        .unwrap_err();
    assert_eq!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            limit: 1,
        }
    );
}

pub(super) struct T167 {
    pub(super) context: CoefficientContext,
    pub(super) indices: [usize; 20],
    pub(super) sector: [bool; 20],
}

impl T167 {
    pub(super) fn new(fixture: &CapturedT167) -> Self {
        let names: Vec<_> = std::iter::once("d".to_owned())
            .chain((0..20).map(|axis| format!("n{axis}")))
            .collect();
        Self {
            context: CoefficientContext::new(names.iter().map(String::as_str)),
            indices: std::array::from_fn(|axis| axis + 1),
            sector: std::array::from_fn(|axis| fixture.sector.as_bytes()[axis] == b'1'),
        }
    }

    pub(super) fn chart(&self, fixed: [Option<i16>; 20], equalities: &[&str]) -> Case<20> {
        let face = coordinate(fixed);
        if equalities.is_empty() {
            return face;
        }
        face.intersect(
            &equations(&self.context, equalities),
            &self.indices,
            &self.sector,
        )
        .unwrap()
        .expect("captured chart was nonempty at admission")
    }
}

/// (c) The captured B5 T167 p1 refusal: under Defer its intersection succeeds
/// with exactly the captured unresolved branch as the one deferred region.
#[test]
fn t167_p1_b5_refusal_becomes_exactly_the_captured_deferred_region() {
    let fixture = &T167_P1_B5;
    let t = T167::new(fixture);
    let parent = t.chart(fixture.original_fixed, fixture.original_equalities);
    let original = equations(&t.context, fixture.original_conjunction);
    let chart = t.chart(fixture.unresolved_fixed, fixture.unresolved_equalities);
    let unresolved = equations(&t.context, fixture.unresolved_conjunction);
    // Refuse (= this tree's engine, S1/S2 on) reproduces the capture.
    let started = Instant::now();
    let refused = parent
        .intersect_many(&original, &t.indices, &t.sector, Default::default())
        .unwrap_err();
    println!(
        "T167_P1_B5 refuse {:?} {:?}",
        started.elapsed(),
        refused.stats
    );
    assert_eq!(
        refused.failure,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(refused.unresolved_parent, chart);
    assert_eq!(
        refused.unresolved_conjunction.as_ref(),
        unresolved.as_slice()
    );
    // Defer: exactly that region, nothing else deferred.
    let started = Instant::now();
    let result = parent
        .intersect_many_with_policy(
            &original,
            &t.indices,
            &t.sector,
            Default::default(),
            None,
            DEFER,
        )
        .unwrap();
    println!(
        "T167_P1_B5 defer {:?} cases={} resolutions={:?} stats={:?}",
        started.elapsed(),
        result.cases.len(),
        result
            .resolutions
            .iter()
            .map(|resolution| resolution.reason)
            .collect::<Vec<_>>(),
        result.stats
    );
    for case in &result.cases {
        let coupled = case.affine().map_or_else(Vec::new, |affine| {
            affine.equations().iter().map(ToString::to_string).collect()
        });
        println!(
            "T167_P1_B5 case fixed={:?} coupled={coupled:?}",
            case.fixed()
        );
    }
    assert_eq!(
        result.deferred,
        vec![DeferredRegion {
            reason: CaseIntersectionFailure::UnsupportedGeometry,
            parent: parent.clone(),
            chart: chart.clone(),
            conjunction: unresolved.clone().into(),
            exclusions: Arc::from([]),
            dependents: Arc::from([]),
        }]
    );
    let region = &result.deferred[0];
    for case in &result.cases {
        assert!(parent.contains(case).unwrap());
        assert!(
            native::restrict(case, &original, &t.indices)
                .unwrap()
                .is_empty()
        );
    }
    // All ones (the diagnosis' first point) is in the region.
    let ones: [i64; 20] = std::array::from_fn(|axis| i64::from(axis < 10));
    assert!(in_region(region, &t.indices, &ones));
    // Exactness on the box 1..=5 of the six free coordinates n0,n2,n3,n4,n6,n9.
    let free = [0, 2, 3, 4, 6, 9];
    let points = grid([1; 6], [5; 6]).map(|values| {
        let mut point = [0_i64; 20];
        for (axis, value) in parent.fixed().iter().enumerate() {
            point[axis] = value.map_or(0, i64::from);
        }
        for (slot, &axis) in free.iter().enumerate() {
            point[axis] = values[slot];
        }
        point
    });
    let count = check_points(&parent, &original, &result, &t.indices, &t.sector, points);
    println!("T167_P1_B5 box 1..=5 {count:?}");
    assert_eq!(count.sector_points, 5_usize.pow(6));
    assert!(count.only_in_regions > 0);
}

/// Deterministic generator (same recurrence as `lift_tests`).
pub(super) struct Lcg(pub(super) u64);

impl Lcg {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    pub(super) fn range(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % (high - low + 1) as u64) as i64
    }
}

pub(super) const NAMES: [&str; 4] = ["a", "b", "c", "e"];

/// A random integer polynomial of degree <= 2 vanishing at `point`.
pub(super) fn planted(random: &mut Lcg, point: [i64; 4], variables: usize) -> String {
    let mut terms = Vec::new();
    let mut value: i128 = 0;
    for _ in 0..random.range(2, 5) {
        let coefficient = random.range(-6, 6) * [1, 1, 2, 3, 5, 7][random.range(0, 5) as usize];
        let mut monomial = format!("({coefficient})");
        let mut product = i128::from(coefficient);
        for _ in 0..random.range(1, 2) {
            let axis = random.range(0, variables as i64 - 1) as usize;
            monomial.push_str(&format!("*{}", NAMES[axis]));
            product *= i128::from(point[axis]);
        }
        value += product;
        terms.push(monomial);
    }
    format!("{}-({value})", terms.join("+"))
}

pub(super) fn row_through(random: &mut Lcg, point: [i64; 4]) -> Option<String> {
    let row: [i64; 4] = std::array::from_fn(|_| {
        random.range(-2, 2) * [1, 2, 3, 4, 5, 6, 7, 9][random.range(0, 7) as usize]
    });
    if row.iter().all(|&value| value == 0) {
        return None;
    }
    let rhs: i64 = row.iter().zip(point).map(|(a, x)| a * x).sum();
    Some(format!(
        "({})*a+({})*b+({})*c+({})*e-({rhs})",
        row[0], row[1], row[2], row[3]
    ))
}

/// (b) Planted points: under Defer every intersection that used to refuse
/// for an unresolved branch now succeeds, never loses a planted point or any
/// other locus point of the box, never puts a deferred or resolved point off
/// the locus, and never returns UnsupportedGeometry/RepeatedState. The other
/// atomic failures (budgets, resources, admission such as compact overflow)
/// are unchanged by the policy and stay refusals.
#[test]
#[allow(clippy::result_large_err)]
fn defer_never_loses_or_invents_a_point() {
    let context = CoefficientContext::new(["d", "a", "b", "c", "e"]);
    let indices = [1, 2, 3, 4];
    let mut random = Lcg(0x0bad_5eed_c0de);
    let (mut ok, mut deferring, mut regions, mut refused, mut timeouts) = (0, 0, 0, 0, 0);
    let mut planted_only_deferred = 0;
    for iteration in 0..240 {
        let point: [i64; 4] = std::array::from_fn(|_| random.range(-3, 5));
        let sector: [bool; 4] = std::array::from_fn(|axis| point[axis] >= 1);
        let mut rows = Vec::new();
        for _ in 0..random.range(0, 2) {
            rows.extend(row_through(&mut random, point));
        }
        let mut fixed = [None; 4];
        if random.range(0, 2) == 0 {
            let axis = random.range(0, 3) as usize;
            fixed[axis] = Some(point[axis] as i16);
        }
        let row_refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let face = coordinate(fixed);
        let parent = face
            .intersect(&equations(&context, &row_refs), &indices, &sector)
            .unwrap()
            .expect("the chart contains the planted point");
        let variables = random.range(2, 3) as usize;
        let first = planted(&mut random, point, variables);
        let second = planted(&mut random, point, variables);
        let obstruction =
            ["a^2-3*b^2-2", "2*a*b+2*a^2+1", "a^2+b^2-3*c^2-3"][random.range(0, 2) as usize];
        let common = format!("(a-({}))", point[0] + random.range(0, 1));
        let conjunction: Vec<String> = match random.range(0, 3) {
            0 => vec![first],
            1 => vec![format!("({first})*({obstruction})")],
            2 => vec![
                format!("{common}*({first})"),
                format!("{common}*({second})"),
            ],
            _ => vec![first, second],
        };
        let conjunction_refs: Vec<&str> = conjunction.iter().map(String::as_str).collect();
        let conjunction = equations(&context, &conjunction_refs);
        let (sender, receiver) = std::sync::mpsc::channel();
        {
            let (parent, conjunction) = (parent.clone(), conjunction.clone());
            std::thread::spawn(move || {
                let budget = ResourceBudget::new(ResourceLimits {
                    max_process_rss_bytes: None,
                    max_elapsed: Some(Duration::from_secs(10)),
                });
                let result = budget.run(|| {
                    parent.intersect_many_with_policy(
                        &conjunction,
                        &indices,
                        &sector,
                        CaseIntersectionLimits::default().with_diagnostics(),
                        None,
                        DEFER,
                    )
                });
                let _ = sender.send(result);
            });
        }
        let Ok(result) = receiver.recv_timeout(Duration::from_secs(30)) else {
            eprintln!("S4_PLANTED_TIMEOUT iteration {iteration}: {conjunction_refs:?} {rows:?}");
            timeouts += 1;
            continue;
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                assert!(
                    !matches!(
                        error.failure,
                        CaseIntersectionFailure::UnsupportedGeometry
                            | CaseIntersectionFailure::RepeatedState
                            | CaseIntersectionFailure::InvalidInput(_)
                    ),
                    "iteration {iteration}: Defer returned {:?}",
                    error.failure
                );
                refused += 1;
                continue;
            }
        };
        ok += 1;
        deferring += usize::from(!result.deferred.is_empty());
        regions += result.deferred.len();
        for region in &result.deferred {
            assert_eq!(region.parent, parent);
            assert!(parent.contains(&region.chart).unwrap());
            assert!(matches!(
                region.reason,
                CaseIntersectionFailure::UnsupportedGeometry
                    | CaseIntersectionFailure::RepeatedState
            ));
        }
        let in_case = result.cases.iter().any(|case| on_chart(case, &point));
        let in_deferred = result
            .deferred
            .iter()
            .any(|region| in_region(region, &indices, &point));
        assert!(
            in_case || in_deferred,
            "iteration {iteration}: planted point {point:?} lost: {conjunction_refs:?} {rows:?} \
             {result:?}"
        );
        planted_only_deferred += usize::from(in_deferred && !in_case);
        for case in &result.cases {
            assert!(parent.contains(case).unwrap());
            assert!(
                native::restrict(case, &conjunction, &indices)
                    .unwrap()
                    .is_empty()
            );
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
        "S4_PLANTED ok={ok} deferring={deferring} regions={regions} \
         planted_only_deferred={planted_only_deferred} refused={refused} timeouts={timeouts}"
    );
    assert_eq!(ok + refused + timeouts, 240);
    assert!(ok > 0 && deferring > 0 && planted_only_deferred > 0);
}
