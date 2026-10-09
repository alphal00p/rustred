//! Source-level comparison of native Euclidean spatial IBPs and independently
//! differentiated occupied-cut integrands for the supplied three-loop seeds.
//!
//! Convention: g_h=(c_h.E)^2-|c_h.p|^2, cut factor C_a(g_h), and for each
//! positively oriented cut theta(s_h*c_h.E) W_0(mu-s_h*c_h.E). Uncut vacuum
//! energy measures are dE/(2*pi*i), whereas cut energies use dE. With this
//! convention the Euclidean conversion factor is (-1)^sum(a) i^sum(alpha),
//! independent of the cut count. Uncut kernels retain the analytic-continuation
//! prescription inherited from the Euclidean cutting construction; this test
//! does not evaluate those kernels or choose prescriptions for their poles.
//!
//! At fixed independent energies, spatial derivatives do not act on the W or
//! positive-energy factors. C'_a=-a C_(a+1), and g C_a=C_(a-1) for a>=2,
//! while g C_1=0. These give the same spatial derivative algebra as ordinary
//! g^(-a), with indispensable culling of nonpositive required-cut powers.
//! The independent rows below are constructed directly from spatial dot
//! products, before any graph routing or reference-reduction certificate.

use std::collections::BTreeMap;
use std::time::Instant;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::solver::PolynomialRow;

use super::reference_cut_plan::cut_plans;
use super::reference_routing::ReferenceKey;
use super::reference_spatial::spatial_sources;

const MOMENTA: [[i64; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, -1, 0],
    [1, 0, -1],
    [0, 1, -1],
];

type RealRow = BTreeMap<ReferenceKey, Coefficient>;
type ComplexRow = BTreeMap<ReferenceKey, (Coefficient, Coefficient)>;

fn key(powers: [i16; 6], energy: [i16; 3]) -> ReferenceKey {
    [
        powers[0], powers[1], powers[2], powers[3], powers[4], powers[5], -energy[0], -energy[1],
        -energy[2],
    ]
}

fn phase(point: &ReferenceKey) -> i64 {
    let denominators = point[..6]
        .iter()
        .map(|power| i64::from(*power))
        .sum::<i64>();
    let energy = -point[6..]
        .iter()
        .map(|power| i64::from(*power))
        .sum::<i64>();
    (2 * denominators + energy).rem_euclid(4)
}

fn with_phase(
    context: &CoefficientContext,
    value: Coefficient,
    phase: i64,
) -> (Coefficient, Coefficient) {
    match phase.rem_euclid(4) {
        0 => (value, context.zero()),
        1 => (context.zero(), value),
        2 => (-value, context.zero()),
        3 => (context.zero(), -value),
        _ => unreachable!(),
    }
}

fn required_cut_zero(mask: u8, point: &ReferenceKey) -> bool {
    (0..6).any(|line| mask & (1 << line) != 0 && point[line] <= 0)
}

fn add(row: &mut RealRow, point: ReferenceKey, coefficient: Coefficient) {
    if coefficient.is_zero() {
        return;
    }
    match row.entry(point) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(coefficient);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let sum = entry.get() + &coefficient;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

fn native_cut_image(
    context: &CoefficientContext,
    row: &PolynomialRow<9>,
    variables: &[usize; 9],
    point: &ReferenceKey,
    mask: u8,
) -> (ComplexRow, usize) {
    let mut image = BTreeMap::new();
    let mut culled = 0;
    for term in row {
        let mut coefficient = term.coefficient.clone();
        for axis in 0..9 {
            coefficient = coefficient.replace(variables[axis], &i64::from(point[axis]).into());
        }
        if coefficient.is_zero() {
            continue;
        }
        let target = std::array::from_fn(|axis| point[axis] + term.integral[axis].value());
        // Each denominator-numerator or energy-numerator term has the same
        // overall Wick phase as its input. Check this instead of assuming it.
        assert_eq!(phase(&target), phase(point));
        if required_cut_zero(mask, &target) {
            culled += 1;
            continue;
        }
        image.insert(
            target,
            with_phase(context, Coefficient::from(coefficient), phase(&target)),
        );
    }
    (image, culled)
}

/// Differentiate the physical spatial integrand directly:
/// d/dp_i . p_j = d delta_ij + sum_h 2 a_h c_hi (q_h.p_j)_spatial / g_h.
/// Its quadratic products are reconstructed from
/// p_k.p_j = E_k E_j - (g_k+g_j-g_(k-j))/2, with the diagonal g_(k-k)=0.
/// Cut and ordinary factors use their own derivative/multiplication laws;
/// only the cut factors annihilate nonpositive resulting powers.
fn weighted_spatial_derivative(
    context: &CoefficientContext,
    point: &ReferenceKey,
    mask: u8,
    differentiated: usize,
    contracted: usize,
) -> (RealRow, usize) {
    let mut row = RealRow::new();
    let mut culled = 0;
    let mut insert = |target, coefficient: Coefficient| {
        if coefficient.is_zero() {
            return;
        }
        if required_cut_zero(mask, &target) {
            culled += 1;
        } else {
            add(&mut row, target, coefficient);
        }
    };
    if differentiated == contracted {
        insert(*point, context.parameter("d").unwrap());
    }
    for (line, covector) in MOMENTA.iter().enumerate() {
        let derivative = i64::from(point[line]) * covector[differentiated];
        if derivative == 0 {
            continue;
        }
        let mut raised = *point;
        raised[line] += 1;
        for (axis, &component) in covector.iter().enumerate() {
            if component == 0 {
                continue;
            }
            let factor = derivative * component;
            // The positive energy product in twice the spatial dot product.
            let mut energy = raised;
            energy[6 + axis] -= 1;
            energy[6 + contracted] -= 1;
            insert(energy, context.integer(2 * factor));
            // Minus twice the Minkowski dot product, with all 1/2 factors
            // cancelled against the derivative's factor two.
            let mut first_square = raised;
            first_square[axis] -= 1;
            insert(first_square, context.integer(-factor));
            let mut second_square = raised;
            second_square[contracted] -= 1;
            insert(second_square, context.integer(-factor));
            if axis != contracted {
                let difference = match (axis.min(contracted), axis.max(contracted)) {
                    (0, 1) => 3,
                    (0, 2) => 4,
                    (1, 2) => 5,
                    _ => unreachable!(),
                };
                let mut difference_square = raised;
                difference_square[difference] -= 1;
                insert(difference_square, context.integer(factor));
            }
        }
    }
    (row, culled)
}

#[test]
fn three_loop_native_spatial_sources_match_independent_weighted_cut_derivatives() {
    let started = Instant::now();
    let sources = spatial_sources::<9>(3).unwrap();
    let construction = started.elapsed();
    let context = &sources.coefficients;
    assert_eq!(sources.rows.len(), 9);
    let f = [1, 1, 1];
    let b = [0, 0, 1];
    let m = [1, 1, 0];
    let seeds = [
        ("R5-polynomial", f, key([1, -1, 1, 1, 1, 1], [0; 3])),
        ("R6-polynomial", f, key([1, 1, 1, 1, 1, -1], [0; 3])),
        ("R7-polynomial", m, key([1, 1, -1, 1, 1, 1], [0; 3])),
        ("R6-a2-020-BB+", b, key([2, 1, 0, 0, 1, 1], [0, 2, 0])),
        ("R6-a2-020-+++", f, key([2, 1, 0, 0, 1, 1], [0, 2, 0])),
        ("R6-a2-110", b, key([2, 1, 0, 0, 1, 1], [1, 1, 0])),
        ("R6-a2-200", b, key([2, 1, 0, 0, 1, 1], [2, 0, 0])),
        ("R6-a22-310", b, key([2, 2, 0, 0, 1, 1], [3, 1, 0])),
        ("R6-a3-310", b, key([3, 1, 0, 0, 1, 1], [3, 1, 0])),
        ("R6-a3-400", b, key([3, 1, 0, 0, 1, 1], [4, 0, 0])),
        ("R7-a2-020", m, key([2, 1, 0, 0, 1, 1], [0, 2, 0])),
        ("R7-a2-110", m, key([2, 1, 0, 0, 1, 1], [1, 1, 0])),
        ("R7-a22-310", m, key([2, 2, 0, 0, 1, 1], [3, 1, 0])),
        ("R7-a3-310", m, key([3, 1, 0, 0, 1, 1], [3, 1, 0])),
        ("R7-a3-400", m, key([3, 1, 0, 0, 1, 1], [4, 0, 0])),
        // Additional odd-total-degree control makes an omitted i observable.
        ("odd-phase-control", f, key([2, 1, 1, 1, 1, 1], [1, 0, 0])),
    ];
    let comparison_started = Instant::now();
    let mut cut_masks = 0;
    let mut rows_checked = 0;
    let mut compared_terms = 0;
    let mut native_boundary_terms = 0;
    let mut weighted_boundary_contributions = 0;
    let mut raised_cut_masks = 0;
    let mut imaginary_rows = 0;
    for (label, contours, point) in seeds {
        for plan in cut_plans(context, contours, point).unwrap() {
            cut_masks += 1;
            raised_cut_masks += usize::from(plan.legs.iter().any(|leg| leg.original_power > 1));
            for differentiated in 0..3 {
                for contracted in 0..3 {
                    let ordinal = differentiated * 3 + contracted;
                    let (actual, native_culled) = native_cut_image(
                        context,
                        &sources.rows[ordinal],
                        &sources.index_variables,
                        &point,
                        plan.cut_mask,
                    );
                    let (weighted, weighted_culled) = weighted_spatial_derivative(
                        context,
                        &point,
                        plan.cut_mask,
                        differentiated,
                        contracted,
                    );
                    let expected: ComplexRow = weighted
                        .into_iter()
                        .map(|(target, coefficient)| {
                            (target, with_phase(context, coefficient, phase(&point)))
                        })
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "{label}, mask={}, derivative={differentiated}, vector={contracted}",
                        plan.cut_mask
                    );
                    assert!(
                        actual
                            .keys()
                            .all(|target| !required_cut_zero(plan.cut_mask, target))
                    );
                    rows_checked += 1;
                    compared_terms += actual.len();
                    native_boundary_terms += native_culled;
                    weighted_boundary_contributions += weighted_culled;
                    imaginary_rows +=
                        usize::from(actual.values().any(|(_, imaginary)| !imaginary.is_zero()));
                }
            }
        }
    }
    assert_eq!(rows_checked, cut_masks * 9);
    assert!(cut_masks > 100 && compared_terms > rows_checked);
    assert!(native_boundary_terms > 0 && weighted_boundary_contributions > 0);
    assert!(raised_cut_masks > 0 && imaginary_rows > 0);
    eprintln!(
        "D1 cutwise spatial source comparison: seeds={} vectors_per_seed=9 cut_masks={cut_masks} exact_rows={rows_checked} compared_terms={compared_terms} raised_cut_masks={raised_cut_masks} native_boundary_terms={native_boundary_terms} weighted_boundary_contributions={weighted_boundary_contributions} imaginary_rows={imaginary_rows} native_construction={construction:?} comparison={:?}; before routing, kernels unevaluated",
        seeds.len(),
        comparison_started.elapsed()
    );
}
