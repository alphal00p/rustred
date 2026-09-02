use crate::algebra::IndexedCoefficientContext;
use crate::algebra::IndexedPolynomial;
use crate::foundry::completion::frame::exact::{ClearedExactCircuit, ExactTargetCircuit};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, TargetColumnPartition,
};

use super::{
    ExactGuardRefinement, ExactGuardRefinementError, ExactGuardRefinementLimits,
    ExactGuardRefinementOutcome, ExceptionalGuardStratum, RequiredGuardPredicate,
};

const CIRCUIT_GUARDS: &str = "exact guard refinement circuit guards";
const CIRCUIT_GUARD_IDENTITY_BYTES: &str = "exact guard refinement circuit guard identity bytes";
const UNIQUE_PREDICATES: &str = "exact guard refinement unique predicates";
const GUARD_ORDINAL_REFERENCES: &str = "exact guard refinement guard-ordinal references";
const EXCEPTIONAL_STRATA: &str = "exact guard refinement exceptional strata";
const RESULT_BRANCH_REFERENCES: &str = "exact guard refinement result guard-branch references";
const RESULT_IDENTITY_BYTES: &str = "exact guard refinement result stratum identity bytes";

/// Refine one exact circuit's parent into its sole all-nonzero application
/// child and a disjoint ordered collection of first-zero obligations.
pub(crate) fn try_refine_exact_circuit_guards(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    partition: &TargetColumnPartition<'_>,
    limits: ExactGuardRefinementLimits,
) -> Result<ExactGuardRefinementOutcome, ExactGuardRefinementError> {
    try_refine_guard_polynomials(
        context,
        circuit,
        circuit
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial()),
        partition,
        limits,
    )
}

/// Refine only the source/family and final-target predicates certified by a
/// fraction-free replay of this precise exact circuit.
pub(crate) fn try_refine_cleared_exact_circuit_guards(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    cleared: &ClearedExactCircuit,
    partition: &TargetColumnPartition<'_>,
    limits: ExactGuardRefinementLimits,
) -> Result<ExactGuardRefinementOutcome, ExactGuardRefinementError> {
    if !cleared.is_bound_to(circuit) {
        return Err(ExactGuardRefinementError::ClearedCircuitMismatch);
    }
    try_refine_guard_polynomials(
        context,
        circuit,
        cleared
            .semantic_guards()
            .iter()
            .map(|guard| guard.polynomial()),
        partition,
        limits,
    )
}

fn try_refine_guard_polynomials<'guard>(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    guard_polynomials: impl ExactSizeIterator<Item = &'guard IndexedPolynomial>,
    partition: &TargetColumnPartition<'_>,
    limits: ExactGuardRefinementLimits,
) -> Result<ExactGuardRefinementOutcome, ExactGuardRefinementError> {
    validate_join(context, circuit, partition)?;
    let guard_count = guard_polynomials.len();
    check_limit(CIRCUIT_GUARDS, guard_count, limits.max_circuit_guards)?;
    check_limit(
        GUARD_ORDINAL_REFERENCES,
        guard_count,
        limits.max_guard_ordinal_references,
    )?;

    let mut grouped: Vec<(GuardBranchIdentity, Vec<usize>)> =
        try_vec(UNIQUE_PREDICATES, guard_count)?;
    let mut circuit_guard_identity_bytes = 0usize;
    for (guard_ordinal, polynomial) in guard_polynomials.enumerate() {
        let identity = try_circuit_guard_identity(
            context,
            polynomial,
            &mut circuit_guard_identity_bytes,
            limits,
        )?;
        if let Some(group_ordinal) = grouped
            .iter()
            .position(|(candidate, _)| candidate == &identity)
        {
            let ordinals = &mut grouped
                .get_mut(group_ordinal)
                .ok_or(ExactGuardRefinementError::Invariant {
                    detail: "guard identity index is outside its unique-predicate table",
                })?
                .1;
            try_reserve(ordinals, 1, GUARD_ORDINAL_REFERENCES)?;
            ordinals.push(guard_ordinal);
            continue;
        }
        let requested = checked_add(UNIQUE_PREDICATES, grouped.len(), 1)?;
        check_limit(UNIQUE_PREDICATES, requested, limits.max_unique_predicates)?;
        let mut ordinals = try_vec(GUARD_ORDINAL_REFERENCES, 1)?;
        ordinals.push(guard_ordinal);
        grouped.push((identity, ordinals));
    }

    let mut required = try_vec(UNIQUE_PREDICATES, grouped.len())?;
    for (identity, ordinals) in grouped {
        required.push(RequiredGuardPredicate::new(identity, ordinals));
    }

    let parent = partition.stratum();
    let mut newly_split = try_vec(UNIQUE_PREDICATES, required.len())?;
    for (required_ordinal, predicate) in required.iter().enumerate() {
        let existing = parent
            .guards()
            .iter()
            .find(|existing| existing.same_predicate(predicate.nonzero_branch()));
        match existing.map(GuardBranchIdentity::branch) {
            Some(GuardBranch::NonZero) => {}
            Some(GuardBranch::Zero) => {
                let first_circuit_guard_ordinal = *predicate
                    .circuit_guard_ordinals()
                    .first()
                    .ok_or(ExactGuardRefinementError::Invariant {
                        detail: "a required predicate retained no circuit guard ordinal",
                    })?;
                return Ok(ExactGuardRefinementOutcome::BlockedByKnownZero {
                    required_predicate_ordinal: required_ordinal,
                    first_circuit_guard_ordinal,
                    zero_branch: predicate.nonzero_branch().with_branch(GuardBranch::Zero),
                });
            }
            None => newly_split.push(required_ordinal),
        }
    }

    preflight_result_shape(parent.guards().len(), newly_split.len(), limits)?;
    let mut prefix = try_vec(
        RESULT_BRANCH_REFERENCES,
        checked_add(
            RESULT_BRANCH_REFERENCES,
            parent.guards().len(),
            newly_split.len(),
        )?,
    )?;
    prefix.extend(parent.guards().iter().cloned());
    let mut exceptional = try_vec(EXCEPTIONAL_STRATA, newly_split.len())?;
    let mut result_identity_bytes = 0usize;

    for &required_ordinal in &newly_split {
        let predicate = required
            .get(required_ordinal)
            .ok_or(ExactGuardRefinementError::Invariant {
                detail: "new guard ordinal is outside its required-predicate table",
            })?
            .nonzero_branch();
        let mut child_guards = try_vec(
            RESULT_BRANCH_REFERENCES,
            checked_add(RESULT_BRANCH_REFERENCES, prefix.len(), 1)?,
        )?;
        child_guards.extend(prefix.iter().cloned());
        child_guards.push(predicate.with_branch(GuardBranch::Zero));
        let child = try_result_stratum(parent, child_guards, &mut result_identity_bytes, limits)?;
        exceptional.push(ExceptionalGuardStratum::new(required_ordinal, child));
        prefix.push(predicate.clone());
    }

    let admitted = try_result_stratum(parent, prefix, &mut result_identity_bytes, limits)?;

    Ok(ExactGuardRefinementOutcome::Admitted(
        ExactGuardRefinement::from_parts(
            parent.id().clone(),
            required,
            newly_split,
            admitted,
            exceptional,
        ),
    ))
}

fn validate_join(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    partition: &TargetColumnPartition<'_>,
) -> Result<(), ExactGuardRefinementError> {
    if context.fingerprint() != partition.frame().context_fingerprint() {
        return Err(ExactGuardRefinementError::WrongContext);
    }
    // `TargetColumnPartition` is crate-private, has no unchecked constructor,
    // and was already cold-verified by `try_new`.  Reconstructing its full
    // frame and owner census for every candidate would multiply K=6 promotion
    // cost without increasing authority.  This boundary therefore rejoins
    // only the circuit-dependent identities below.
    if circuit.stratum_id() != partition.stratum_id() {
        return Err(ExactGuardRefinementError::CircuitStratumMismatch);
    }
    if !circuit
        .fixed_indices()
        .iter()
        .copied()
        .eq(partition.stratum().singleton_index_assignments())
    {
        return Err(ExactGuardRefinementError::CircuitStratumMismatch);
    }
    if circuit.owner_snapshot_id() != partition.snapshot_id() {
        return Err(ExactGuardRefinementError::CircuitOwnerSnapshotMismatch);
    }
    if circuit.target_column() != partition.target_column()
        || circuit.modular_diagnostics().target_column != partition.target_column()
        || circuit.modular_diagnostics().forbidden_columns.as_ref() != partition.forbidden_columns()
    {
        return Err(ExactGuardRefinementError::CircuitTargetMismatch);
    }
    if partition.frame().columns().get(partition.target_column()) != Some(circuit.target_shift()) {
        return Err(ExactGuardRefinementError::CircuitTargetShiftMismatch);
    }
    Ok(())
}

fn preflight_result_shape(
    parent_guards: usize,
    new_guards: usize,
    limits: ExactGuardRefinementLimits,
) -> Result<(), ExactGuardRefinementError> {
    check_limit(
        EXCEPTIONAL_STRATA,
        new_guards,
        limits.max_exceptional_strata,
    )?;
    let exceptional_parent = checked_mul(RESULT_BRANCH_REFERENCES, new_guards, parent_guards)?;
    let triangular = checked_mul(
        RESULT_BRANCH_REFERENCES,
        new_guards,
        checked_add(RESULT_BRANCH_REFERENCES, new_guards, 1)?,
    )? / 2;
    let admitted = checked_add(RESULT_BRANCH_REFERENCES, parent_guards, new_guards)?;
    let total = checked_add(
        RESULT_BRANCH_REFERENCES,
        checked_add(RESULT_BRANCH_REFERENCES, exceptional_parent, triangular)?,
        admitted,
    )?;
    check_limit(
        RESULT_BRANCH_REFERENCES,
        total,
        limits.max_result_guard_branch_references,
    )
}

fn try_circuit_guard_identity(
    context: &IndexedCoefficientContext,
    polynomial: &crate::algebra::IndexedPolynomial,
    charged: &mut usize,
    limits: ExactGuardRefinementLimits,
) -> Result<GuardBranchIdentity, ExactGuardRefinementError> {
    let remaining = remaining_budget(
        CIRCUIT_GUARD_IDENTITY_BYTES,
        limits.max_circuit_guard_identity_bytes,
        *charged,
    )?;
    let mut identity_limits = limits.strata;
    identity_limits.max_guard_identity_bytes =
        identity_limits.max_guard_identity_bytes.min(remaining);
    let identity = GuardBranchIdentity::try_from_indexed_polynomial(
        context,
        polynomial,
        GuardBranch::NonZero,
        limits.exact_algebra,
        identity_limits,
    )
    .map_err(|error| {
        remap_inner_byte_limit(
            error,
            "guard predicate identity bytes",
            CIRCUIT_GUARD_IDENTITY_BYTES,
            *charged,
            remaining,
            limits.max_circuit_guard_identity_bytes,
        )
    })?;
    *charged = checked_add(
        CIRCUIT_GUARD_IDENTITY_BYTES,
        *charged,
        identity.predicate().len(),
    )?;
    check_limit(
        CIRCUIT_GUARD_IDENTITY_BYTES,
        *charged,
        limits.max_circuit_guard_identity_bytes,
    )?;
    Ok(identity)
}

fn try_result_stratum(
    parent: &DecoratedStratum,
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
    charged_identity_bytes: &mut usize,
    limits: ExactGuardRefinementLimits,
) -> Result<DecoratedStratum, ExactGuardRefinementError> {
    let remaining = remaining_budget(
        RESULT_IDENTITY_BYTES,
        limits.max_result_stratum_identity_bytes,
        *charged_identity_bytes,
    )?;
    let mut stratum_limits = limits.strata;
    stratum_limits.max_stratum_identity_bytes =
        stratum_limits.max_stratum_identity_bytes.min(remaining);
    let stratum = DecoratedStratum::try_new(
        parent.family_fingerprint(),
        parent.context_fingerprint(),
        parent.domain().clone(),
        guards,
        stratum_limits,
    )
    .map_err(|error| {
        remap_inner_byte_limit(
            error,
            "decorated-stratum identity bytes",
            RESULT_IDENTITY_BYTES,
            *charged_identity_bytes,
            remaining,
            limits.max_result_stratum_identity_bytes,
        )
    })?;
    *charged_identity_bytes = checked_add(
        RESULT_IDENTITY_BYTES,
        *charged_identity_bytes,
        stratum.id().as_str().len(),
    )?;
    check_limit(
        RESULT_IDENTITY_BYTES,
        *charged_identity_bytes,
        limits.max_result_stratum_identity_bytes,
    )?;
    Ok(stratum)
}

fn remap_inner_byte_limit(
    error: crate::foundry::completion::stratum::StratumRegistryError,
    inner_resource: &'static str,
    aggregate_resource: &'static str,
    charged: usize,
    remaining: usize,
    aggregate_limit: usize,
) -> ExactGuardRefinementError {
    match error {
        crate::foundry::completion::stratum::StratumRegistryError::ResourceLimit {
            resource,
            requested,
            limit,
        } if resource == inner_resource && limit == remaining => {
            match charged.checked_add(requested) {
                Some(requested) => ExactGuardRefinementError::ResourceLimit {
                    resource: aggregate_resource,
                    requested,
                    limit: aggregate_limit,
                },
                None => ExactGuardRefinementError::ResourceCountOverflow {
                    resource: aggregate_resource,
                },
            }
        }
        error => ExactGuardRefinementError::Stratum(error),
    }
}

fn remaining_budget(
    resource: &'static str,
    limit: usize,
    charged: usize,
) -> Result<usize, ExactGuardRefinementError> {
    limit
        .checked_sub(charged)
        .ok_or(ExactGuardRefinementError::ResourceLimit {
            resource,
            requested: charged,
            limit,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactGuardRefinementError> {
    left.checked_add(right)
        .ok_or(ExactGuardRefinementError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactGuardRefinementError> {
    left.checked_mul(right)
        .ok_or(ExactGuardRefinementError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactGuardRefinementError> {
    if requested > limit {
        Err(ExactGuardRefinementError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ExactGuardRefinementError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ExactGuardRefinementError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactGuardRefinementError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ExactGuardRefinementError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
