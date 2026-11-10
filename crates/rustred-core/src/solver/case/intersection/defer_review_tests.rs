//! Independent soundness review of S4 (opt-in "defer unsupported"), written by the
//! reviewer, not the implementer. Membership is decided here with its own exact
//! evaluator, never through engine geometry:
//! - a point is on a chart iff it satisfies the chart's fixed values and its
//!   primitive integer rows (cross-checked against the defining equations);
//! - a point is in a deferred region iff every polynomial of
//!   `DeferredRegion::predicates` vanishes there (cross-checked against chart rows
//!   plus `conjunction`).
//!
//! Adversarial points: planted ON a nonlinear region, its +-1 neighbours NEXT TO
//! it, and points OFF it that satisfy the restricted conjunction but lie off the
//! chart lattice (the restricted polynomial alone must never admit them).

use symbolica::prelude::Integer;

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::t167_p1_astra_box::T167_P1_ASTRA_BOX;
use super::t167_p1_b5_captured::T167_P1_B5;
use super::{
    CaseIntersectionFailure, CaseIntersectionResult, DeferredRegion, UnsupportedBranchPolicy,
};

fn polys(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

/// Exact integer value at an integer point (i128; test-sized values only).
fn eval<const N: usize>(
    polynomial: &CoefficientPolynomial,
    indices: &[usize; N],
    point: &[i64; N],
) -> i128 {
    let nvars = polynomial.nvars();
    let mut total = 0_i128;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        let mut product = i128::from(coefficient.to_i64().expect("test-sized coefficient"));
        for (variable, &power) in polynomial.exponents[term * nvars..(term + 1) * nvars]
            .iter()
            .enumerate()
        {
            if power != 0 {
                let axis = indices
                    .iter()
                    .position(|&index| index == variable)
                    .expect("predicates contain index variables only");
                product *= i128::from(point[axis]).pow(u32::from(power));
            }
        }
        total += product;
    }
    total
}

fn zero<const N: usize>(
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    point: &[i64; N],
) -> bool {
    conjunction.iter().all(|p| eval(p, indices, point) == 0)
}

/// Chart membership from fixed values and primitive integer rows `[A | b]`,
/// cross-checked against fixed values plus the coupled `equations`.
fn on_chart<const N: usize>(chart: &Case<N>, indices: &[usize; N], point: &[i64; N]) -> bool {
    let fixed = chart
        .fixed()
        .iter()
        .zip(point)
        .all(|(value, &x)| value.is_none_or(|value| i64::from(value) == x));
    let rows = chart.affine().is_none_or(|affine| {
        affine.primitive_matrix().row_iter().all(|row| {
            let entry = |value: &Integer| i128::from(value.to_i64().expect("small row"));
            (0..N)
                .map(|a| entry(&row[a]) * i128::from(point[a]))
                .sum::<i128>()
                == entry(&row[N])
        })
    });
    let equations = chart
        .affine()
        .is_none_or(|affine| zero(affine.equations(), indices, point));
    assert_eq!(
        fixed && rows,
        fixed && equations,
        "chart rows vs equations at {point:?}"
    );
    fixed && rows
}

/// The deferred regions of a result with their predicate lists computed once.
struct Regions<'a, const N: usize> {
    regions: &'a [DeferredRegion<N>],
    predicates: Vec<Vec<CoefficientPolynomial>>,
}

impl<'a, const N: usize> Regions<'a, N> {
    fn new(result: &'a CaseIntersectionResult<N>, indices: &[usize; N]) -> Self {
        Self {
            regions: &result.deferred,
            predicates: result
                .deferred
                .iter()
                .map(|region| {
                    region
                        .predicates(indices)
                        .expect("engine region has predicates")
                })
                .collect(),
        }
    }

    /// Membership through the public predicate list, cross-checked against the
    /// region's chart rows plus its conjunction.
    fn contains(&self, indices: &[usize; N], point: &[i64; N]) -> bool {
        let mut any = false;
        for (region, predicates) in self.regions.iter().zip(&self.predicates) {
            let by_predicates = zero(predicates, indices, point);
            let by_parts = on_chart(&region.chart, indices, point)
                && zero(&region.conjunction, indices, point);
            assert_eq!(
                by_predicates, by_parts,
                "predicates disagree with chart+conjunction at {point:?}"
            );
            any |= by_predicates;
        }
        any
    }
}

fn in_cases<const N: usize>(
    result: &CaseIntersectionResult<N>,
    indices: &[usize; N],
    point: &[i64; N],
) -> bool {
    result
        .cases
        .iter()
        .any(|case| on_chart(case, indices, point))
}

fn neighbours(point: [i64; 3]) -> impl Iterator<Item = [i64; 3]> {
    (0..27).filter(|&k| k != 13).map(move |k| {
        let step = [k % 3 - 1, k / 3 % 3 - 1, k / 9 - 1];
        std::array::from_fn(|a| point[a] + step[a])
    })
}

/// Rational affine charts (dependent coordinate with a non-unit pivot, so the
/// integer points are a proper congruence sublattice) carrying a Pell-type
/// branch next to an affine sibling. The deferred region must be EXACTLY the
/// chart lattice points on the Pell curve, the resolved cases EXACTLY the
/// sibling, on every point of a box that holds three Pell points per chart.
#[test]
fn review_rational_chart_pell_region_is_exact_on_and_off_the_lattice() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let indices = [1, 2, 3];
    let sector = [true; 3];
    struct Chart {
        defining: &'static str,
        on_line: fn([i64; 3]) -> bool,
        pell: fn([i64; 3]) -> bool,
        conjunction: &'static str,
        planted: [[i64; 3]; 3],
        sibling: fn([i64; 3]) -> bool,
    }
    let charts = [
        // b = 2a - 1 (b odd); branch b^2 - 2c^2 = 1 or c = 5.
        Chart {
            defining: "2*a-b-1",
            on_line: |[a, b, _]| 2 * a - b == 1,
            pell: |[_, b, c]| b * b - 2 * c * c == 1,
            conjunction: "(c-5)*(b^2-2*c^2-1)",
            planted: [[2, 3, 2], [9, 17, 12], [50, 99, 70]],
            sibling: |[_, _, c]| c == 5,
        },
        // 3a - 2b = 1 (a odd, b = 3k+1); branch a^2 - 2c^2 = 1 or b = c + 7.
        Chart {
            defining: "3*a-2*b-1",
            on_line: |[a, b, _]| 3 * a - 2 * b == 1,
            pell: |[a, _, c]| a * a - 2 * c * c == 1,
            conjunction: "(b-c-7)*(a^2-2*c^2-1)",
            planted: [[3, 4, 2], [17, 25, 12], [99, 148, 70]],
            sibling: |[_, b, c]| b == c + 7,
        },
    ];
    for chart in &charts {
        let parent = Case::<3>::generic()
            .intersect(&polys(&context, &[chart.defining]), &indices, &sector)
            .unwrap()
            .expect("nonempty chart");
        assert!(
            parent.affine().is_some(),
            "{}: coupled chart expected",
            chart.defining
        );
        let original = polys(&context, &[chart.conjunction]);
        let result = parent
            .intersect_many_with_policy(
                &original,
                &indices,
                &sector,
                Default::default(),
                None,
                UnsupportedBranchPolicy::Defer,
            )
            .unwrap();
        println!(
            "REVIEW_RATIONAL {}: cases {:?} regions {:?}",
            chart.defining,
            result.cases,
            result
                .deferred
                .iter()
                .map(|r| (
                    r.chart.clone(),
                    r.conjunction
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        );
        assert!(
            !result.deferred.is_empty(),
            "{}: the Pell branch must be deferred",
            chart.defining
        );
        // Refuse is still the atomic refusal on the same input.
        assert!(matches!(
            parent
                .intersect_many(&original, &indices, &sector, Default::default())
                .unwrap_err()
                .failure,
            CaseIntersectionFailure::UnsupportedGeometry | CaseIntersectionFailure::RepeatedState
        ));
        for region in &result.deferred {
            assert_eq!(region.parent, parent);
            assert!(parent.contains(&region.chart).unwrap());
            // S1 evidence recomputed, not inferred.
            assert!(region.checks.chart_in_original_parent && region.checks.failure.is_none());
            assert_eq!(region.checks.original_restricts_to_zero, Some(false));
        }
        let regions = Regions::new(&result, &indices);
        // ON: planted Pell points of the lattice are deferred, never a case.
        for point in chart.planted {
            assert!(
                (chart.on_line)(point) && (chart.pell)(point),
                "{point:?} is not a planted point"
            );
            assert!(
                regions.contains(&indices, &point),
                "planted {point:?} not deferred"
            );
            assert!(
                !in_cases(&result, &indices, &point),
                "planted {point:?} in a resolved case"
            );
            // NEXT TO: every +-1 neighbour is deferred iff it is itself a lattice Pell point.
            for next in neighbours(point) {
                let expected = (chart.on_line)(next) && (chart.pell)(next);
                assert_eq!(
                    regions.contains(&indices, &next),
                    expected,
                    "neighbour {next:?}"
                );
            }
        }
        // Exhaustive box: region == lattice AND Pell, cases == lattice AND sibling,
        // and points where only the restricted conjunction vanishes stay outside.
        let (mut region_points, mut case_points, mut off_lattice_conjunction_zeros) = (0, 0, 0);
        let (max_a, max_b, max_c) = (100, 150, 72);
        for a in 1..=max_a {
            for b in 1..=max_b {
                for c in 1..=max_c {
                    let point = [a, b, c];
                    let lattice = (chart.on_line)(point);
                    assert_eq!(on_chart(&parent, &indices, &point), lattice);
                    let region = regions.contains(&indices, &point);
                    let case = in_cases(&result, &indices, &point);
                    assert_eq!(
                        region,
                        lattice && (chart.pell)(point),
                        "region at {point:?}"
                    );
                    assert_eq!(
                        case,
                        lattice && (chart.sibling)(point),
                        "cases at {point:?}"
                    );
                    region_points += usize::from(region);
                    case_points += usize::from(case);
                    off_lattice_conjunction_zeros += usize::from(
                        !lattice
                            && result
                                .deferred
                                .iter()
                                .any(|r| zero(&r.conjunction, &indices, &point)),
                    );
                }
            }
        }
        println!(
            "REVIEW_RATIONAL {}: box region {region_points} cases {case_points} \
             off-lattice conjunction zeros (correctly excluded) {off_lattice_conjunction_zeros}",
            chart.defining
        );
        assert_eq!(region_points, 3);
        assert!(case_points > 0);
        assert!(
            off_lattice_conjunction_zeros > 0,
            "the restricted conjunction must have zeros off the lattice for this test to bite"
        );
    }
}

/// The captured T167 p1 refusal under Defer against Astra's independent exact
/// membership of the flat R* box on that chart (A <= 18, R = 0: every
/// six-positive free tuple with sum <= 14). Region membership must agree with
/// Astra's unresolved-zero flag on all 836 locus points (372 region points incl.
/// both nondegenerate witnesses), no other box point may be in the region, the
/// 464 other locus points must lie in resolved cases, and nothing off the locus
/// may be in a case or region.
#[test]
fn review_t167_p1_region_matches_astra_box_membership() {
    let fixture = &T167_P1_B5;
    let names: Vec<String> = std::iter::once("d".to_owned())
        .chain((0..20).map(|axis| format!("n{axis}")))
        .collect();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let indices: [usize; 20] = std::array::from_fn(|axis| axis + 1);
    let sector: [bool; 20] = std::array::from_fn(|axis| fixture.sector.as_bytes()[axis] == b'1');
    assert!(fixture.original_equalities.is_empty() && fixture.unresolved_equalities.is_empty());
    let parent: Case<20> = CoordinateCase::new(fixture.original_fixed).unwrap().into();
    let original = polys(&context, fixture.original_conjunction);
    let unresolved = polys(&context, fixture.unresolved_conjunction);
    let result = parent
        .intersect_many_with_policy(
            &original,
            &indices,
            &sector,
            Default::default(),
            None,
            UnsupportedBranchPolicy::Defer,
        )
        .unwrap();
    assert_eq!(result.deferred.len(), 1);
    assert_eq!(
        result.deferred[0].checks.original_restricts_to_zero,
        Some(false)
    );
    let regions = Regions::new(&result, &indices);
    let free = [0, 2, 3, 4, 6, 9];
    let lift = |values: [i64; 6]| -> [i64; 20] {
        let mut point = [0_i64; 20];
        for (axis, value) in parent.fixed().iter().enumerate() {
            point[axis] = value.map_or(0, i64::from);
        }
        for (slot, &axis) in free.iter().enumerate() {
            point[axis] = values[slot];
        }
        point
    };
    let astra: std::collections::HashMap<[i64; 6], bool> =
        T167_P1_ASTRA_BOX.iter().copied().collect();
    assert_eq!(
        (astra.len(), astra.values().filter(|&&u| u).count()),
        (836, 372)
    );
    let (mut enumerated, mut locus, mut region_count, mut only_region, mut case_count) =
        (0, 0, 0, 0, 0);
    // All six-positive tuples with sum <= 14 (C(14, 6) = 3003), i.e. the R* box on this chart.
    let mut stack = vec![(Vec::<i64>::new(), 0_i64)];
    while let Some((prefix, sum)) = stack.pop() {
        if prefix.len() == 6 {
            enumerated += 1;
            let values: [i64; 6] = prefix.clone().try_into().unwrap();
            let point = lift(values);
            assert_eq!(
                4 + values.iter().sum::<i64>(),
                point.iter().filter(|&&x| x > 0).sum::<i64>()
            );
            let is_locus = zero(&original, &indices, &point);
            let is_unresolved_zero = zero(&unresolved, &indices, &point);
            let region = regions.contains(&indices, &point);
            let case = in_cases(&result, &indices, &point);
            // Independent data agree with this evaluator.
            assert_eq!(is_locus, astra.contains_key(&values), "locus at {values:?}");
            if let Some(&u) = astra.get(&values) {
                assert_eq!(u, is_unresolved_zero, "Astra U flag at {values:?}");
            }
            assert!(!region || is_locus, "region point {values:?} off the locus");
            assert!(!case || is_locus, "case point {values:?} off the locus");
            assert!(!is_locus || case || region, "locus point {values:?} lost");
            // The region is exactly the captured unresolved branch on this chart.
            assert_eq!(
                region, is_unresolved_zero,
                "region vs Astra U at {values:?}"
            );
            locus += usize::from(is_locus);
            region_count += usize::from(region);
            only_region += usize::from(region && !case);
            case_count += usize::from(case);
            continue;
        }
        for v in 1..=(14 - sum - (5 - prefix.len() as i64)) {
            let mut next = prefix.clone();
            next.push(v);
            stack.push((next, sum + v));
        }
    }
    println!(
        "REVIEW_T167_P1_ASTRA enumerated {enumerated} locus {locus} region {region_count} \
         only_region {only_region} in_cases {case_count}"
    );
    assert_eq!((enumerated, locus, region_count), (3003, 836, 372));
    // Both nondegenerate witnesses (A = 17) are region points.
    for values in [[2, 1, 1, 4, 2, 3], [2, 2, 1, 4, 2, 2]] {
        assert!(regions.contains(&indices, &lift(values)), "{values:?}");
    }
}
