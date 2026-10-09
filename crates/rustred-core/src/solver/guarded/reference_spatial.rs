//! Independently generated spatial IBPs for the contour-integral controls.
//!
//! These identities differentiate only spatial loop momenta. Energy contours
//! and their labels are spectators; no contour translation, scalelessness,
//! factorization, or supplied target reduction is asserted here.

use std::collections::BTreeMap;

use crate::algebra::{CoefficientContext, CoefficientPolynomial, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{ParametricIbpGenerator, ParametricRelation, RowId};
use crate::solver::{Integral, PolynomialRow, SolverError, Term};

/// Spatial combinations checked against an independent full-D generator.
/// `rows` contains only the L² spatial combinations, with divergence d=D-1.
pub(super) struct SpatialSources<const N: usize> {
    /// Shared public fixture map: d, n1, ..., nN.
    pub coefficients: CoefficientContext,
    pub index_variables: [usize; N],
    pub rows: Vec<PolynomialRow<N>>,
}

/// Ordered coordinates are loop squares, pair-difference squares, then energies.
/// Thus D1 has N=9 (L=3), D4 has N=5 (L=2), and D5 has N=2 (L=1).
pub(super) fn spatial_sources<const N: usize>(
    loops: usize,
) -> Result<SpatialSources<N>, SolverError> {
    if !(1..=3).contains(&loops) || loops * (loops + 3) / 2 != N {
        return Err(invalid(
            "spatial reference supports D5/N=2, D4/N=5, or D1/N=9",
        ));
    }
    let base = crate::algebra::CoefficientContext::try_new(["d"]).map_err(invalid)?;
    let quadratic_count = loops * (loops + 1) / 2;
    let mut scalar_products = BTreeMap::new();
    for i in 0..loops {
        for j in i..loops {
            scalar_products.insert((i, j), scalar_products.len());
        }
    }
    let mut momenta = Vec::new();
    for i in 0..loops {
        let mut momentum = vec![0_i64; loops];
        momentum[i] = 1;
        momenta.push(momentum);
    }
    let mut differences = BTreeMap::new();
    for i in 0..loops {
        for j in i + 1..loops {
            differences.insert((i, j), momenta.len());
            let mut momentum = vec![0_i64; loops];
            momentum[i] = 1;
            momentum[j] = -1;
            momenta.push(momentum);
        }
    }
    let mut denominators = Vec::new();
    for momentum in &momenta {
        let mut coefficients = vec![base.zero(); N];
        for (&(i, j), &position) in &scalar_products {
            coefficients[position] =
                base.integer(momentum[i] * momentum[j] * if i == j { 1 } else { 2 });
        }
        denominators.push(AffineDenominator::new(base.zero(), coefficients));
    }
    for i in 0..loops {
        let mut coefficients = vec![base.zero(); N];
        coefficients[quadratic_count + i] = base.one();
        denominators.push(AffineDenominator::new(base.zero(), coefficients));
    }
    let dimension = &base.parameter("d").expect("declared spatial dimension") + &base.one();
    let family = IntegralFamily::new(
        format!("contour-spatial-{loops}-loop"),
        (0..loops).map(|i| format!("P{i}")).collect(),
        vec!["u".into()],
        base.clone(),
        dimension,
        denominators,
        vec![vec![base.one()]],
        vec![base.zero(); N],
    )
    .map_err(invalid)?;
    let generator = ParametricIbpGenerator::try_new(&family).map_err(invalid)?;
    let context = generator.context().clone();
    let batch = generator.prepare_ordinary_ibp().map_err(invalid)?;
    let generated = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let ordinary = batch.complete(generated).map_err(invalid)?.into_relations();
    let dimension = context
        .lift(&base.parameter("d").expect("declared spatial dimension"))
        .map_err(invalid)?
        .raw()
        .numerator
        .clone();
    let mut rows: Vec<PolynomialRow<N>> = Vec::new();
    for differentiated in 0..loops {
        for contracted in 0..loops {
            // Direct spatial differentiation: ∂p_i·p_j D_h equals
            // 2 c_hi sum_k c_hk (P_j·P_k - E_j E_k).
            let mut direct = BTreeMap::new();
            if differentiated == contracted {
                add(&mut direct, [0; N], dimension.clone());
            }
            for (denominator, momentum) in momenta.iter().enumerate() {
                let index = context
                    .index(denominator)
                    .map_err(invalid)?
                    .raw()
                    .numerator
                    .clone();
                for (k, &component) in momentum.iter().enumerate() {
                    let weight = momentum[differentiated] * component;
                    if weight == 0 {
                        continue;
                    }
                    let mut denominator_terms = vec![(contracted, 1), (k, 1)];
                    if contracted != k {
                        let pair = (contracted.min(k), contracted.max(k));
                        denominator_terms.push((differences[&pair], -1));
                    }
                    for (factor, multiplier) in denominator_terms {
                        let mut shift = [0_i16; N];
                        shift[denominator] += 1;
                        shift[factor] -= 1;
                        let scalar = context
                            .integer(-weight * multiplier)
                            .raw()
                            .numerator
                            .clone();
                        add(&mut direct, shift, &index * &scalar);
                    }
                    let mut shift = [0_i16; N];
                    shift[denominator] += 1;
                    shift[quadratic_count + contracted] -= 1;
                    shift[quadratic_count + k] -= 1;
                    let scalar = context.integer(2 * weight).raw().numerator.clone();
                    add(&mut direct, shift, &index * &scalar);
                }
            }
            // Independent native derivation. R(i,P_j;n)-R(i,u;n-e_Ej)
            // equals ∂p_i·p_j at D=d+1. Translating the complete u-row,
            // including its index coefficients, supplies the derivative of
            // E_j and changes the diagonal divergence from D to D-1.
            let row = |contraction| {
                ordinary
                    .iter()
                    .find(|relation| {
                        *relation.row_id()
                            == RowId::OrdinaryIbp {
                                contraction_momentum: contraction,
                                differentiated_loop: differentiated,
                            }
                    })
                    .ok_or_else(|| invalid("native spatial comparison source is absent"))
            };
            let mut native = BTreeMap::new();
            add_native(&mut native, row(contracted)?, &[0; N], 1, &context)?;
            let mut energy_shift = [0_i64; N];
            energy_shift[quadratic_count + contracted] = -1;
            add_native(&mut native, row(loops)?, &energy_shift, -1, &context)?;
            if direct != native {
                return Err(invalid(format!(
                    "independent spatial/native IBPs disagree at loop {differentiated}, contraction {contracted}",
                )));
            }
            // Spatial differentiation cannot differentiate energy numerators.
            for coefficient in direct.values() {
                for energy in quadratic_count..N {
                    if coefficient.contains(base.parameter_names().len() + energy) {
                        return Err(invalid("spatial row retains an energy-power coefficient"));
                    }
                }
            }
            rows.push(
                direct
                    .into_iter()
                    .map(|(shift, coefficient)| {
                        Ok(Term {
                            integral: Integral::symbolic(shift)?,
                            coefficient,
                        })
                    })
                    .collect::<Result<_, SolverError>>()?,
            );
        }
    }
    let coefficients = CoefficientContext::try_new(
        std::iter::once("d".to_owned()).chain((1..=N).map(|axis| format!("n{axis}"))),
    )
    .map_err(invalid)?;
    let native_variables = context.one().raw().numerator.variables().clone();
    let fixture_variables = coefficients.one().numerator.variables().clone();
    for row in &mut rows {
        for term in row {
            for (native, fixture) in native_variables.iter().zip(fixture_variables.iter()) {
                if native != fixture {
                    term.coefficient.rename_variable(native, fixture);
                }
            }
            if term.coefficient.variables() != &fixture_variables {
                return Err(invalid(
                    "spatial fixture variable rebinding changed coordinate order",
                ));
            }
        }
    }
    let index_variables = std::array::from_fn(|axis| axis + 1);
    Ok(SpatialSources {
        coefficients,
        index_variables,
        rows,
    })
}

fn add_native<const N: usize>(
    row: &mut BTreeMap<[i16; N], CoefficientPolynomial>,
    relation: &ParametricRelation,
    translation: &[i64; N],
    sign: i64,
    context: &IndexedCoefficientContext,
) -> Result<(), SolverError> {
    for (shift, coefficient) in relation.terms() {
        let translated = context
            .translate(coefficient, translation, Default::default())
            .map_err(invalid)?;
        if !translated.raw().denominator.is_one() {
            return Err(invalid(
                "native spatial comparison unexpectedly requires denominator clearing",
            ));
        }
        let mut target = [0_i16; N];
        for (axis, value) in shift.values().iter().enumerate() {
            target[axis] = i16::try_from(*value + translation[axis]).map_err(invalid)?;
        }
        let scalar = context.integer(sign).raw().numerator.clone();
        add(row, target, &translated.raw().numerator * &scalar);
    }
    Ok(())
}

fn add<const N: usize>(
    row: &mut BTreeMap<[i16; N], CoefficientPolynomial>,
    shift: [i16; N],
    value: CoefficientPolynomial,
) {
    if value.is_zero() {
        return;
    }
    match row.entry(shift) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let sum = entry.get() + &value;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

fn invalid(error: impl std::fmt::Display) -> SolverError {
    SolverError::InvalidInput(error.to_string())
}

#[test]
fn one_two_three_loop_spatial_rows_match_native_projected_generators() {
    assert_eq!(spatial_sources::<2>(1).unwrap().rows.len(), 1);
    assert_eq!(spatial_sources::<5>(2).unwrap().rows.len(), 4);
    assert_eq!(spatial_sources::<9>(3).unwrap().rows.len(), 9);
}

#[test]
fn spatial_tadpole_has_d_divergence_and_energy_squared_numerator() {
    let sources = spatial_sources::<2>(1).unwrap();
    let expected = BTreeMap::from([
        (
            [0, 0],
            sources.coefficients.coefficient_fixture("d-2*n1").numerator,
        ),
        (
            [1, -2],
            sources.coefficients.coefficient_fixture("2*n1").numerator,
        ),
    ]);
    let actual: BTreeMap<_, _> = sources.rows[0]
        .iter()
        .map(|term| {
            (
                term.integral.powers().map(|power| power.value()),
                term.coefficient.clone(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
}
