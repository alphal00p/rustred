use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, multiply_coefficient_matrices};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::{
    AffineDenominator, IntegralFamily, ScalarProductCoordinate, congruence_symbolic_matrix,
    invert_symbolic_matrix,
};
use crate::foundry::artifact::FactorizationRule;

use super::exact_limits;
use super::model::{AffineForm, CornerAngularForm, DenominatorRelation, RoutedBasis};
use super::{ARITY, LOOP_COUNT};

fn matrix_limits() -> SymbolicaCoefficientMatrixLimits {
    SymbolicaCoefficientMatrixLimits::default()
}

fn coefficient_matrix(
    context: &CoefficientContext,
    entries: &[i64],
    dimension: usize,
) -> Vec<Vec<Coefficient>> {
    entries
        .chunks_exact(dimension)
        .map(|row| row.iter().map(|&entry| context.integer(entry)).collect())
        .collect()
}

fn transpose(matrix: &[Vec<Coefficient>]) -> Vec<Vec<Coefficient>> {
    (0..matrix.len())
        .map(|column| matrix.iter().map(|row| row[column].clone()).collect())
        .collect()
}

fn denominator_quadratic(
    family: &IntegralFamily,
    denominator: &AffineDenominator,
) -> Vec<Vec<Coefficient>> {
    let context = family.coefficient_context();
    let mut matrix = vec![vec![context.zero(); family.loop_count()]; family.loop_count()];
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                matrix[left][right] = coefficient.clone();
            }
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let half = context
                    .try_div(coefficient, &context.integer(2), exact_limits())
                    .unwrap();
                matrix[left][right] = half.clone();
                matrix[right][left] = half;
            }
            ScalarProductCoordinate::LoopExternal { .. } => {
                panic!("the K=6 pressure family is a vacuum family")
            }
        }
    }
    matrix
}

fn scalar_coefficients_from_quadratic(
    family: &IntegralFamily,
    quadratic: &[Vec<Coefficient>],
) -> Box<[Coefficient]> {
    let context = family.coefficient_context();
    family
        .coordinates()
        .iter()
        .map(|coordinate| match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                quadratic[left][right].clone()
            }
            ScalarProductCoordinate::LoopLoop { left, right } => context
                .try_mul(&context.integer(2), &quadratic[left][right], exact_limits())
                .unwrap(),
            ScalarProductCoordinate::LoopExternal { .. } => {
                panic!("the K=6 pressure family is a vacuum family")
            }
        })
        .collect()
}

/// Derive `D_i(k(q)) = c_i + sum_s a_is S'_s` through Symbolica's exact
/// inverse and congruence matrix APIs.
pub(super) fn transform_family_forms(
    family: &IntegralFamily,
    loop_basis: &[i64],
) -> Box<[AffineForm]> {
    let context = family.coefficient_context();
    let basis = coefficient_matrix(context, loop_basis, family.loop_count());
    let (inverse, determinant) =
        invert_symbolic_matrix(context, &basis, family.construction_limits()).unwrap();
    assert!(determinant == context.one() || determinant == context.integer(-1));
    let inverse_transpose = transpose(&inverse);
    family
        .denominators()
        .iter()
        .map(|denominator| {
            let quadratic = denominator_quadratic(family, denominator);
            let transformed = congruence_symbolic_matrix(
                context,
                &inverse_transpose,
                &quadratic,
                family.construction_limits(),
            )
            .unwrap();
            AffineForm {
                constant: denominator.constant().clone(),
                scalar_coefficients: scalar_coefficients_from_quadratic(family, &transformed),
            }
        })
        .collect()
}

/// Convert transformed scalar-coordinate forms to exact affine combinations
/// of the canonical family denominators. Both matrix products are delegated
/// to Symbolica: first `A^-1 c`, then `[c' a'] [1 0; -A^-1 c A^-1]`.
fn relations_in_canonical_denominators(
    family: &IntegralFamily,
    forms: &[AffineForm],
) -> Box<[DenominatorRelation]> {
    let context = family.coefficient_context();
    let constants = family
        .denominators()
        .iter()
        .map(|denominator| vec![denominator.constant().clone()])
        .collect::<Vec<_>>();
    let (inverse_times_constants, _) =
        multiply_coefficient_matrices(context, family.inverse_basis(), &constants, matrix_limits())
            .unwrap();

    let mut affine_inverse = vec![vec![context.zero(); ARITY + 1]; ARITY + 1];
    affine_inverse[0][0] = context.one();
    for row in 0..ARITY {
        affine_inverse[row + 1][0] = context
            .try_neg(&inverse_times_constants[row][0], exact_limits())
            .unwrap();
        for column in 0..ARITY {
            affine_inverse[row + 1][column + 1] = family.inverse_basis()[row][column].clone();
        }
    }

    forms
        .iter()
        .map(|form| {
            let mut row = Vec::with_capacity(ARITY + 1);
            row.push(form.constant.clone());
            row.extend(form.scalar_coefficients.iter().cloned());
            let (relation, _) =
                multiply_coefficient_matrices(context, &[row], &affine_inverse, matrix_limits())
                    .unwrap();
            let mut relation = relation.into_iter().next().unwrap();
            DenominatorRelation {
                constant: relation.remove(0),
                denominator_coefficients: relation.into_boxed_slice(),
            }
        })
        .collect()
}

fn unit_image(context: &CoefficientContext, relation: &DenominatorRelation) -> Option<usize> {
    if !relation.constant.is_zero() {
        return None;
    }
    let mut image = None;
    for (slot, coefficient) in relation.denominator_coefficients.iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        if coefficient != &context.one() || image.replace(slot).is_some() {
            return None;
        }
    }
    image
}

fn signed_basis(base: &[i64], signs: usize) -> Box<[i64]> {
    base.chunks_exact(LOOP_COUNT)
        .enumerate()
        .flat_map(|(row, entries)| {
            let sign = if signs & (1 << row) == 0 { 1 } else { -1 };
            entries.iter().map(move |entry| sign * entry)
        })
        .collect()
}

/// Row signs are a gauge freedom of a vacuum factorization. Enumerate this
/// finite exact portfolio and retain the lexicographically least basis with
/// the largest number of canonical unit-row images.
pub(super) fn best_routed_basis(family: &IntegralFamily, rule: &FactorizationRule) -> RoutedBasis {
    let context = family.coefficient_context();
    let mut candidates = (0..1_usize << family.loop_count())
        .map(|signs| {
            let basis = signed_basis(rule.loop_basis().row_major(), signs);
            let forms = transform_family_forms(family, &basis);
            let relations = relations_in_canonical_denominators(family, &forms);
            let unit_images = relations
                .iter()
                .map(|relation| unit_image(context, relation))
                .collect::<Box<[_]>>();
            let unit_image_count = unit_images.iter().filter(|image| image.is_some()).count();
            RoutedBasis {
                signed_loop_basis: basis,
                transformed_forms: forms,
                relations,
                unit_images,
                unit_image_count,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .unit_image_count
            .cmp(&left.unit_image_count)
            .then_with(|| left.signed_loop_basis.cmp(&right.signed_loop_basis))
    });
    candidates.remove(0)
}

pub(super) fn factorization_for_sector<'a>(
    rules: &'a [FactorizationRule],
    sector: &[i64; ARITY],
) -> &'a FactorizationRule {
    rules
        .iter()
        .find(|rule| {
            rule.application_domain()
                .sector()
                .active_bits()
                .iter()
                .zip(sector)
                .all(|(&active, &power)| active == (power >= 1))
        })
        .unwrap()
}

pub(super) fn routed_base(
    target: &[i64; ARITY],
    selected: usize,
    routed: &RoutedBasis,
) -> [i64; ARITY] {
    let mut output = [0_i64; ARITY];
    for (source, (&power, image)) in target.iter().zip(&routed.unit_images).enumerate() {
        if source == selected {
            continue;
        }
        let image = image.expect("every nonselected row must have a canonical unit image");
        output[image] = power;
    }
    output
}

pub(super) fn replay_relation(
    family: &IntegralFamily,
    form: &AffineForm,
    relation: &DenominatorRelation,
) {
    let context = family.coefficient_context();
    let mut replay_constant = relation.constant.clone();
    let mut replay_scalars = vec![context.zero(); ARITY];
    for (multiplier, denominator) in relation
        .denominator_coefficients
        .iter()
        .zip(family.denominators())
    {
        let constant = context
            .try_mul(multiplier, denominator.constant(), exact_limits())
            .unwrap();
        replay_constant = context
            .try_add(&replay_constant, &constant, exact_limits())
            .unwrap();
        for (slot, coefficient) in denominator.coefficients().iter().enumerate() {
            let contribution = context
                .try_mul(multiplier, coefficient, exact_limits())
                .unwrap();
            replay_scalars[slot] = context
                .try_add(&replay_scalars[slot], &contribution, exact_limits())
                .unwrap();
        }
    }
    assert!(
        context
            .try_sub(&replay_constant, &form.constant, exact_limits())
            .unwrap()
            .is_zero()
    );
    for (actual, expected) in replay_scalars.iter().zip(&form.scalar_coefficients) {
        assert!(
            context
                .try_sub(actual, expected, exact_limits())
                .unwrap()
                .is_zero()
        );
    }
}

pub(super) fn corner_angular_form(
    family: &IntegralFamily,
    form: &AffineForm,
) -> Result<CornerAngularForm, ExactAlgebraError> {
    let context = family.coefficient_context();
    let mut constant = form.constant.clone();
    let mut cross_coefficients = std::array::from_fn(|_| context.zero());
    for (slot, coordinate) in family.coordinates().iter().enumerate() {
        match *coordinate {
            // At a one-loop tadpole corner, every positive radial moment
            // q_i^(2r) equals the corner after scaleless polynomial pieces
            // are discarded. The production action must instead route these
            // powers through the dependency reducer.
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                constant =
                    context.try_add(&constant, &form.scalar_coefficients[slot], exact_limits())?;
            }
            ScalarProductCoordinate::LoopLoop { left: 0, right: 1 } => {
                cross_coefficients[0] = form.scalar_coefficients[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { left: 0, right: 2 } => {
                cross_coefficients[1] = form.scalar_coefficients[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { left: 1, right: 2 } => {
                cross_coefficients[2] = form.scalar_coefficients[slot].clone();
            }
            ScalarProductCoordinate::LoopLoop { .. } => unreachable!(),
            ScalarProductCoordinate::LoopExternal { .. } => {
                panic!("the K=6 pressure family is a vacuum family")
            }
        }
    }
    Ok(CornerAngularForm {
        constant,
        cross_coefficients,
    })
}
