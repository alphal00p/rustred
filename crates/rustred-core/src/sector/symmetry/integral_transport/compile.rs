use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::sector::Mask;
use crate::sector::symmetry::{DenominatorAction, Jacobian, VerifiedMap};

use super::{Error, ExpansionError, ExpansionLimits, Prepared};

/// Bind an exact verified map to its families and a unit active-row bijection.
///
/// Rational-constant affine inactive rows, zero analytic shifts and an
/// unconditional unit Jacobian are the initial supported lane. Unsupported
/// maps fail explicitly; no kinematic conditions are silently discarded.
pub fn compile(
    source: &IntegralFamily,
    target: Arc<IntegralFamily>,
    map: Arc<VerifiedMap>,
    source_root: Mask,
    target_root: Mask,
    limits: ExpansionLimits,
) -> Result<Prepared, Error> {
    for (side, actual, expected) in [
        (
            "source",
            source.fingerprint(),
            map.source_family_fingerprint(),
        ),
        (
            "target",
            target.fingerprint(),
            map.target_family_fingerprint(),
        ),
    ] {
        if actual != expected {
            return Err(Error::FamilyMismatch { side });
        }
    }
    if !source
        .coefficient_context()
        .has_same_variable_map(target.coefficient_context())
    {
        return Err(Error::ForeignCoefficientContext);
    }
    if source.dimension() != target.dimension() {
        return Err(Error::DimensionMismatch);
    }
    for (side, root, family) in [
        ("source", &source_root, source),
        ("target", &target_root, target.as_ref()),
    ] {
        if root.arity() != family.denominator_count() {
            return Err(Error::RootArity {
                side,
                expected: family.denominator_count(),
                actual: root.arity(),
            });
        }
        if let Some(axis) = family
            .power_shifts()
            .iter()
            .position(|shift| !shift.is_zero())
        {
            return Err(Error::AnalyticPowerShift { side, axis });
        }
    }
    if !matches!(map.jacobian(), Jacobian::Unit { .. }) {
        return Err(Error::NonUnitJacobian);
    }
    for (ordinal, condition) in map.nonzero_conditions().iter().enumerate() {
        let polynomial = condition.polynomial();
        if !polynomial.is_constant() || polynomial.is_zero() {
            return Err(Error::UnresolvedCondition { ordinal });
        }
    }
    let arity = source.denominator_count();
    let target_arity = target.denominator_count();
    let entries = arity
        .checked_mul(
            target_arity
                .checked_add(1)
                .ok_or_else(|| overflow("transport map entries"))?,
        )
        .ok_or_else(|| overflow("transport map entries"))?;
    admit(
        "transport map entries",
        entries,
        limits.max_relation_coefficient_entries,
    )?;
    admit(
        "transport source axes",
        arity,
        limits.max_endpoint_power_entries,
    )?;
    admit(
        "transport target axes",
        target_arity,
        limits.max_endpoint_power_entries,
    )?;
    let mut active_target = reserved(arity, "transport active map")?;
    active_target.resize(arity, None);
    let mut seen = reserved(target_arity, "transport target active map")?;
    seen.resize(target_arity, false);
    let one = target.coefficient_context().one();
    for row in 0..arity {
        target
            .coefficient_context()
            .validate_with_limits(&map.denominators().constant()[row], limits.exact_algebra)
            .map_err(ExpansionError::ExactAlgebra)?;
        if !map.denominators().constant()[row].is_constant() {
            return Err(Error::NonconstantCoefficient { row, column: None });
        }
        for column in 0..target_arity {
            let coefficient = map
                .denominators()
                .linear()
                .get(row, column)
                .expect("verified matrix shape");
            target
                .coefficient_context()
                .validate_with_limits(coefficient, limits.exact_algebra)
                .map_err(ExpansionError::ExactAlgebra)?;
            if !coefficient.is_constant() {
                return Err(Error::NonconstantCoefficient {
                    row,
                    column: Some(column),
                });
            }
        }
        if source_root.active_bits()[row] {
            let DenominatorAction::Monomial {
                target: axis,
                scale,
            } = &map.row_actions()[row]
            else {
                return Err(Error::NonUnitActiveRow { axis: row });
            };
            if scale != &one {
                return Err(Error::NonUnitActiveRow { axis: row });
            }
            if !target_root.active_bits()[*axis] || seen[*axis] {
                return Err(Error::ActiveBijection);
            }
            seen[*axis] = true;
            active_target[row] = Some(*axis);
        }
    }
    if seen.as_slice() != target_root.active_bits() {
        return Err(Error::ActiveBijection);
    }
    Ok(Prepared {
        source_fingerprint: source.fingerprint_owner(),
        target,
        map,
        source_root,
        target_root,
        active_target: active_target.into_boxed_slice(),
    })
}

pub(super) fn admit(resource: &'static str, requested: usize, limit: usize) -> Result<(), Error> {
    if requested > limit {
        Err(ExpansionError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        .into())
    } else {
        Ok(())
    }
}

pub(super) fn overflow(resource: &'static str) -> Error {
    ExpansionError::ResourceCountOverflow { resource }.into()
}

pub(super) fn reserved<T>(count: usize, resource: &'static str) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ExpansionError::AllocationFailure {
            resource,
            requested: count,
        })?;
    Ok(values)
}
