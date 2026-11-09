//! S1 (lift) and S2 (modular integer emptiness) for stuck AND branches.
//!
//! The T167 inputs are exact captured refusals (see `t167_captured_inputs.rs`).
//! Synthetic inputs check soundness: S2 never discards a branch with a real
//! integer point, and S1 never lifts a chart on which the original conjunction
//! does not vanish identically.

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::engine::{original_vanishes_on, resolve_stuck_branch_for_test};
use super::{
    CaseIntersectionFailure, CaseIntersectionLimits, CaseResolutionReason, modular, native,
};

#[path = "t167_captured_inputs.rs"]
mod inputs;
use inputs::{CapturedT167, T167_ORIG, T167_P1_PF0};

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

struct T167 {
    context: CoefficientContext,
    indices: [usize; 20],
    sector: [bool; 20],
}

impl T167 {
    fn new(fixture: &CapturedT167) -> Self {
        let names: Vec<_> = std::iter::once("d".to_owned())
            .chain((0..20).map(|axis| format!("n{axis}")))
            .collect();
        Self {
            context: CoefficientContext::new(names.iter().map(String::as_str)),
            indices: std::array::from_fn(|axis| axis + 1),
            sector: std::array::from_fn(|axis| fixture.sector.as_bytes()[axis] == b'1'),
        }
    }

    fn chart(&self, fixed: [Option<i16>; 20], equalities: &[&str]) -> Case<20> {
        let coordinate: Case<20> = CoordinateCase::new(fixed).unwrap().into();
        if equalities.is_empty() {
            return coordinate;
        }
        coordinate
            .intersect(
                &equations(&self.context, equalities),
                &self.indices,
                &self.sector,
            )
            .unwrap()
            .expect("captured chart was nonempty at admission")
    }

    fn equations(&self, values: &[&str]) -> Vec<CoefficientPolynomial> {
        equations(&self.context, values)
    }

    fn at(
        &self,
        polynomial: &CoefficientPolynomial,
        point: &[(usize, i64)],
    ) -> CoefficientPolynomial {
        let mut value = polynomial.clone();
        for &(axis, coordinate) in point {
            value = value.replace(self.indices[axis], &Integer::from(coordinate));
        }
        value
    }
}

fn restricts_to_zero(
    case: &Case<20>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; 20],
) -> bool {
    native::restrict(case, conjunction, indices)
        .unwrap()
        .is_empty()
}

#[test]
fn t167_original_order_refused_branch_is_lifted_by_s1() {
    let t = T167::new(&T167_ORIG);
    let original_parent = t.chart(T167_ORIG.original_fixed, T167_ORIG.original_equalities);
    let original = t.equations(T167_ORIG.original_conjunction);
    let chart = t.chart(T167_ORIG.unresolved_fixed, T167_ORIG.unresolved_equalities);
    let branch = t.equations(T167_ORIG.unresolved_conjunction);
    assert!(original_parent.contains(&chart).unwrap());
    // The captured branch Q = 0 on L is a nonempty Pell conic: S2 must not
    // (and cannot) discard it; the lift is the exact step.
    assert_eq!(
        modular::integer_empty_prime(&chart, &branch, &t.indices).unwrap(),
        None
    );
    assert!(original_vanishes_on(&original_parent, &original, &chart, &t.indices).unwrap());
    let (resolved, resolutions, stats) = resolve_stuck_branch_for_test(
        &original_parent,
        &original,
        &chart,
        &branch,
        &t.indices,
        &t.sector,
        CaseIntersectionFailure::UnsupportedGeometry,
    )
    .unwrap();
    assert_eq!(resolved, vec![chart.clone()]);
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0].reason, CaseResolutionReason::Lifted);
    assert_eq!(
        resolutions[0].replaced,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(resolutions[0].chart, chart);
    assert_eq!(stats.lifted_branches, 1);
    assert_eq!(stats.modular_empty_branches, 0);
    // Every point of the lifted chart is exceptional for the original rule.
    assert!(restricts_to_zero(&chart, &original, &t.indices));
}

#[test]
fn t167_original_order_branch_alone_is_neither_lifted_nor_discarded() {
    // The same branch as its own original conjunction: Q does not vanish on L,
    // and Q = 0 has the integer point (n0, n1, n2) = (927, 2060, 723) on L.
    let t = T167::new(&T167_ORIG);
    let chart = t.chart(T167_ORIG.unresolved_fixed, T167_ORIG.unresolved_equalities);
    let branch = t.equations(T167_ORIG.unresolved_conjunction);
    let point = [(0, 927), (1, 2060), (2, 723)];
    assert!(t.at(&branch[0], &point).is_zero());
    let chart_equation = t.equations(T167_ORIG.unresolved_equalities);
    assert!(t.at(&chart_equation[0], &point).is_zero());
    assert!(!original_vanishes_on(&chart, &branch, &chart, &t.indices).unwrap());
    let error = chart
        .intersect_many(
            &branch,
            &t.indices,
            &t.sector,
            CaseIntersectionLimits::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.stats.lifted_branches, 0);
    assert_eq!(error.stats.modular_empty_branches, 0);
}

#[test]
fn t167_p1_refused_branch_is_integer_empty_modulo_two() {
    let t = T167::new(&T167_P1_PF0);
    let original_parent = t.chart(T167_P1_PF0.original_fixed, T167_P1_PF0.original_equalities);
    let original = t.equations(T167_P1_PF0.original_conjunction);
    let chart = t.chart(
        T167_P1_PF0.unresolved_fixed,
        T167_P1_PF0.unresolved_equalities,
    );
    let branch = t.equations(T167_P1_PF0.unresolved_conjunction);
    assert!(original_parent.contains(&chart).unwrap());
    assert!(!original_vanishes_on(&original_parent, &original, &chart, &t.indices).unwrap());
    // The parity needs the chart lattice: the conjunct alone has zeros mod 2.
    let generic = Case::<20>::generic();
    assert_eq!(
        modular::integer_empty_prime(&generic, &branch, &t.indices).unwrap(),
        None
    );
    assert_eq!(
        modular::integer_empty_prime(&chart, &branch, &t.indices).unwrap(),
        Some(2)
    );
    let (resolved, resolutions, stats) = resolve_stuck_branch_for_test(
        &original_parent,
        &original,
        &chart,
        &branch,
        &t.indices,
        &t.sector,
        CaseIntersectionFailure::UnsupportedGeometry,
    )
    .unwrap();
    assert!(resolved.is_empty());
    assert_eq!(resolutions.len(), 1);
    assert_eq!(
        resolutions[0].reason,
        CaseResolutionReason::ModularEmpty { prime: 2 }
    );
    assert_eq!(stats.modular_empty_branches, 1);
    assert_eq!(stats.lifted_branches, 0);
}

#[test]
fn t167_p1_complete_intersection_now_succeeds() {
    let t = T167::new(&T167_P1_PF0);
    let parent = t.chart(T167_P1_PF0.original_fixed, T167_P1_PF0.original_equalities);
    let conjunction = t.equations(T167_P1_PF0.original_conjunction);
    let result = parent
        .intersect_many(
            &conjunction,
            &t.indices,
            &t.sector,
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    println!(
        "T167_P1 cases={} resolutions={:?} stats={:?}",
        result.cases.len(),
        result
            .resolutions
            .iter()
            .map(|resolution| resolution.reason)
            .collect::<Vec<_>>(),
        result.stats
    );
    assert!(
        result
            .resolutions
            .iter()
            .any(|resolution| resolution.reason == CaseResolutionReason::ModularEmpty { prime: 2 })
    );
    assert_eq!(
        result.stats.modular_empty_branches + result.stats.lifted_branches,
        result.resolutions.len()
    );
    for case in &result.cases {
        assert!(parent.contains(case).unwrap());
        assert!(restricts_to_zero(case, &conjunction, &t.indices));
    }
}

/// About 40 s of exact native normalization in a release build (captured
/// stats: 39 s); run with `--release -- --ignored`.
#[test]
#[ignore = "release only: the captured T167 conjunction needs ~40 s of exact F4"]
fn t167_original_order_complete_intersection_now_succeeds() {
    let t = T167::new(&T167_ORIG);
    let parent = t.chart(T167_ORIG.original_fixed, T167_ORIG.original_equalities);
    let conjunction = t.equations(T167_ORIG.original_conjunction);
    let result = parent
        .intersect_many(
            &conjunction,
            &t.indices,
            &t.sector,
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    println!(
        "T167_ORIG cases={} resolutions={:?} stats={:?}",
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
            "T167_ORIG case fixed={:?} coupled={coupled:?}",
            case.fixed()
        );
    }
    assert!(
        result
            .resolutions
            .iter()
            .any(|resolution| resolution.reason == CaseResolutionReason::Lifted)
    );
    // Exact soundness: every case lies in the parent and in the locus.
    for case in &result.cases {
        assert!(parent.contains(case).unwrap());
        assert!(restricts_to_zero(case, &conjunction, &t.indices));
    }
    // The five affine faces of the diagnosis are each covered.
    for face in [
        ["n4-1", "n5-1"],
        ["n4-1", "n5-n6"],
        ["n2-1", "n4-1"],
        ["n6-1", "n4-n5"],
        ["n2-1", "n5-n6"],
    ] {
        let face = parent
            .intersect(&t.equations(&face), &t.indices, &t.sector)
            .unwrap()
            .unwrap();
        assert!(restricts_to_zero(&face, &conjunction, &t.indices));
        assert!(
            result
                .cases
                .iter()
                .any(|case| case.contains(&face).unwrap()),
            "face {face:?} not covered"
        );
    }
}

/// A small deterministic generator for the soundness property.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn range(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % (high - low + 1) as u64) as i64
    }
}

/// A random integer polynomial in a, b, c, e that vanishes at `point`.
fn planted(random: &mut Lcg, point: [i64; 4]) -> String {
    const NAMES: [&str; 4] = ["a", "b", "c", "e"];
    let mut terms = Vec::new();
    let mut value: i128 = 0;
    for _ in 0..random.range(2, 6) {
        let coefficient = random.range(-9, 9);
        let mut monomial = format!("({coefficient})");
        let mut product = i128::from(coefficient);
        for _ in 0..random.range(0, 3) {
            let axis = random.range(0, 3) as usize;
            monomial.push_str(&format!("*{}", NAMES[axis]));
            product *= i128::from(point[axis]);
        }
        value += product;
        terms.push(monomial);
    }
    format!("{}-({value})", terms.join("+"))
}

#[test]
fn s2_never_discards_a_branch_with_a_planted_integer_point() {
    let context = CoefficientContext::new(["d", "a", "b", "c", "e"]);
    let indices = [1, 2, 3, 4];
    let mut random = Lcg(0x5eed_1677);
    let mut charts_tested = 0;
    let mut rational_charts = 0;
    for _ in 0..400 {
        let point: [i64; 4] = std::array::from_fn(|_| random.range(-4, 6));
        let sector: [bool; 4] = std::array::from_fn(|axis| point[axis] >= 1);
        let conjunction = equations(
            &context,
            &[&planted(&mut random, point), &planted(&mut random, point)],
        );
        // Charts through the point: generic, one fixed coordinate, and an
        // affine row with small (often even or 3-divisible) coefficients.
        let mut charts = vec![Case::<4>::generic()];
        let axis = random.range(0, 3) as usize;
        let mut fixed = [None; 4];
        fixed[axis] = Some(point[axis] as i16);
        charts.push(CoordinateCase::new(fixed).unwrap().into());
        let row: [i64; 4] = std::array::from_fn(|_| random.range(-3, 3) * random.range(1, 3));
        if row.iter().any(|&value| value != 0) {
            let rhs: i64 = row.iter().zip(point).map(|(a, x)| a * x).sum();
            let equality = format!(
                "({})*a+({})*b+({})*c+({})*e-({rhs})",
                row[0], row[1], row[2], row[3]
            );
            let chart = Case::<4>::generic()
                .intersect(&equations(&context, &[&equality]), &indices, &sector)
                .unwrap()
                .expect("the chart contains the planted point");
            rational_charts += usize::from(
                chart
                    .affine()
                    .is_some_and(|affine| !affine.has_integral_chart()),
            );
            charts.push(chart);
        }
        for chart in &charts {
            charts_tested += 1;
            assert_eq!(
                modular::integer_empty_prime(chart, &conjunction, &indices).unwrap(),
                None,
                "planted point {point:?} on {chart:?} for {conjunction:?}"
            );
            let restricted = native::restrict(chart, &conjunction, &indices).unwrap();
            if !restricted.is_empty() {
                assert_eq!(
                    modular::integer_empty_prime(chart, &restricted, &indices).unwrap(),
                    None,
                    "planted point {point:?} on {chart:?} for restricted {restricted:?}"
                );
            }
        }
    }
    assert!(charts_tested > 1000);
    assert!(rational_charts > 0);
}

#[test]
fn s2_proves_parity_content_and_lattice_emptiness() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    // Content: odd at every integer point.
    assert_eq!(
        modular::integer_empty_prime(&generic, &equations(&context, &["2*x*y+2*x^2+1"]), &indices)
            .unwrap(),
        Some(2)
    );
    // x^2 = 3 y^2 + 2 has no solution modulo 3.
    assert_eq!(
        modular::integer_empty_prime(&generic, &equations(&context, &["x^2-3*y^2-2"]), &indices)
            .unwrap(),
        Some(3)
    );
    // Only the rational chart 2z = 3(x-y) forces x = y modulo 2.
    let conjunction = equations(&context, &["(x-y)*(x+y+z)-1"]);
    assert_eq!(
        modular::integer_empty_prime(&generic, &conjunction, &indices).unwrap(),
        None
    );
    let chart = generic
        .intersect(&equations(&context, &["2*z-3*x+3*y"]), &indices, &sector)
        .unwrap()
        .unwrap();
    assert!(!chart.affine().unwrap().has_integral_chart());
    assert_eq!(modular::residue_space_dimension(&chart, 2), Some(2));
    assert_eq!(
        modular::integer_empty_prime(&chart, &conjunction, &indices).unwrap(),
        Some(2)
    );
    let restricted = native::restrict(&chart, &conjunction, &indices).unwrap();
    assert_eq!(
        modular::integer_empty_prime(&chart, &restricted, &indices).unwrap(),
        Some(2)
    );
    // A conjunct with a non-index parameter is never used.
    let context = CoefficientContext::new(["d", "x"]);
    assert_eq!(
        modular::integer_empty_prime(
            &Case::<1>::generic(),
            &equations(&context, &["2*d*x+1"]),
            &[1]
        )
        .unwrap(),
        None
    );
}

#[test]
fn s2_resolves_a_stuck_modular_empty_conic_end_to_end() {
    let context = CoefficientContext::new(["x", "y"]);
    let conjunction = equations(&context, &["x^2-3*y^2-2"]);
    let result = Case::<2>::generic()
        .intersect_many(
            &conjunction,
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert!(result.cases.is_empty());
    assert_eq!(result.resolutions.len(), 1);
    assert_eq!(
        result.resolutions[0].reason,
        CaseResolutionReason::ModularEmpty { prime: 3 }
    );
    assert_eq!(
        result.resolutions[0].replaced,
        CaseIntersectionFailure::UnsupportedGeometry
    );
    assert_eq!(result.stats.modular_empty_branches, 1);
}

#[test]
fn pell_branches_with_integer_points_are_still_refused() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for input in [&["x^2-2*y^2-1"][..], &["(z-1)*(x^2-2*y^2-1)"][..]] {
        let conjunction = equations(&context, input);
        let error = Case::<3>::generic()
            .intersect_many(
                &conjunction,
                &[0, 1, 2],
                &[true; 3],
                CaseIntersectionLimits::default(),
            )
            .unwrap_err();
        assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
        assert_eq!(error.stats.lifted_branches, 0);
        assert_eq!(error.stats.modular_empty_branches, 0);
    }
}

#[test]
fn s1_never_lifts_a_chart_with_a_nonvanishing_restriction() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let sector = [true; 3];
    let generic = Case::<3>::generic();
    let original = equations(&context, &["(x-1)*(y^2-2*z^2-1)", "(x-1)*(x-2)"]);
    let on = |fixed: [Option<i16>; 3]| -> Case<3> { CoordinateCase::new(fixed).unwrap().into() };
    // x = 1 lies in the locus; x = 2 and the generic chart do not.
    assert!(
        original_vanishes_on(&generic, &original, &on([Some(1), None, None]), &indices).unwrap()
    );
    assert!(
        !original_vanishes_on(&generic, &original, &on([Some(2), None, None]), &indices).unwrap()
    );
    assert!(!original_vanishes_on(&generic, &original, &generic, &indices).unwrap());
    // A chart outside the original parent is never lifted.
    let parent = on([None, Some(3), None]);
    assert!(
        !original_vanishes_on(&parent, &original, &on([Some(1), None, None]), &indices).unwrap()
    );
    // The engine seam: x = 2 with the Pell branch keeps its refusal.
    let branch = equations(&context, &["y^2-2*z^2-1"]);
    let refused = resolve_stuck_branch_for_test(
        &generic,
        &original,
        &on([Some(2), None, None]),
        &branch,
        &indices,
        &sector,
        CaseIntersectionFailure::UnsupportedGeometry,
    )
    .unwrap_err();
    assert_eq!(refused, CaseIntersectionFailure::UnsupportedGeometry);
    let refused = resolve_stuck_branch_for_test(
        &generic,
        &original,
        &on([Some(2), None, None]),
        &branch,
        &indices,
        &sector,
        CaseIntersectionFailure::RepeatedState,
    )
    .unwrap_err();
    assert_eq!(refused, CaseIntersectionFailure::RepeatedState);
    // x = 1 with the same branch is redundant and lifted.
    let (resolved, resolutions, _) = resolve_stuck_branch_for_test(
        &generic,
        &original,
        &on([Some(1), None, None]),
        &branch,
        &indices,
        &sector,
        CaseIntersectionFailure::RepeatedState,
    )
    .unwrap();
    assert_eq!(resolved, vec![on([Some(1), None, None])]);
    assert_eq!(resolutions[0].reason, CaseResolutionReason::Lifted);
    assert_eq!(
        resolutions[0].replaced,
        CaseIntersectionFailure::RepeatedState
    );
}

#[test]
fn s1_end_to_end_keeps_the_union_exact() {
    // Whatever path the engine takes, a successful union must lie in the locus.
    let context = CoefficientContext::new(["x", "y", "z"]);
    let indices = [0, 1, 2];
    let original = equations(&context, &["(x-1)*(y^2-2*z^2-1)", "(x-1)*(x^2+1)"]);
    let result = Case::<3>::generic().intersect_many(
        &original,
        &indices,
        &[true; 3],
        CaseIntersectionLimits::default(),
    );
    println!("S1_SYNTHETIC {result:?}");
    let result = result.unwrap();
    let x_is_one: Case<3> = CoordinateCase::new([Some(1), None, None]).unwrap().into();
    assert_eq!(result.cases, vec![x_is_one]);
    for case in &result.cases {
        assert!(
            native::restrict(case, &original, &indices)
                .unwrap()
                .is_empty()
        );
    }
}
