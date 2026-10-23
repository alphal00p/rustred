//! Cheap signed-line proposals; exact native maps remain the only authority.

use super::*;
use symbolica::prelude::{Matrix, PolyVariable, Q, Rational};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::sector::symmetry::CoefficientMatrix;

type NativeMatrix = Matrix<symbolica::domains::rational::RationalField>;
type Signature = Vec<(usize, i64)>;

pub(super) fn prepare(
    family: Arc<IntegralFamily>,
    keys: &BTreeSet<IntegralKey>,
    limits: TerminalNumeratorSymmetryLimits,
) -> Result<TerminalNumeratorSymmetryEquations, Error> {
    for key in keys {
        validate_key(&family, key)?;
    }
    let mut plan = TerminalNumeratorSymmetryEquations {
        family_fingerprint: family.fingerprint_owner(),
        input_keys: keys.clone(),
        rows: BTreeMap::new(),
        witnesses: BTreeMap::new(),
        statistics: TerminalNumeratorSymmetryStatistics {
            input_keys: keys.len(),
            ..Default::default()
        },
    };
    let family_skip = if family.external_count() != 0 {
        Some(TerminalNumeratorSymmetrySkipReason::ExternalMomenta)
    } else if family.power_shifts().iter().any(|shift| !shift.is_zero()) {
        Some(TerminalNumeratorSymmetrySkipReason::AnalyticPowerShifts)
    } else {
        None
    };
    if let Some(reason) = family_skip {
        plan.statistics.skipped.insert(reason, keys.len());
        return Ok(plan);
    }
    let variables = Arc::new(
        (0..family.loop_count())
            .map(PolyVariable::Temporary)
            .collect(),
    );
    let mut momenta = vec![None; family.denominator_count()];
    let mut supports: BTreeMap<Signature, Vec<Arc<VerifiedMap>>> = BTreeMap::new();
    for key in keys {
        if !key.powers().iter().any(|&power| power < 0) {
            skip(&mut plan, TerminalNumeratorSymmetrySkipReason::NoNumerator);
            continue;
        }
        let signature: Signature = key
            .powers()
            .iter()
            .enumerate()
            .filter_map(|(slot, &power)| (power > 0).then_some((slot, power)))
            .collect();
        if !supports.contains_key(&signature) {
            plan.statistics.analyzed_supports += 1;
            let maps = prepare_support(
                &family,
                &signature,
                &variables,
                &mut momenta,
                &mut plan.statistics,
                limits,
            )?;
            supports.insert(signature.clone(), maps);
        }
        let maps = &supports[&signature];
        if maps.is_empty() {
            skip(
                &mut plan,
                TerminalNumeratorSymmetrySkipReason::NoProposedSymmetry,
            );
        }
        let mut rows = Vec::new();
        let mut witnesses = Vec::new();
        for map in maps {
            let prepared = prepare_weighted(&family, key, Arc::clone(map), limits)?;
            let equation = equation(&family, key, &prepared, limits)?;
            if equation.terms.is_empty() {
                plan.statistics.trivial_equations += 1;
                continue;
            }
            // Coalesced exact rows are compared before retaining/pivoting.
            if rows.contains(&equation) {
                continue;
            }
            plan.statistics.output_terms = plan
                .statistics
                .output_terms
                .checked_add(equation.terms.len())
                .ok_or(Error::InvalidWitness(
                    "numerator symmetry output count overflow",
                ))?;
            plan.statistics.equations += 1;
            witnesses.push(TerminalNumeratorSymmetryWitness {
                family: family.fingerprint_owner(),
                source: key.clone(),
                map: Arc::clone(map),
                equation: equation.clone(),
            });
            rows.push(equation);
        }
        if !rows.is_empty() {
            plan.statistics.equation_targets += 1;
            plan.rows.insert(key.clone(), rows);
            plan.witnesses.insert(key.clone(), witnesses);
        }
    }
    Ok(plan)
}

fn prepare_support(
    family: &Arc<IntegralFamily>,
    signature: &Signature,
    variables: &Arc<Vec<PolyVariable>>,
    momenta: &mut [Option<Result<Vec<Coefficient>, ProductSkipReason>>],
    statistics: &mut TerminalNumeratorSymmetryStatistics,
    limits: TerminalNumeratorSymmetryLimits,
) -> Result<Vec<Arc<VerifiedMap>>, Error> {
    let mut rows = Vec::new();
    for &(slot, _) in signature {
        if momenta[slot].is_none() {
            statistics.analyzed_denominators += 1;
            momenta[slot] = Some(super::super::products::momentum_quadratic(
                family, slot, variables,
            )?);
        }
        match momenta[slot].as_ref().expect("cached proposal") {
            Ok(row) => rows.push(
                row.iter()
                    .map(|value| rational(value, family.coefficient_context()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Err(reason) => {
                *statistics
                    .skipped
                    .entry(TerminalNumeratorSymmetrySkipReason::MomentumGeometry(
                        *reason,
                    ))
                    .or_default() += 1;
                return Ok(Vec::new());
            }
        }
    }
    let loops = family.loop_count();
    if rows.len() < loops || loops == 0 {
        *statistics
            .skipped
            .entry(TerminalNumeratorSymmetrySkipReason::RankDeficientSupport)
            .or_default() += 1;
        return Ok(Vec::new());
    }
    let all = matrix(&rows)?;
    let mut transpose = all.transpose();
    let rank = transpose.row_reduce(u32::try_from(rows.len()).map_err(algebra)?);
    if rank != loops {
        *statistics
            .skipped
            .entry(TerminalNumeratorSymmetrySkipReason::RankDeficientSupport)
            .or_default() += 1;
        return Ok(Vec::new());
    }
    // Leading columns of native RREF identify independent original line rows.
    let basis_slots: Vec<_> = transpose
        .row_iter()
        .filter_map(|row| row.iter().position(|value| !value.is_zero()))
        .collect();
    let basis = matrix(
        &basis_slots
            .iter()
            .map(|&slot| rows[slot].clone())
            .collect::<Vec<_>>(),
    )?;
    let inverse = basis.inv().map_err(algebra)?;
    let mut maps = Vec::new();
    // A fixed signed-line transposition leaves other lines fixed. This is a
    // useful, finite generating heuristic, not full automorphism enumeration.
    for left in 0..rows.len() {
        let mut reflected = rows.clone();
        reflected[left] = rows[left].iter().map(|value| -value.clone()).collect();
        propose(
            family,
            signature,
            &all,
            &rows,
            &basis_slots,
            &inverse,
            &reflected,
            &mut maps,
            statistics,
            limits,
        )?;
        for right in left + 1..rows.len() {
            if signature[left].1 != signature[right].1 {
                statistics.rejected_power_permutations += 1;
                continue;
            }
            // The true masses remain in the verified denominator equations;
            // this exact comparison is only an inexpensive proposal filter.
            if family.denominators()[signature[left].0].constant()
                != family.denominators()[signature[right].0].constant()
            {
                continue;
            }
            for left_sign in [-1, 1] {
                for right_sign in [-1, 1] {
                    let mut targets = rows.clone();
                    targets[left] = rows[right]
                        .iter()
                        .map(|value| {
                            if left_sign < 0 {
                                -value.clone()
                            } else {
                                value.clone()
                            }
                        })
                        .collect();
                    targets[right] = rows[left]
                        .iter()
                        .map(|value| {
                            if right_sign < 0 {
                                -value.clone()
                            } else {
                                value.clone()
                            }
                        })
                        .collect();
                    propose(
                        family,
                        signature,
                        &all,
                        &rows,
                        &basis_slots,
                        &inverse,
                        &targets,
                        &mut maps,
                        statistics,
                        limits,
                    )?;
                }
            }
        }
    }
    Ok(maps)
}

#[allow(clippy::too_many_arguments)]
fn propose(
    family: &Arc<IntegralFamily>,
    signature: &Signature,
    all: &NativeMatrix,
    original_rows: &[Vec<Rational>],
    basis_slots: &[usize],
    inverse: &NativeMatrix,
    targets: &[Vec<Rational>],
    maps: &mut Vec<Arc<VerifiedMap>>,
    statistics: &mut TerminalNumeratorSymmetryStatistics,
    limits: TerminalNumeratorSymmetryLimits,
) -> Result<(), Error> {
    statistics.proposed_generators += 1;
    let target_basis = matrix(
        &basis_slots
            .iter()
            .map(|&slot| targets[slot].clone())
            .collect::<Vec<_>>(),
    )?;
    let a = inverse * &target_basis;
    // Native exact multiplication checks dependent rows before the more
    // expensive generic family-map authority pipeline.
    if &(all * &a) != &matrix(targets)? || original_rows == targets {
        return Ok(());
    }
    let context = family.coefficient_context();
    let entries = a
        .into_vec()
        .iter()
        .map(|value| coefficient(value, context))
        .collect::<Result<Vec<_>, _>>()?;
    let loops = family.loop_count();
    let proposal = MomentumMap::new(
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            loops,
            entries,
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            loops,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
        CoefficientMatrix::try_new_with_max_entries(
            0,
            0,
            [],
            limits.verification.max_matrix_entries,
        )
        .map_err(algebra)?,
    );
    if maps.iter().any(|map| map.momentum() == &proposal) {
        return Ok(());
    }
    let verified =
        Arc::new(symmetry::verify(family, family, proposal, limits.verification).map_err(algebra)?);
    if !matches!(verified.jacobian(), Jacobian::Unit { .. }) {
        return Ok(());
    }
    for &(slot, power) in signature {
        let DenominatorAction::Monomial { target, scale } = &verified.row_actions()[slot] else {
            return Ok(());
        };
        if !scale.is_one()
            || !signature
                .iter()
                .any(|&(other, other_power)| other == *target && other_power == power)
        {
            return Ok(());
        }
    }
    // This independently enforces zero analytic shifts, active bijection,
    // rational affine numerator coefficients and unconditional unit measure.
    let mut source = vec![0; family.denominator_count()];
    for &(slot, power) in signature {
        source[slot] = power;
    }
    prepare_weighted(
        family,
        &IntegralKey::try_new(source).map_err(algebra)?,
        Arc::clone(&verified),
        limits,
    )?;
    statistics.verified_generators += 1;
    maps.push(verified);
    Ok(())
}

fn matrix(rows: &[Vec<Rational>]) -> Result<NativeMatrix, Error> {
    let columns = rows
        .first()
        .ok_or(Error::InvalidWitness("empty symmetry momentum matrix"))?
        .len();
    if rows.iter().any(|row| row.len() != columns) {
        return Err(Error::InvalidWitness("ragged symmetry momentum matrix"));
    }
    Matrix::from_linear(
        rows.iter().flatten().cloned().collect(),
        u32::try_from(rows.len()).map_err(algebra)?,
        u32::try_from(columns).map_err(algebra)?,
        Q,
    )
    .map_err(algebra)
}

fn rational(value: &Coefficient, context: &CoefficientContext) -> Result<Rational, Error> {
    context
        .validate_with_limits(value, ExactAlgebraLimits::unbounded_resources())
        .map_err(algebra)?;
    if !value.numerator.is_constant()
        || !value.denominator.is_constant()
        || value.denominator.is_zero()
    {
        return Err(Error::InvalidWitness(
            "momentum coefficient is not constant rational",
        ));
    }
    Ok(Rational::from((
        value.numerator.get_constant(),
        value.denominator.get_constant(),
    )))
}

fn coefficient(value: &Rational, context: &CoefficientContext) -> Result<Coefficient, Error> {
    let numerator: Coefficient = context
        .zero()
        .numerator
        .constant(value.numerator_ref().clone())
        .into();
    let denominator: Coefficient = context
        .zero()
        .numerator
        .constant(value.denominator_ref().clone())
        .into();
    context
        .try_div(
            &numerator,
            &denominator,
            ExactAlgebraLimits::unbounded_resources(),
        )
        .map_err(algebra)
}

fn skip(
    plan: &mut TerminalNumeratorSymmetryEquations,
    reason: TerminalNumeratorSymmetrySkipReason,
) {
    *plan.statistics.skipped.entry(reason).or_default() += 1;
}
