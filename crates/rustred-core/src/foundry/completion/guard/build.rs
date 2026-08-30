use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::{GuardBranch, GuardBranchIdentity, StratumRegistryError};

use super::{CoefficientIdealGuardAtom, CoefficientIdealGuardError, CoefficientIdealGuardLimits};

const GENERATOR_IDENTITY_BYTES: &str = "coefficient-ideal guard generator identity bytes";

impl CoefficientIdealGuardAtom {
    /// Pull an exact source-coordinate guard back to target coordinates and
    /// compile its simultaneous parameter-coefficient ideal.
    pub(crate) fn try_for_target(
        context: &IndexedCoefficientContext,
        source_guard: IndexedPolynomial,
        target_shift: &[i64],
        limits: CoefficientIdealGuardLimits,
    ) -> Result<Self, CoefficientIdealGuardError> {
        if target_shift.len() != context.index_count() {
            return Err(CoefficientIdealGuardError::IndexedAlgebra(
                crate::algebra::IndexedAlgebraError::WrongIndexArity {
                    expected: context.index_count(),
                    actual: target_shift.len(),
                },
            ));
        }
        let mut pullback = try_i64_vec(target_shift.len())?;
        for (index, &shift) in target_shift.iter().enumerate() {
            pullback.push(
                shift
                    .checked_neg()
                    .ok_or(CoefficientIdealGuardError::TargetPullbackOverflow { index, shift })?,
            );
        }
        let pulled_back =
            context.translate_polynomial(&source_guard, &pullback, limits.indexed_algebra)?;
        Self::try_from_pulled_back(context, pulled_back, limits)
    }

    /// Compile a guard already expressed in target coordinates into the
    /// simultaneous ideal of its index-polynomial coefficients over the
    /// declared algebraically independent base parameters.
    pub(crate) fn try_from_pulled_back(
        context: &IndexedCoefficientContext,
        pulled_back_guard: IndexedPolynomial,
        limits: CoefficientIdealGuardLimits,
    ) -> Result<Self, CoefficientIdealGuardError> {
        if pulled_back_guard.is_zero() {
            return Err(CoefficientIdealGuardError::IdenticallyZeroGuard);
        }
        let coefficient_system = context.base_coefficient_system(
            &pulled_back_guard,
            limits.indexed_algebra,
            limits.guard_algebra,
        )?;
        if coefficient_system.equations().is_empty() {
            return Err(CoefficientIdealGuardError::IdenticallyZeroGuard);
        }

        let mut generators = try_vec(coefficient_system.equations().len())?;
        let mut charged_identity_bytes = 0usize;
        for equation in coefficient_system.equations() {
            let remaining = limits
                .max_generator_identity_bytes
                .checked_sub(charged_identity_bytes)
                .ok_or(CoefficientIdealGuardError::ResourceLimit {
                    resource: GENERATOR_IDENTITY_BYTES,
                    requested: charged_identity_bytes,
                    limit: limits.max_generator_identity_bytes,
                })?;
            let mut identity_limits = limits.predicate_identity;
            identity_limits.max_guard_identity_bytes =
                identity_limits.max_guard_identity_bytes.min(remaining);
            let identity = GuardBranchIdentity::try_from_indexed_polynomial(
                context,
                equation.index_polynomial(),
                GuardBranch::Zero,
                limits.indexed_algebra.exact_algebra,
                identity_limits,
            )
            .map_err(|error| {
                remap_identity_limit(
                    error,
                    charged_identity_bytes,
                    remaining,
                    limits.max_generator_identity_bytes,
                )
            })?;
            charged_identity_bytes = charged_identity_bytes
                .checked_add(identity.predicate().len())
                .ok_or(CoefficientIdealGuardError::ResourceCountOverflow {
                    resource: GENERATOR_IDENTITY_BYTES,
                })?;
            if charged_identity_bytes > limits.max_generator_identity_bytes {
                return Err(CoefficientIdealGuardError::ResourceLimit {
                    resource: GENERATOR_IDENTITY_BYTES,
                    requested: charged_identity_bytes,
                    limit: limits.max_generator_identity_bytes,
                });
            }
            generators.push(identity);
        }
        generators.sort_unstable();
        generators.dedup();
        let has_literal_unit_generator = coefficient_system.has_nonzero_constant_equation();
        Ok(Self::from_parts(
            context.fingerprint_owner(),
            pulled_back_guard,
            coefficient_system,
            generators,
            has_literal_unit_generator,
        ))
    }
}

fn try_i64_vec(capacity: usize) -> Result<Vec<i64>, CoefficientIdealGuardError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        CoefficientIdealGuardError::AllocationFailure {
            resource: "coefficient-ideal target pullback",
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn remap_identity_limit(
    error: StratumRegistryError,
    charged: usize,
    remaining: usize,
    aggregate_limit: usize,
) -> CoefficientIdealGuardError {
    match error {
        StratumRegistryError::ResourceLimit {
            resource: "guard predicate identity bytes",
            requested,
            limit,
        } if limit == remaining => match charged.checked_add(requested) {
            Some(requested) => CoefficientIdealGuardError::ResourceLimit {
                resource: GENERATOR_IDENTITY_BYTES,
                requested,
                limit: aggregate_limit,
            },
            None => CoefficientIdealGuardError::ResourceCountOverflow {
                resource: GENERATOR_IDENTITY_BYTES,
            },
        },
        error => CoefficientIdealGuardError::PredicateIdentity(error),
    }
}

fn try_vec(capacity: usize) -> Result<Vec<GuardBranchIdentity>, CoefficientIdealGuardError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        CoefficientIdealGuardError::AllocationFailure {
            resource: "coefficient-ideal guard generators",
            requested: capacity,
        }
    })?;
    Ok(values)
}
