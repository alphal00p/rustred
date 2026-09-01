//! Exact, non-owning routing for the frozen matcher-chart experiment.
//!
//! The witness rows define local loop momenta `q = T k`.  Surviving parent
//! denominators are routed into the local chart with `T^-T Q T^-1`, the local
//! basis is completed deterministically, and every completed row is routed
//! back with `T^T Q_local T`.  Symbolica owns inversion, congruence, affine
//! basis conversion, and replay through RustRed's checked matrix adapters.

use std::error::Error;
use std::fmt;

use crate::algebra::matrix::multiply_coefficient_matrices;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::isp::IspCompletion;
use crate::family::{
    AffineDenominator, IntegralFamily, IntegralKey, IntegralKeyError, ScalarProductCoordinate,
    congruence_symbolic_matrix, invert_symbolic_matrix, symbolica_matrix_limits,
};

use super::super::manifest::VakintClassWitness;

/// Caller-owned work limit for one concrete numerator-only chart admission.
/// This is a resource policy, not a semantic rank bound on RustRed integrals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MatcherChartTransportLimits {
    max_total_auxiliary_numerator_degree: u64,
}

impl MatcherChartTransportLimits {
    pub(super) const fn new(max_total_auxiliary_numerator_degree: u64) -> Self {
        Self {
            max_total_auxiliary_numerator_degree,
        }
    }

    pub(super) const fn max_total_auxiliary_numerator_degree(self) -> u64 {
        self.max_total_auxiliary_numerator_degree
    }
}

/// One exact affine local row after routing it back to the parent denominator
/// basis: `C_local = constant + sum_i coefficients[i] D_parent_i`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParentAffineRelation {
    constant: Coefficient,
    denominator_coefficients: Box<[Coefficient]>,
}

impl ParentAffineRelation {
    pub(super) fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub(super) fn denominator_coefficients(&self) -> &[Coefficient] {
        &self.denominator_coefficients
    }
}

/// Cold exact routing evidence for one foreign matcher chart.
///
/// This value can admit fixed numerator-only samples for later expansion.  It
/// cannot construct parent sources, rules, terminal owners, or artifacts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExactMatcherChartRouting {
    loop_basis: Box<[i64]>,
    inverse_loop_basis: Box<[Coefficient]>,
    determinant: Coefficient,
    physical_parent_slots: Box<[usize]>,
    local_to_parent: Box<[ParentAffineRelation]>,
}

impl ExactMatcherChartRouting {
    pub(super) fn loop_basis(&self) -> &[i64] {
        &self.loop_basis
    }

    pub(super) fn inverse_loop_basis(&self) -> &[Coefficient] {
        &self.inverse_loop_basis
    }

    pub(super) fn determinant(&self) -> &Coefficient {
        &self.determinant
    }

    pub(super) fn physical_parent_slots(&self) -> &[usize] {
        &self.physical_parent_slots
    }

    pub(super) fn local_to_parent(&self) -> &[ParentAffineRelation] {
        &self.local_to_parent
    }

    /// Admit only the finite part of local-to-parent transport.  Physical
    /// lines retain their stable parent slots.  Auxiliary rows may occur only
    /// as fixed polynomial numerators; their exact affine relations are
    /// retained above for a later Symbolica sparse-polynomial expansion.
    pub(super) fn try_admit_numerator_only_transport(
        &self,
        local: &IntegralKey,
        limits: MatcherChartTransportLimits,
    ) -> Result<ParentTransportAdmission, MatcherChartTransportError> {
        let expected = self.local_to_parent.len();
        if local.powers().len() != expected {
            return Err(MatcherChartTransportError::WrongArity {
                expected,
                actual: local.powers().len(),
            });
        }

        let mut parent_powers = vec![0_i64; expected];
        for (local_slot, &parent_slot) in self.physical_parent_slots.iter().enumerate() {
            parent_powers[parent_slot] = local.powers()[local_slot];
        }

        let mut total_auxiliary_numerator_degree = 0_u64;
        for (local_slot, &power) in local
            .powers()
            .iter()
            .enumerate()
            .skip(self.physical_parent_slots.len())
        {
            if power > 0 {
                return Err(MatcherChartTransportError::PositiveAuxiliaryPole {
                    local_slot,
                    power,
                });
            }
            let degree = power.unsigned_abs();
            total_auxiliary_numerator_degree = total_auxiliary_numerator_degree
                .checked_add(degree)
                .ok_or(MatcherChartTransportError::AuxiliaryDegreeOverflow)?;
            if total_auxiliary_numerator_degree > limits.max_total_auxiliary_numerator_degree() {
                return Err(MatcherChartTransportError::AuxiliaryDegreeLimit {
                    requested: total_auxiliary_numerator_degree,
                    limit: limits.max_total_auxiliary_numerator_degree(),
                });
            }
        }

        Ok(ParentTransportAdmission {
            parent_physical_key: IntegralKey::try_new(parent_powers)?,
            total_auxiliary_numerator_degree,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParentTransportAdmission {
    parent_physical_key: IntegralKey,
    total_auxiliary_numerator_degree: u64,
}

impl ParentTransportAdmission {
    pub(super) fn parent_physical_key(&self) -> &IntegralKey {
        &self.parent_physical_key
    }

    pub(super) const fn total_auxiliary_numerator_degree(&self) -> u64 {
        self.total_auxiliary_numerator_degree
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MatcherChartTransportError {
    WrongArity { expected: usize, actual: usize },
    PositiveAuxiliaryPole { local_slot: usize, power: i64 },
    AuxiliaryDegreeOverflow,
    AuxiliaryDegreeLimit { requested: u64, limit: u64 },
    IntegralKey(IntegralKeyError),
}

impl fmt::Display for MatcherChartTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "matcher-chart transport expected {expected} powers but received {actual}"
            ),
            Self::PositiveAuxiliaryPole { local_slot, power } => write!(
                formatter,
                "local auxiliary slot {local_slot} has unsupported positive pole power {power}"
            ),
            Self::AuxiliaryDegreeOverflow => {
                formatter.write_str("the total fixed auxiliary numerator degree overflowed u64")
            }
            Self::AuxiliaryDegreeLimit { requested, limit } => write!(
                formatter,
                "fixed auxiliary numerator degree {requested} exceeds the matcher-chart limit {limit}"
            ),
            Self::IntegralKey(error) => error.fmt(formatter),
        }
    }
}

impl Error for MatcherChartTransportError {}

impl From<IntegralKeyError> for MatcherChartTransportError {
    fn from(error: IntegralKeyError) -> Self {
        Self::IntegralKey(error)
    }
}

/// Apply one frozen routing witness, complete the resulting local coordinate
/// chart, and independently replay every completed row in the parent basis.
pub(super) fn try_route_and_complete(
    parent: &IntegralFamily,
    witness: VakintClassWitness,
) -> Result<(IspCompletion, ExactMatcherChartRouting), Box<dyn Error>> {
    if parent.external_count() != 0 {
        return Err("the matcher-chart routing fixture supports vacuum families only".into());
    }
    if parent.denominator_count() != witness.active_slots.len() {
        return Err("matcher-chart stable-slot arity does not match the parent family".into());
    }
    let loop_count = parent.loop_count();
    let expected_entries = loop_count
        .checked_mul(loop_count)
        .ok_or("matcher-chart loop-basis size overflow")?;
    if witness.routing_rows.len() != expected_entries {
        return Err("matcher-chart loop basis has the wrong shape".into());
    }

    let context = parent.coefficient_context();
    let loop_basis = witness.routing_rows.to_vec().into_boxed_slice();
    let basis = coefficient_matrix(context, &loop_basis, loop_count);
    let (inverse, determinant) =
        invert_symbolic_matrix(context, &basis, parent.construction_limits())?;
    if determinant != context.one() && determinant != context.integer(-1) {
        return Err("a matcher chart supplied a non-unimodular loop basis".into());
    }
    let inverse_transpose = transpose_square(&inverse)?;

    let physical_parent_slots = witness
        .active_slots
        .iter()
        .enumerate()
        .filter_map(|(slot, &active)| active.then_some(slot))
        .collect::<Vec<_>>();
    let routed_physical = physical_parent_slots
        .iter()
        .map(|&slot| route_denominator(parent, &parent.denominators()[slot], &inverse_transpose))
        .collect::<Result<Vec<_>, _>>()?;
    let chart_id = witness
        .active_slots
        .iter()
        .map(|&active| if active { '1' } else { '0' })
        .collect::<String>();
    let completion = IspCompletion::try_new(
        format!("rustred-k6-sector-seed-chart-{chart_id}"),
        (0..loop_count)
            .map(|index| format!("q{}", index + 1))
            .collect(),
        parent.external_momenta().to_vec(),
        context.clone(),
        parent.dimension().clone(),
        routed_physical,
        parent.external_gram().to_vec(),
        physical_parent_slots
            .iter()
            .map(|&slot| parent.power_shifts()[slot].clone())
            .collect(),
    )?;
    if completion.family().fingerprint() == parent.fingerprint() {
        return Err("a matcher chart inherited parent family authority".into());
    }

    let basis_transpose = transpose_square(&basis)?;
    let back_routed = completion
        .family()
        .denominators()
        .iter()
        .map(|denominator| route_denominator(completion.family(), denominator, &basis_transpose))
        .collect::<Result<Vec<_>, _>>()?;
    let local_to_parent = relations_in_parent_denominators(parent, &back_routed)?;
    replay_parent_relations(parent, &back_routed, &local_to_parent)?;
    verify_stable_physical_slots(parent, &physical_parent_slots, &local_to_parent)?;

    let inverse_loop_basis = inverse
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Ok((
        completion,
        ExactMatcherChartRouting {
            loop_basis,
            inverse_loop_basis,
            determinant,
            physical_parent_slots: physical_parent_slots.into_boxed_slice(),
            local_to_parent,
        },
    ))
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

fn transpose_square(matrix: &[Vec<Coefficient>]) -> Result<Vec<Vec<Coefficient>>, Box<dyn Error>> {
    let dimension = matrix.len();
    if matrix.iter().any(|row| row.len() != dimension) {
        return Err("Symbolica returned a non-square matcher-chart matrix".into());
    }
    Ok((0..dimension)
        .map(|column| matrix.iter().map(|row| row[column].clone()).collect())
        .collect())
}

fn route_denominator(
    family: &IntegralFamily,
    denominator: &AffineDenominator,
    transform: &[Vec<Coefficient>],
) -> Result<AffineDenominator, Box<dyn Error>> {
    let quadratic = denominator_quadratic(family, denominator)?;
    let transformed = congruence_symbolic_matrix(
        family.coefficient_context(),
        transform,
        &quadratic,
        family.construction_limits(),
    )?;
    Ok(AffineDenominator::new(
        denominator.constant().clone(),
        scalar_coefficients_from_quadratic(family, &transformed)?,
    ))
}

fn denominator_quadratic(
    family: &IntegralFamily,
    denominator: &AffineDenominator,
) -> Result<Vec<Vec<Coefficient>>, Box<dyn Error>> {
    let context = family.coefficient_context();
    let mut matrix = vec![vec![context.zero(); family.loop_count()]; family.loop_count()];
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                matrix[left][right] = coefficient.clone();
            }
            ScalarProductCoordinate::LoopLoop { left, right } => {
                let half = context.try_div(
                    coefficient,
                    &context.integer(2),
                    family.construction_limits().exact_algebra,
                )?;
                matrix[left][right] = half.clone();
                matrix[right][left] = half;
            }
            ScalarProductCoordinate::LoopExternal { .. } => {
                return Err(
                    "the matcher-chart routing fixture received external kinematics".into(),
                );
            }
        }
    }
    Ok(matrix)
}

fn scalar_coefficients_from_quadratic(
    family: &IntegralFamily,
    quadratic: &[Vec<Coefficient>],
) -> Result<Vec<Coefficient>, Box<dyn Error>> {
    let context = family.coefficient_context();
    family
        .coordinates()
        .iter()
        .map(|coordinate| match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } if left == right => {
                Ok(quadratic[left][right].clone())
            }
            ScalarProductCoordinate::LoopLoop { left, right } => Ok(context.try_mul(
                &context.integer(2),
                &quadratic[left][right],
                family.construction_limits().exact_algebra,
            )?),
            ScalarProductCoordinate::LoopExternal { .. } => {
                Err("the matcher-chart routing fixture received external kinematics".into())
            }
        })
        .collect()
}

fn relations_in_parent_denominators(
    parent: &IntegralFamily,
    forms: &[AffineDenominator],
) -> Result<Box<[ParentAffineRelation]>, Box<dyn Error>> {
    let context = parent.coefficient_context();
    let arity = parent.denominator_count();
    let affine_size = arity
        .checked_add(1)
        .ok_or("matcher-chart affine basis size overflow")?;
    let constants = parent
        .denominators()
        .iter()
        .map(|denominator| vec![denominator.constant().clone()])
        .collect::<Vec<_>>();
    let limits = symbolica_matrix_limits(parent.construction_limits());
    let (inverse_times_constants, _) =
        multiply_coefficient_matrices(context, parent.inverse_basis(), &constants, limits)?;

    let mut affine_inverse = vec![vec![context.zero(); affine_size]; affine_size];
    affine_inverse[0][0] = context.one();
    for row in 0..arity {
        affine_inverse[row + 1][0] = context.try_neg(
            &inverse_times_constants[row][0],
            parent.construction_limits().exact_algebra,
        )?;
        for column in 0..arity {
            affine_inverse[row + 1][column + 1] = parent.inverse_basis()[row][column].clone();
        }
    }

    let form_rows = forms
        .iter()
        .map(|form| {
            std::iter::once(form.constant().clone())
                .chain(form.coefficients().iter().cloned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let (relation_rows, _) =
        multiply_coefficient_matrices(context, &form_rows, &affine_inverse, limits)?;
    relation_rows
        .into_iter()
        .map(|mut row| {
            if row.len() != affine_size {
                return Err("Symbolica returned a malformed matcher-chart affine relation".into());
            }
            let denominator_coefficients = row.split_off(1).into_boxed_slice();
            let constant = row
                .pop()
                .ok_or("Symbolica returned an empty matcher-chart affine relation")?;
            Ok(ParentAffineRelation {
                constant,
                denominator_coefficients,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()
        .map(Vec::into_boxed_slice)
}

fn replay_parent_relations(
    parent: &IntegralFamily,
    expected: &[AffineDenominator],
    relations: &[ParentAffineRelation],
) -> Result<(), Box<dyn Error>> {
    if expected.len() != relations.len() {
        return Err("matcher-chart affine replay row count mismatch".into());
    }
    let context = parent.coefficient_context();
    let arity = parent.denominator_count();
    let affine_size = arity
        .checked_add(1)
        .ok_or("matcher-chart affine replay size overflow")?;
    let relation_rows = relations
        .iter()
        .map(|relation| {
            std::iter::once(relation.constant.clone())
                .chain(relation.denominator_coefficients.iter().cloned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut parent_affine = vec![vec![context.zero(); affine_size]; affine_size];
    parent_affine[0][0] = context.one();
    for (row, denominator) in parent.denominators().iter().enumerate() {
        parent_affine[row + 1][0] = denominator.constant().clone();
        for (column, coefficient) in denominator.coefficients().iter().enumerate() {
            parent_affine[row + 1][column + 1] = coefficient.clone();
        }
    }
    let (replayed, _) = multiply_coefficient_matrices(
        context,
        &relation_rows,
        &parent_affine,
        symbolica_matrix_limits(parent.construction_limits()),
    )?;
    for (row, (actual, expected)) in replayed.iter().zip(expected).enumerate() {
        for (component, (actual, expected)) in actual
            .iter()
            .zip(std::iter::once(expected.constant()).chain(expected.coefficients()))
            .enumerate()
        {
            if !coefficients_equal(parent, actual, expected)? {
                return Err(format!(
                    "matcher-chart affine replay failed at row {row}, component {component}"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn verify_stable_physical_slots(
    parent: &IntegralFamily,
    physical_parent_slots: &[usize],
    relations: &[ParentAffineRelation],
) -> Result<(), Box<dyn Error>> {
    let context = parent.coefficient_context();
    for (local_slot, &parent_slot) in physical_parent_slots.iter().enumerate() {
        let relation = &relations[local_slot];
        if !coefficients_equal(parent, &relation.constant, &context.zero())? {
            return Err(format!(
                "routed physical slot {local_slot} acquired a nonzero affine constant"
            )
            .into());
        }
        for (candidate, coefficient) in relation.denominator_coefficients.iter().enumerate() {
            let expected = if candidate == parent_slot {
                context.one()
            } else {
                context.zero()
            };
            if !coefficients_equal(parent, coefficient, &expected)? {
                return Err(format!(
                    "routed physical slot {local_slot} did not replay to stable parent slot {parent_slot}"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn coefficients_equal(
    family: &IntegralFamily,
    left: &Coefficient,
    right: &Coefficient,
) -> Result<bool, Box<dyn Error>> {
    if left == right {
        Ok(true)
    } else {
        Ok(family
            .coefficient_context()
            .try_sub(left, right, family.construction_limits().exact_algebra)?
            .is_zero())
    }
}
