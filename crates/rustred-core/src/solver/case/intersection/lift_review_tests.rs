//! Independent soundness review of S1 (lift) and S2 (modular emptiness):
//! adversarial inputs. S2 may be incomplete, never unsound: a branch with an
//! integer point on its chart is never reported modular-empty, whatever the
//! lattice index of the chart or the denominators cleared by a rational
//! chart. S1 lifts only charts inside the original parent on which the
//! ORIGINAL conjunction restricts to the zero polynomial.

use std::time::{Duration, Instant};

use symbolica::prelude::Integer;
use symbolica::utils::resources::{ResourceBudget, ResourceLimits};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};

use super::super::{Case, CoordinateCase};
use super::engine::{original_vanishes_on, resolve_stuck_branch_for_test};
use super::{
    CaseIntersectionFailure, CaseIntersectionLimits, CaseResolutionReason, modular, native,
};

use super::t167_captured_inputs::T167_ORIG;

fn equations(context: &CoefficientContext, values: &[&str]) -> Vec<CoefficientPolynomial> {
    values
        .iter()
        .map(|value| context.coefficient_fixture(value).numerator)
        .collect()
}

fn chart<const N: usize>(
    context: &CoefficientContext,
    indices: &[usize; N],
    sector: &[bool; N],
    fixed: [Option<i16>; N],
    rows: &[&str],
) -> Case<N> {
    let face: Case<N> = CoordinateCase::new(fixed).unwrap().into();
    if rows.is_empty() {
        return face;
    }
    face.intersect(&equations(context, rows), indices, sector)
        .unwrap()
        .expect("the chart is nonempty")
}

fn point_case<const N: usize>(point: [i64; N]) -> Case<N> {
    CoordinateCase::new(point.map(|value| Some(i16::try_from(value).unwrap())))
        .unwrap()
        .into()
}

/// Exact membership of an integer point of any size (compact fixed values
/// stop near 64) in a chart: its fixed coordinates and every primitive row
/// `A n = b`, evaluated independently of S2.
fn on_chart<const N: usize>(chart: &Case<N>, point: [i64; N]) -> bool {
    let fixed = chart
        .fixed()
        .iter()
        .zip(point)
        .all(|(value, coordinate)| value.is_none_or(|value| i64::from(value) == coordinate));
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

fn at<const N: usize>(
    polynomial: &CoefficientPolynomial,
    indices: &[usize; N],
    point: [i64; N],
) -> CoefficientPolynomial {
    let mut value = polynomial.clone();
    for (axis, coordinate) in point.into_iter().enumerate() {
        value = value.replace(indices[axis], &Integer::from(coordinate));
    }
    value
}

/// S2 on the raw conjunction and on its restriction to the chart (the form the
/// engine holds), as `(raw, restricted)`.
fn s2<const N: usize>(
    chart: &Case<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> (Option<u32>, Option<u32>) {
    let raw = modular::integer_empty_prime(chart, conjunction, indices).unwrap();
    let restricted = native::restrict(chart, conjunction, indices).unwrap();
    let restricted = modular::integer_empty_prime(chart, &restricted, indices).unwrap();
    (raw, restricted)
}

#[test]
fn review_s2_lattice_index_divisible_by_p() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let indices = [1, 2, 3];
    let sector = [true; 3];
    let free = [None; 3];
    // x = 2y (index 2): x^2 = 1 has no point with x even; x^2 = 4 has (2, 1, z).
    let even = chart(&context, &indices, &sector, free, &["x-2*y"]);
    assert_eq!(
        s2(&even, &equations(&context, &["x^2-1"]), &indices),
        (Some(2), Some(2))
    );
    assert_eq!(
        s2(&even, &equations(&context, &["x^2-4"]), &indices),
        (None, None)
    );
    // x = 4y (index 4): mod 2 sees only x even, never more; x^2 = 16 has (4, 1, z).
    let four = chart(&context, &indices, &sector, free, &["x-4*y"]);
    assert_eq!(
        s2(&four, &equations(&context, &["x^2-16"]), &indices),
        (None, None)
    );
    // 4y = 2 + 4z is empty; only the restricted (content-free) form shows it.
    assert_eq!(
        s2(&four, &equations(&context, &["x-2-4*z"]), &indices),
        (None, Some(2))
    );
    // 3x = 5y (rational chart, 3 | y and 5 | x): y = 1 + 3z is empty, xy = 15 is not.
    let three_five = chart(&context, &indices, &sector, free, &["3*x-5*y"]);
    assert_eq!(
        s2(&three_five, &equations(&context, &["y-1-3*z"]), &indices),
        (Some(3), Some(3))
    );
    for nonempty in [
        &["x*y-15"][..],
        &["x^2+y^2-34"][..],
        &["x-5-15*z", "y-3-9*z"][..],
    ] {
        assert_eq!(
            s2(&three_five, &equations(&context, nonempty), &indices),
            (None, None),
            "{nonempty:?}"
        );
    }
    // 7x = 2y (7 | y): y = 3 + 7z is empty, y^2 = 49 has (2, 7, z).
    let seven = chart(&context, &indices, &sector, free, &["7*x-2*y"]);
    assert_eq!(
        s2(&seven, &equations(&context, &["y-3-7*z"]), &indices),
        (Some(7), Some(7))
    );
    assert_eq!(
        s2(&seven, &equations(&context, &["y^2-49"]), &indices),
        (None, None)
    );
    // x = y = z: the two rows are dependent modulo 2 (superset residue space).
    let diagonal = chart(&context, &indices, &sector, free, &["x+y-2*z", "x-y"]);
    assert_eq!(
        s2(&diagonal, &equations(&context, &["x*y-4"]), &indices),
        (None, None)
    );
    assert_eq!(
        s2(&diagonal, &equations(&context, &["x*z-y-2"]), &indices),
        (None, None)
    );
    // A fixed coordinate congruent to zero modulo p, with a coupled row.
    let fixed = chart(
        &context,
        &indices,
        &sector,
        [None, None, Some(6)],
        &["2*x-3*y"],
    );
    assert_eq!(
        s2(&fixed, &equations(&context, &["x*y-z"]), &indices),
        (None, None)
    );
}

#[test]
fn review_s2_denominators_never_discard_a_point() {
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let indices = [1, 2, 3];
    let sector = [true; 3];
    let free = [None; 3];
    // Equations are integer numerators: (x^2-x)/2 = 0 is x^2 - x = 0 (x = 0).
    let generic = Case::<3>::generic();
    for nonempty in ["(x^2-x)/2", "(x^2-x)/2-3", "(x^2-x)/6-1"] {
        assert_eq!(
            s2(&generic, &equations(&context, &[nonempty]), &indices),
            (None, None),
            "{nonempty}"
        );
    }
    // (x^2 - x)/2 + 1 > 0 at every integer point: proved at 3, never at 2.
    assert_eq!(
        s2(&generic, &equations(&context, &["(x^2-x)/2+1"]), &indices),
        (Some(3), Some(3))
    );
    // x = y/2: restriction clears 1/4 (x^2 - 1 -> y^2 - 4); points (1, 2), (0, 0).
    let half = chart(&context, &indices, &sector, free, &["2*x-y"]);
    assert!(!half.affine().unwrap().has_integral_chart());
    for nonempty in [&["x^2-1"][..], &["x^2-x"][..], &["x^2+x*z-2*z"][..]] {
        assert_eq!(
            s2(&half, &equations(&context, nonempty), &indices),
            (None, None)
        );
    }
    assert_eq!(
        s2(&half, &equations(&context, &["2*x-1"]), &indices),
        (Some(2), Some(2))
    );
    // x = y/5: restriction clears 1/25 (x^2 + x - 2 -> y^2 + 5y - 50, 5 | all
    // but the leading term); the points x = 1, -2 survive modulo 5.
    let fifth = chart(&context, &indices, &sector, free, &["5*x-y"]);
    assert_eq!(
        s2(&fifth, &equations(&context, &["x^2+x-2"]), &indices),
        (None, None)
    );
    // x = 3y/2: x^2 = 1 forces y = 2/3 (empty, proved at 3 not 2); x^2 = 9 has (3, 2).
    let three_halves = chart(&context, &indices, &sector, free, &["2*x-3*y"]);
    assert_eq!(
        s2(&three_halves, &equations(&context, &["x^2-1"]), &indices),
        (Some(3), Some(3))
    );
    assert_eq!(
        s2(&three_halves, &equations(&context, &["x^2-9"]), &indices),
        (None, None)
    );
    // 7x = 6y + 7z (7 | y): xy = z + 48 has (7, 7, 1).
    let sevenths = chart(&context, &indices, &sector, free, &["7*x-6*y-7*z"]);
    assert_eq!(
        s2(&sevenths, &equations(&context, &["x*y-z-48"]), &indices),
        (None, None)
    );
}

#[test]
fn review_s2_pell_point_charts_are_never_discarded() {
    // T167 orig refused branch: Q = 0 on L has (n0, n1, n2) = (927, 2060, 723).
    let names: Vec<_> = std::iter::once("d".to_owned())
        .chain((0..20).map(|axis| format!("n{axis}")))
        .collect();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let indices: [usize; 20] = std::array::from_fn(|axis| axis + 1);
    let sector: [bool; 20] = std::array::from_fn(|axis| T167_ORIG.sector.as_bytes()[axis] == b'1');
    let branch = equations(&context, T167_ORIG.unresolved_conjunction);
    let mut point = [0_i64; 20];
    for (axis, value) in T167_ORIG.unresolved_fixed.iter().enumerate() {
        if let Some(value) = value {
            point[axis] = i64::from(*value);
        }
    }
    point[0] = 927;
    point[1] = 2060;
    point[2] = 723;
    assert!(at(&branch[0], &indices, point).is_zero());
    let base = chart(
        &context,
        &indices,
        &sector,
        T167_ORIG.unresolved_fixed,
        T167_ORIG.unresolved_equalities,
    );
    assert!(on_chart(&base, point));
    // Sub-charts through the far point with a lattice index divisible by
    // 2, 3, 5, 7 (rational charts included).
    let mut charts = vec![base.clone()];
    for row in [
        "2*n0-n2-1131",
        "3*n2-n1-109",
        "5*n0-2*n1-515",
        "7*n2-n0-4134",
        "4*n0-6*n2-(4*927-6*723)",
    ] {
        let mut rows = T167_ORIG.unresolved_equalities.to_vec();
        rows.push(row);
        charts.push(chart(
            &context,
            &indices,
            &sector,
            T167_ORIG.unresolved_fixed,
            &rows,
        ));
    }
    for chart in &charts {
        assert!(on_chart(chart, point), "{chart:?}");
        assert_eq!(s2(chart, &branch, &indices), (None, None), "{chart:?}");
        // As its own original conjunction (no lift), the stuck branch keeps its refusal.
        for failure in [
            CaseIntersectionFailure::UnsupportedGeometry,
            CaseIntersectionFailure::RepeatedState,
        ] {
            let refused = resolve_stuck_branch_for_test(
                chart,
                &branch,
                chart,
                &branch,
                &indices,
                &sector,
                failure.clone(),
            )
            .unwrap_err();
            assert_eq!(refused, failure);
        }
    }
    // The classic Pell conic x^2 - 2y^2 = 1 through (577, 408) on x = 2z - 1.
    let context = CoefficientContext::new(["d", "x", "y", "z"]);
    let indices = [1, 2, 3];
    let pell = equations(&context, &["x^2-2*y^2-1"]);
    for rows in [
        &["x-2*z+1"][..],
        &["2*x-3*z+(3*385-2*577)"][..],
        &["x-y-169"][..],
    ] {
        let chart = chart(&context, &indices, &[true; 3], [None; 3], rows);
        assert_eq!(s2(&chart, &pell, &indices), (None, None), "{rows:?}");
    }
}

#[test]
fn review_s1_lifts_only_identically_vanishing_original_conjunctions() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let indices = [1, 2];
    let sector = [true; 2];
    let generic = Case::<2>::generic();
    let on = |fixed: [Option<i16>; 2]| -> Case<2> { CoordinateCase::new(fixed).unwrap().into() };
    // A non-index parameter is not zero: d*x - 1 restricts to d - 1 on x = 1.
    let original = equations(&context, &["d*x-1", "(x-1)*(y^2-3)"]);
    assert!(!original_vanishes_on(&generic, &original, &on([Some(1), None]), &indices).unwrap());
    let original = equations(&context, &["d*(x-1)", "(x-1)*(y^2-3)"]);
    assert!(original_vanishes_on(&generic, &original, &on([Some(1), None]), &indices).unwrap());
    // x = 1 is not inside the parent x = 2y (no integer point there): never lifted.
    let parent = chart(&context, &indices, &sector, [None; 2], &["x-2*y"]);
    let original = equations(&context, &["(x-1)*(y^2-3)"]);
    assert!(!original_vanishes_on(&parent, &original, &on([Some(1), None]), &indices).unwrap());
    // Its branch y^2 = 4 has integer points: the refusal stays.
    let refused = resolve_stuck_branch_for_test(
        &parent,
        &original,
        &on([Some(1), None]),
        &equations(&context, &["y^2-4"]),
        &indices,
        &sector,
        CaseIntersectionFailure::UnsupportedGeometry,
    )
    .unwrap_err();
    assert_eq!(refused, CaseIntersectionFailure::UnsupportedGeometry);
    // Its branch y^2 = 3 is integer-empty: dropped by S2 (prime 5), not lifted.
    let (resolved, resolutions, stats) = resolve_stuck_branch_for_test(
        &parent,
        &original,
        &on([Some(1), None]),
        &equations(&context, &["y^2-3"]),
        &indices,
        &sector,
        CaseIntersectionFailure::UnsupportedGeometry,
    )
    .unwrap();
    assert!(resolved.is_empty());
    assert_eq!(stats.lifted_branches, 0);
    assert_eq!(
        resolutions[0].reason,
        CaseResolutionReason::ModularEmpty { prime: 5 }
    );
    // Vanishing modulo p or at sample points is not identical vanishing.
    let original = equations(&context, &["2*(x-1)*y", "(x-1)*(x-2)*y"]);
    assert!(!original_vanishes_on(&generic, &original, &on([None, Some(1)]), &indices).unwrap());
    assert!(!original_vanishes_on(&generic, &original, &on([Some(2), None]), &indices).unwrap());
    assert!(original_vanishes_on(&generic, &original, &on([Some(1), None]), &indices).unwrap());
    assert!(original_vanishes_on(&generic, &original, &on([None, Some(0)]), &indices).unwrap());
}

/// Deterministic generator (same recurrence as `lift_tests`).
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

const NAMES: [&str; 4] = ["a", "b", "c", "e"];

/// A random integer polynomial of degree <= 2 vanishing at `point`.
fn planted(random: &mut Lcg, point: [i64; 4], variables: usize) -> String {
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

/// A chart row through `point` whose coefficients are often divisible by p.
fn row_through(random: &mut Lcg, point: [i64; 4]) -> Option<String> {
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

#[test]
#[allow(clippy::result_large_err)]
fn review_engine_never_loses_a_planted_point() {
    let context = CoefficientContext::new(["d", "a", "b", "c", "e"]);
    let indices = [1, 2, 3, 4];
    let mut random = Lcg(0x0bad_5eed_c0de);
    let (mut ok, mut refused, mut lifted, mut modular_empty, mut s2_direct) = (0, 0, 0, 0, 0);
    let mut timeouts = 0;
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
        let face: Case<4> = CoordinateCase::new(fixed).unwrap().into();
        let Some(parent) = face
            .intersect(&equations(&context, &row_refs), &indices, &sector)
            .unwrap()
        else {
            panic!("chart through the planted point {point:?} proved empty: {rows:?}");
        };
        let point_case = point_case(point);
        assert!(parent.contains(&point_case).unwrap());
        // Shapes that reach stuck branches: a planted conic, a planted conic
        // times a modular-empty factor, a common affine factor (S1 territory).
        let variables = random.range(2, 3) as usize;
        let first = planted(&mut random, point, variables);
        let second = planted(&mut random, point, variables);
        let obstruction =
            ["a^2-3*b^2-2", "2*a*b+2*a^2+1", "a^2+b^2-3*c^2-3"][random.range(0, 2) as usize];
        let common = format!("(a-({}))", point[0] + random.range(0, 1));
        let shape = random.range(0, 3);
        let conjunction: Vec<String> = match shape {
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
        // S2 directly: the planted point is a common zero on the chart.
        assert_eq!(
            s2(&parent, &conjunction, &indices),
            (None, None),
            "{conjunction_refs:?}"
        );
        s2_direct += 1;
        // Exact refusals are allowed. Random multi-equation conjunctions can
        // reach a long native Lex F4 (pre-existing, not S1/S2): run each
        // intersection with the diagnostic work limits and an elapsed budget
        // in a watchdog thread, and leave a hung one behind as a timeout.
        let (sender, receiver) = std::sync::mpsc::channel();
        {
            let (parent, conjunction) = (parent.clone(), conjunction.clone());
            std::thread::spawn(move || {
                let budget = ResourceBudget::new(ResourceLimits {
                    max_process_rss_bytes: None,
                    max_elapsed: Some(Duration::from_secs(10)),
                });
                let result = budget.run(|| {
                    parent.intersect_many(
                        &conjunction,
                        &indices,
                        &sector,
                        CaseIntersectionLimits::default().with_diagnostics(),
                    )
                });
                let _ = sender.send(result);
            });
        }
        let started = Instant::now();
        let Ok(result) = receiver.recv_timeout(Duration::from_secs(30)) else {
            eprintln!(
                "REVIEW_PLANTED_TIMEOUT iteration {iteration}: {conjunction_refs:?} rows {rows:?}"
            );
            timeouts += 1;
            continue;
        };
        if started.elapsed() > Duration::from_secs(2) {
            eprintln!(
                "REVIEW_PLANTED_SLOW iteration {iteration} {:?}: {conjunction_refs:?}",
                started.elapsed()
            );
        }
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                assert!(
                    !matches!(error.failure, CaseIntersectionFailure::InvalidInput(_)),
                    "{error}"
                );
                refused += 1;
                continue;
            }
        };
        ok += 1;
        for resolution in &result.resolutions {
            match resolution.reason {
                CaseResolutionReason::Lifted => {
                    lifted += 1;
                    assert!(parent.contains(&resolution.chart).unwrap());
                    assert!(
                        native::restrict(&resolution.chart, &conjunction, &indices)
                            .unwrap()
                            .is_empty()
                    );
                }
                CaseResolutionReason::ModularEmpty { .. } => {
                    modular_empty += 1;
                    if resolution.chart.contains(&point_case).unwrap() {
                        assert!(
                            resolution
                                .conjunction
                                .iter()
                                .any(|equation| !at(equation, &indices, point).is_zero()),
                            "iteration {iteration}: modular-empty branch contains {point:?}: {resolution:?}"
                        );
                    }
                }
            }
        }
        assert!(
            result
                .cases
                .iter()
                .any(|case| case.contains(&point_case).unwrap()),
            "iteration {iteration}: planted point {point:?} lost; rows {rows:?}, conjunction \
             {conjunction_refs:?}, cases {:?}, resolutions {:?}",
            result.cases,
            result.resolutions
        );
        for case in &result.cases {
            assert!(parent.contains(case).unwrap());
            assert!(
                native::restrict(case, &conjunction, &indices)
                    .unwrap()
                    .is_empty()
            );
        }
    }
    println!(
        "REVIEW_PLANTED ok={ok} refused={refused} timeouts={timeouts} lifted={lifted} \
         modular_empty={modular_empty} s2_direct={s2_direct}"
    );
    assert!(ok > 0 && s2_direct == 240);
    assert!(
        modular_empty > 0,
        "the obstruction factors exercise S2 end to end"
    );
}
