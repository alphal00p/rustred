//! Formal occupied-cut recipes for the supplied D1 reference terms.
//!
//! This is a test utility, not a vacuum-kernel evaluator or a proof of any
//! supplied reduction. A cut set consists of linearly independent charged
//! denominator covectors. Independence implements the non-disconnecting-cut
//! condition in the original diagram; deleting edges of the *momentum*
//! tetrahedron would implement the wrong condition. Every cut momentum is
//! oriented to positive chemical potential before its energy becomes iE.
//!
//! The uncut kernel is evaluated at zero chemical potential, with the cut
//! momenta as external momenta. Its integration prescription remains that of
//! the finite-density cutting rules (arXiv:1609.04339, sections 2 and 4).
//! Polynomial energy numerators remain part of the kernel H. Raised-cut jets
//! differentiate that complete H, including these numerator phases; they
//! cannot be replaced by derivatives of the numerator alone.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};

use super::reference_cut_conversion::{CutJet, raised_cut_expansion};
use super::reference_routing::{ReferenceKey, canonical_contours};

/// Denominators P², Q², R², (P-Q)², (P-R)², (Q-R)².
const COVECTORS: [[i64; 3]; 6] = [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, -1, 0],
    [1, 0, -1],
    [0, 1, -1],
];
const MAX_ENERGY_DEGREE: usize = 24;
const MAX_JET_TERMS: usize = 65_536;

/// Exact coefficients (real, imaginary) of monomials in the new energies.
/// Cut axes denote real positive E; the remaining axes denote real vacuum
/// contour energies. Factors of i on the cut axes are already included.
pub(super) type EnergyPolynomial = BTreeMap<[u16; 3], (i64, i64)>;

#[derive(Clone, Debug)]
pub(super) struct ReferenceCutLeg {
    pub line: usize,
    pub original_power: u32,
    pub orientation: i8,
}

#[derive(Clone, Debug)]
pub(super) struct ReferenceCutPlan {
    pub cut_mask: u8,
    /// Legs follow ascending original line number and occupy the first
    /// `legs.len()` rows of loop_basis / axes of kernel_jets.
    pub legs: Vec<ReferenceCutLeg>,
    /// New momenta = loop_basis * (P,Q,R). Cut rows come first; original
    /// unit momenta complete them to an invertible integer basis.
    pub loop_basis: [[i64; 3]; 3],
    /// (P0,Q0,R0) = original_from_basis * (iE_cut, z_vacuum).
    pub original_from_basis: [[i64; 3]; 3],
    /// All six original line covectors in the new basis, including cuts.
    pub kernel_covectors: [[i64; 3]; 6],
    /// Selected cut propagators are absent from H. Negative original powers
    /// remain polynomial numerators, and unselected raised powers remain.
    pub kernel_powers: [i16; 6],
    pub energy_numerator: EnergyPolynomial,
    /// Tensor products of per-leg (extra E power, H derivative, W index).
    /// Unused vacuum axes carry (0,0,0). Coefficients include all cut minus
    /// signs and raised-power factorials. Derivatives act on the FULL H.
    pub kernel_jets: BTreeMap<[CutJet; 3], Coefficient>,
}

/// Enumerate formal cut recipes without importing any reference equality.
/// The supported contour classes are precisely the supplied two-level
/// chemical-potential routings. No scaleless or zero-sector inference occurs.
pub(super) fn cut_plans(
    context: &CoefficientContext,
    contours: [i8; 3],
    key: ReferenceKey,
) -> Result<Vec<ReferenceCutPlan>, String> {
    canonical_contours(contours)?;
    if key[6..].iter().any(|&power| power > 0) {
        return Err("cut-plan energy indices must be polynomial numerators".into());
    }
    let degree: usize = key[6..]
        .iter()
        .map(|power| usize::from(power.unsigned_abs()))
        .sum();
    if degree > MAX_ENERGY_DEGREE {
        return Err("cut-plan numerator degree budget exhausted".into());
    }
    let charges = COVECTORS.map(|covector| {
        covector
            .iter()
            .zip(contours)
            .map(|(coefficient, charge)| coefficient * i64::from(charge))
            .sum::<i64>()
    });
    let active = (0..6).fold(0_u8, |mask, line| {
        if key[line] > 0 && charges[line] != 0 {
            mask | (1 << line)
        } else {
            mask
        }
    });
    let mut plans = Vec::new();
    for mask in 0_u8..64 {
        if mask & !active != 0 {
            continue;
        }
        let lines: Vec<_> = (0..6).filter(|line| mask & (1 << line) != 0).collect();
        let mut rows: Vec<_> = lines
            .iter()
            .map(|&line| COVECTORS[line].map(|entry| charges[line].signum() * entry))
            .collect();
        if !independent(&rows) {
            continue;
        }
        for unit in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
            if rows.len() == 3 {
                break;
            }
            let mut extended = rows.clone();
            extended.push(unit);
            if independent(&extended) {
                rows = extended;
            }
        }
        let basis: [[i64; 3]; 3] = rows
            .try_into()
            .map_err(|_| "failed to complete an independent cut basis".to_string())?;
        let inverse = integer_inverse(basis)?;
        let kernel_covectors = COVECTORS.map(|row| {
            std::array::from_fn(|column| (0..3).map(|i| row[i] * inverse[i][column]).sum())
        });
        let mut kernel_powers = std::array::from_fn(|line| key[line]);
        let mut legs = Vec::new();
        let mut kernel_jets = BTreeMap::from([([(0, 0, 0); 3], context.one())]);
        for (axis, &line) in lines.iter().enumerate() {
            kernel_powers[line] = 0;
            let power = u32::try_from(key[line]).map_err(|_| "nonpositive cut power")?;
            // This test utility is intended for the supplied raised powers,
            // not an unbounded symbolic differentiator.
            if power > 8 {
                return Err("cut-plan raised-power budget exhausted".into());
            }
            let jets = raised_cut_expansion(context, power).map_err(|error| error.to_string())?;
            if kernel_jets
                .len()
                .checked_mul(jets.len())
                .is_none_or(|terms| terms > MAX_JET_TERMS)
            {
                return Err("cut-plan mixed-kernel-jet budget exhausted".into());
            }
            let mut product = BTreeMap::new();
            for (previous, coefficient) in &kernel_jets {
                for (&jet, factor) in &jets {
                    let mut output = *previous;
                    output[axis] = jet;
                    product.insert(output, coefficient * factor);
                }
            }
            kernel_jets = product;
            legs.push(ReferenceCutLeg {
                line,
                original_power: power,
                orientation: charges[line].signum() as i8,
            });
        }
        let energy_numerator = expand_energy_numerator(inverse, lines.len(), &key)?;
        plans.push(ReferenceCutPlan {
            cut_mask: mask,
            legs,
            loop_basis: basis,
            original_from_basis: inverse,
            kernel_covectors,
            kernel_powers,
            energy_numerator,
            kernel_jets,
        });
    }
    Ok(plans)
}

fn determinant(matrix: [[i64; 3]; 3]) -> i64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn independent(rows: &[[i64; 3]]) -> bool {
    match rows {
        [] => true,
        [a] => a.iter().any(|entry| *entry != 0),
        [a, b] => (0..3).any(|i| {
            let j = (i + 1) % 3;
            a[i] * b[j] - a[j] * b[i] != 0
        }),
        [a, b, c] => determinant([*a, *b, *c]) != 0,
        _ => false,
    }
}

fn integer_inverse(matrix: [[i64; 3]; 3]) -> Result<[[i64; 3]; 3], String> {
    let determinant = determinant(matrix);
    if determinant.abs() != 1 {
        return Err("cut basis is not unimodular".into());
    }
    Ok(std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            let kept_rows: Vec<_> = (0..3).filter(|i| *i != column).collect();
            let kept_columns: Vec<_> = (0..3).filter(|i| *i != row).collect();
            let minor = matrix[kept_rows[0]][kept_columns[0]]
                * matrix[kept_rows[1]][kept_columns[1]]
                - matrix[kept_rows[0]][kept_columns[1]] * matrix[kept_rows[1]][kept_columns[0]];
            let sign = if (row + column) % 2 == 0 { 1 } else { -1 };
            sign * minor / determinant
        })
    }))
}

fn expand_energy_numerator(
    inverse: [[i64; 3]; 3],
    cut_count: usize,
    key: &ReferenceKey,
) -> Result<EnergyPolynomial, String> {
    let mut polynomial = BTreeMap::from([([0_u16; 3], 1_i64)]);
    for original in 0..3 {
        for _ in 0..key[6 + original].unsigned_abs() {
            let mut next = BTreeMap::new();
            for (powers, coefficient) in polynomial {
                for (axis, &factor) in inverse[original].iter().enumerate() {
                    if factor == 0 {
                        continue;
                    }
                    let mut output = powers;
                    output[axis] = output[axis]
                        .checked_add(1)
                        .ok_or_else(|| "cut numerator degree overflow".to_string())?;
                    let contribution = coefficient
                        .checked_mul(factor)
                        .ok_or_else(|| "cut numerator coefficient overflow".to_string())?;
                    let previous = next.entry(output).or_insert(0_i64);
                    *previous = previous
                        .checked_add(contribution)
                        .ok_or_else(|| "cut numerator coefficient overflow".to_string())?;
                }
            }
            next.retain(|_, value| *value != 0);
            polynomial = next;
        }
    }
    polynomial
        .into_iter()
        .map(|(powers, coefficient)| {
            let phase: u16 = powers[..cut_count].iter().sum();
            let negative = || {
                coefficient
                    .checked_neg()
                    .ok_or_else(|| "cut phase overflow".to_string())
            };
            let complex = match phase % 4 {
                0 => (coefficient, 0),
                1 => (0, coefficient),
                2 => (negative()?, 0),
                3 => (0, negative()?),
                _ => unreachable!(),
            };
            Ok((powers, complex))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(powers: [i16; 6], energy: [i16; 3]) -> ReferenceKey {
        [
            powers[0], powers[1], powers[2], powers[3], powers[4], powers[5], -energy[0],
            -energy[1], -energy[2],
        ]
    }

    #[test]
    fn allowed_cut_sets_use_covector_independence_and_positive_propagator_powers() {
        let context = CoefficientContext::new(["d"]);
        let full = key([1; 6], [0; 3]);
        let three = cut_plans(&context, [1, 1, 1], full).unwrap();
        assert_eq!(three.len(), 8);
        assert_eq!(
            three.iter().map(|plan| plan.cut_mask).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        // P,Q,R form a valid complete cut basis. Deleting those three edges
        // from the momentum-coordinate tetrahedron isolates its origin, but
        // that is not the graph-theoretic cut condition for the Feynman graph.
        assert_eq!(three.last().unwrap().legs.len(), 3);
        let two = cut_plans(&context, [1, 1, 0], full).unwrap();
        assert_eq!(two.len(), 15);
        assert!(two.iter().all(|plan| plan.cut_mask != 0b110011));
        assert!(two.iter().any(|plan| plan.cut_mask == 0));
        let polynomial = cut_plans(&context, [1, 1, 1], key([0, -1, 1, 1, 1, 1], [0; 3])).unwrap();
        assert_eq!(
            polynomial
                .iter()
                .map(|plan| plan.cut_mask)
                .collect::<Vec<_>>(),
            vec![0, 4]
        );
        assert!(polynomial.iter().all(|plan| plan.kernel_powers[1] == -1));
        assert_eq!(cut_plans(&context, [0; 3], full).unwrap().len(), 1);
    }

    #[test]
    fn every_cut_basis_has_exact_inverse_and_correct_oriented_line_images() {
        let context = CoefficientContext::new(["d"]);
        for contours in [[1, 1, 1], [1, 1, 0], [0, 0, 1], [-1, 0, 0]] {
            for plan in cut_plans(&context, contours, key([1; 6], [0; 3])).unwrap() {
                assert_eq!(determinant(plan.loop_basis).abs(), 1);
                for row in 0..3 {
                    for column in 0..3 {
                        assert_eq!(
                            (0..3)
                                .map(|i| plan.loop_basis[row][i]
                                    * plan.original_from_basis[i][column])
                                .sum::<i64>(),
                            i64::from(row == column)
                        );
                    }
                }
                for (axis, leg) in plan.legs.iter().enumerate() {
                    assert_eq!(leg.original_power, 1);
                    assert_eq!(plan.kernel_powers[leg.line], 0);
                    let expected = std::array::from_fn(|column| {
                        if column == axis {
                            i64::from(leg.orientation)
                        } else {
                            0
                        }
                    });
                    assert_eq!(plan.kernel_covectors[leg.line], expected);
                    let charge = plan.loop_basis[axis]
                        .iter()
                        .zip(contours)
                        .map(|(factor, shift)| factor * i64::from(shift))
                        .sum::<i64>();
                    assert_eq!(charge, 1);
                }
                let expected_sign = context.integer(if plan.legs.len() % 2 == 0 { 1 } else { -1 });
                assert_eq!(
                    plan.kernel_jets,
                    BTreeMap::from([([(0, 0, 0); 3], expected_sign)])
                );
            }
        }
    }

    #[test]
    fn negatively_charged_lines_preserve_energy_polynomials_and_i_phases() {
        let context = CoefficientContext::new(["d"]);
        let plans = cut_plans(&context, [0, 0, 1], key([1; 6], [0, 0, 2])).unwrap();
        let plan = plans.iter().find(|plan| plan.cut_mask == 1 << 4).unwrap();
        assert_eq!(plan.legs[0].orientation, -1);
        // The cut momentum is R-P; the completed basis is (R-P,P,Q), hence
        // R0=iE+zP and R0²=-E²+2iE*zP+zP².
        assert_eq!(plan.loop_basis, [[-1, 0, 1], [1, 0, 0], [0, 1, 0]]);
        assert_eq!(
            plan.energy_numerator,
            BTreeMap::from([
                ([2, 0, 0], (-1, 0)),
                ([1, 1, 0], (0, 2)),
                ([0, 2, 0], (1, 0))
            ])
        );
    }

    #[test]
    fn all_supplied_energy_tensors_expand_to_the_original_complex_polynomial() {
        let context = CoefficientContext::new(["d"]);
        let multiply = |left: (i128, i128), right: (i128, i128)| {
            (
                left.0 * right.0 - left.1 * right.1,
                left.0 * right.1 + left.1 * right.0,
            )
        };
        let power = |value, exponent: u16| {
            (0..exponent).fold((1_i128, 0_i128), |product, _| multiply(product, value))
        };
        let values = [2_i128, 3, 5];
        for contours in [[1, 1, 1], [1, 1, 0], [0, 0, 1]] {
            for energy in [
                [0; 3],
                [0, 2, 0],
                [1, 1, 0],
                [2, 0, 0],
                [3, 1, 0],
                [4, 0, 0],
                [1, 0, 1],
            ] {
                for plan in cut_plans(&context, contours, key([1; 6], energy)).unwrap() {
                    let basis_values: [(i128, i128); 3] = std::array::from_fn(|axis| {
                        if axis < plan.legs.len() {
                            (0, values[axis])
                        } else {
                            (values[axis], 0)
                        }
                    });
                    let original: [(i128, i128); 3] = plan.original_from_basis.map(|row| {
                        row.iter().zip(basis_values).fold(
                            (0_i128, 0_i128),
                            |sum, (factor, value)| {
                                (
                                    sum.0 + i128::from(*factor) * value.0,
                                    sum.1 + i128::from(*factor) * value.1,
                                )
                            },
                        )
                    });
                    let expected = (0..3).fold((1_i128, 0_i128), |product, axis| {
                        multiply(product, power(original[axis], energy[axis] as u16))
                    });
                    let actual = plan.energy_numerator.iter().fold(
                        (0_i128, 0_i128),
                        |sum, (powers, coefficient)| {
                            let value = (0..3)
                                .map(|axis| values[axis].pow(u32::from(powers[axis])))
                                .product::<i128>();
                            (
                                sum.0 + i128::from(coefficient.0) * value,
                                sum.1 + i128::from(coefficient.1) * value,
                            )
                        },
                    );
                    assert_eq!(
                        actual, expected,
                        "contours={contours:?}, energy={energy:?}, mask={}",
                        plan.cut_mask
                    );
                }
            }
        }
    }

    #[test]
    fn raised_multi_cut_recipes_keep_surface_and_mixed_full_kernel_derivatives() {
        let context = CoefficientContext::new(["d"]);
        let plans = cut_plans(&context, [1, 1, 0], key([3, 2, 1, 1, 1, 1], [0; 3])).unwrap();
        let plan = plans.iter().find(|plan| plan.cut_mask == 3).unwrap();
        assert_eq!(plan.kernel_jets.len(), 18);
        assert!(
            plan.kernel_jets
                .keys()
                .any(|jets| jets[0].2 > 0 && jets[1].2 > 0)
        );
        assert!(
            plan.kernel_jets
                .keys()
                .any(|jets| jets[0].1 > 0 && jets[1].1 > 0)
        );
        for jets in plan.kernel_jets.keys() {
            for (axis, leg) in plan.legs.iter().enumerate() {
                let (energy, derivative, surface) = jets[axis];
                assert!(derivative + surface < leg.original_power);
                assert_eq!(
                    energy,
                    -2 * (i64::from(leg.original_power) - 1) + i64::from(derivative + surface)
                );
            }
        }
        // A scalar numerator is no permission to drop H derivatives: the
        // exchanged/vacuum denominators retain cut-energy dependence.
        assert_eq!(plan.energy_numerator, BTreeMap::from([([0; 3], (1, 0))]));
    }
}
