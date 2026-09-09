use crate::algebra::indexed::IntegerZeroLocusDomainResolution;
use crate::algebra::{IndexedAlgebraError, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::frame::admission::ExactGuardRefinement;
use crate::foundry::completion::frame::exact::{ClearedExactCircuit, ExactTargetCircuit};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, StratumRegistryError,
};
use crate::sector::{InteriorBounds, SectorMonotoneDomain};

use super::super::SpiredCoordinateCaseObligation;
use super::{
    SpiredCoordinateGuardCaseCensus, SpiredCoordinateGuardCaseError,
    SpiredCoordinateGuardCaseIncomplete, SpiredCoordinateGuardCaseLimits,
    SpiredCoordinateGuardCaseOutcome, SpiredCoordinateGuardCaseRejection,
    SpiredCoordinateGuardCases,
};

const REQUIRED_PREDICATES: &str = "SpIReD guard-case required predicates";
const GUARD_ORDINAL_REFERENCES: &str = "SpIReD guard-case guard-ordinal references";
const EXACT_HYPERPLANES: &str = "SpIReD guard-case exact hyperplanes";
const OUTPUT_CASES: &str = "SpIReD guard-case output cases";
const OUTPUT_COORDINATE_CELLS: &str = "SpIReD guard-case output coordinate cells";
const OUTPUT_IDENTITY_BYTES: &str = "SpIReD guard-case output identity bytes";

enum Stop {
    CandidateUnusable(SpiredCoordinateGuardCaseRejection),
    Incomplete(SpiredCoordinateGuardCaseIncomplete),
    Error(SpiredCoordinateGuardCaseError),
}

/// Convert every required exact semantic guard independently into overlapping
/// equality-only coordinate cases.
///
/// `ExactGuardRefinement` remains the exact owner compiler's first-zero input,
/// but its disjoint exceptional strata and nonzero prefixes are deliberately
/// ignored here. Discovery needs the broader equality cases `C & g_i = 0` so
/// generic-first worklist subsumption remains valid.
pub(crate) fn try_materialize_spired_coordinate_guard_cases(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    cleared: &ClearedExactCircuit,
    refinement: &ExactGuardRefinement,
    parent: &SpiredCoordinateCaseObligation,
    limits: SpiredCoordinateGuardCaseLimits,
) -> Result<SpiredCoordinateGuardCaseOutcome, SpiredCoordinateGuardCaseError> {
    match try_materialize(context, circuit, cleared, refinement, parent, limits) {
        Ok(exact) => Ok(SpiredCoordinateGuardCaseOutcome::Exact(exact)),
        Err(Stop::CandidateUnusable(rejection)) => Ok(
            SpiredCoordinateGuardCaseOutcome::CandidateUnusable(rejection),
        ),
        Err(Stop::Incomplete(incomplete)) => {
            Ok(SpiredCoordinateGuardCaseOutcome::Incomplete(incomplete))
        }
        Err(Stop::Error(error)) => Err(error),
    }
}

fn try_materialize(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    cleared: &ClearedExactCircuit,
    refinement: &ExactGuardRefinement,
    parent: &SpiredCoordinateCaseObligation,
    limits: SpiredCoordinateGuardCaseLimits,
) -> Result<SpiredCoordinateGuardCases, Stop> {
    let guard_ordinal_references = validate_join(
        context,
        circuit,
        cleared,
        refinement,
        parent.stratum(),
        limits,
    )?;
    let mut census = SpiredCoordinateGuardCaseCensus {
        required_predicates: refinement.required_predicates().len(),
        guard_ordinal_references,
        ..Default::default()
    };
    let mut cases = try_vec(OUTPUT_CASES, 0)?;

    for (required_ordinal, required) in refinement.required_predicates().iter().enumerate() {
        let guard_ordinal = *required.circuit_guard_ordinals().first().ok_or(
            SpiredCoordinateGuardCaseError::RefinementShapeMismatch {
                detail: "a required predicate retained no semantic guard ordinal",
            },
        )?;
        let guard = cleared.semantic_guards().get(guard_ordinal).ok_or(
            SpiredCoordinateGuardCaseError::GuardOrdinalOutOfRange {
                ordinal: guard_ordinal,
                available: cleared.semantic_guards().len(),
            },
        )?;
        let mut generated = try_materialize_one_guard(
            context,
            guard.polynomial(),
            required_ordinal,
            parent,
            limits,
            &mut census,
        )?;
        try_reserve(&mut cases, generated.len(), OUTPUT_CASES)?;
        cases.append(&mut generated);
        census.resolved_required_predicates =
            checked_add(REQUIRED_PREDICATES, census.resolved_required_predicates, 1)?;
    }

    canonicalize_cases(cases, census, limits)
}

fn validate_join(
    context: &IndexedCoefficientContext,
    circuit: &ExactTargetCircuit,
    cleared: &ClearedExactCircuit,
    refinement: &ExactGuardRefinement,
    parent: &DecoratedStratum,
    limits: SpiredCoordinateGuardCaseLimits,
) -> Result<usize, Stop> {
    require_guard_blind(parent)?;
    if parent.context_fingerprint() != context.fingerprint()
        || parent.domain().arity() != context.index_count()
    {
        return Err(SpiredCoordinateGuardCaseError::WrongContext.into());
    }
    if !cleared.is_bound_to(circuit) {
        return Err(SpiredCoordinateGuardCaseError::ClearedCircuitMismatch.into());
    }
    if cleared.target_column() != circuit.target_column() {
        return Err(SpiredCoordinateGuardCaseError::ClearedTargetMismatch.into());
    }
    if circuit.stratum_id() != parent.id() {
        return Err(SpiredCoordinateGuardCaseError::CircuitParentMismatch.into());
    }
    if refinement.parent_stratum_id() != parent.id() {
        return Err(SpiredCoordinateGuardCaseError::RefinementParentMismatch.into());
    }
    if !circuit
        .fixed_indices()
        .iter()
        .copied()
        .eq(parent.singleton_index_assignments())
    {
        return Err(SpiredCoordinateGuardCaseError::CircuitFixedDomainMismatch.into());
    }
    validate_refined_owner_domain(parent, refinement.admitted_stratum())?;

    check_limit(
        REQUIRED_PREDICATES,
        refinement.required_predicates().len(),
        limits.max_required_predicates,
    )?;
    let guard_count = cleared.semantic_guards().len();
    check_limit(
        GUARD_ORDINAL_REFERENCES,
        guard_count,
        limits.max_guard_ordinal_references,
    )?;
    let mut seen = try_vec(GUARD_ORDINAL_REFERENCES, guard_count)?;
    seen.resize(guard_count, false);
    let mut reference_count = 0usize;
    for (required_ordinal, required) in refinement.required_predicates().iter().enumerate() {
        if required.nonzero_branch().branch() != GuardBranch::NonZero {
            return Err(SpiredCoordinateGuardCaseError::RefinementShapeMismatch {
                detail: "a required predicate is not the nonzero branch",
            }
            .into());
        }
        if required.circuit_guard_ordinals().is_empty() {
            return Err(SpiredCoordinateGuardCaseError::RefinementShapeMismatch {
                detail: "a required predicate retained no semantic guard ordinal",
            }
            .into());
        }
        reference_count = checked_add(
            GUARD_ORDINAL_REFERENCES,
            reference_count,
            required.circuit_guard_ordinals().len(),
        )?;
        check_limit(
            GUARD_ORDINAL_REFERENCES,
            reference_count,
            limits.max_guard_ordinal_references,
        )?;
        for &guard_ordinal in required.circuit_guard_ordinals() {
            let guard = cleared.semantic_guards().get(guard_ordinal).ok_or(
                SpiredCoordinateGuardCaseError::GuardOrdinalOutOfRange {
                    ordinal: guard_ordinal,
                    available: guard_count,
                },
            )?;
            let was_seen = seen.get_mut(guard_ordinal).ok_or(
                SpiredCoordinateGuardCaseError::GuardOrdinalOutOfRange {
                    ordinal: guard_ordinal,
                    available: guard_count,
                },
            )?;
            if *was_seen {
                return Err(SpiredCoordinateGuardCaseError::DuplicateGuardOrdinal {
                    ordinal: guard_ordinal,
                }
                .into());
            }
            *was_seen = true;
            let identity = GuardBranchIdentity::try_from_indexed_polynomial(
                context,
                guard.polynomial(),
                GuardBranch::NonZero,
                limits.indexed_algebra.exact_algebra,
                limits.strata,
            )
            .map_err(classify_stratum_error)?;
            if &identity != required.nonzero_branch() {
                return Err(SpiredCoordinateGuardCaseError::GuardIdentityMismatch {
                    required_predicate_ordinal: required_ordinal,
                    guard_ordinal,
                }
                .into());
            }
        }
    }
    if let Some(ordinal) = seen.iter().position(|seen| !seen) {
        return Err(SpiredCoordinateGuardCaseError::UnreferencedGuardOrdinal { ordinal }.into());
    }
    Ok(reference_count)
}

fn validate_refined_owner_domain(
    parent: &DecoratedStratum,
    admitted: &DecoratedStratum,
) -> Result<(), Stop> {
    if parent.family_fingerprint() != admitted.family_fingerprint()
        || parent.context_fingerprint() != admitted.context_fingerprint()
        || parent.domain() != admitted.domain()
    {
        return Err(SpiredCoordinateGuardCaseError::RefinementShapeMismatch {
            detail: "the admitted owner stratum changed the parent family, context, or coordinate domain",
        }
        .into());
    }
    Ok(())
}

fn require_guard_blind(parent: &DecoratedStratum) -> Result<(), Stop> {
    if parent.guards().is_empty() {
        Ok(())
    } else {
        Err(SpiredCoordinateGuardCaseError::GuardedParentCase {
            guard_branches: parent.guards().len(),
        }
        .into())
    }
}

fn try_materialize_one_guard(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    required_predicate_ordinal: usize,
    parent: &SpiredCoordinateCaseObligation,
    limits: SpiredCoordinateGuardCaseLimits,
    census: &mut SpiredCoordinateGuardCaseCensus,
) -> Result<Vec<SpiredCoordinateCaseObligation>, Stop> {
    let parent_stratum = parent.stratum();
    require_guard_blind(parent_stratum)?;
    let system = context
        .base_coefficient_system(polynomial, limits.indexed_algebra, limits.guard_algebra)
        .map_err(classify_indexed_error)?;
    let bounds = parent_stratum.domain().bounds();
    let resolution = context
        .integer_zero_locus_domain_resolution(&system, limits.guard_algebra, |position, root| {
            bounds
                .get(position)
                .is_some_and(|bound| root.to_i64().is_some_and(|value| bound.contains(value)))
        })
        .map_err(classify_indexed_error)?;

    let roots = match resolution {
        IntegerZeroLocusDomainResolution::IdenticallyZero => {
            return Err(Stop::CandidateUnusable(
                SpiredCoordinateGuardCaseRejection::GuardIdenticallyZero {
                    required_predicate_ordinal,
                },
            ));
        }
        IntegerZeroLocusDomainResolution::MissesDomain => return Ok(Vec::new()),
        IntegerZeroLocusDomainResolution::IntersectsConservativeCover(roots) => {
            return Err(Stop::Incomplete(
                SpiredCoordinateGuardCaseIncomplete::ConservativeCoordinateCover {
                    required_predicate_ordinal,
                    hyperplanes: roots.len(),
                },
            ));
        }
        IntegerZeroLocusDomainResolution::UnsupportedCoupled => {
            return Err(Stop::Incomplete(
                SpiredCoordinateGuardCaseIncomplete::UnsupportedCoupledOrNonAffineGeometry {
                    required_predicate_ordinal,
                },
            ));
        }
        IntegerZeroLocusDomainResolution::IntersectsExactHyperplanes(roots) => roots,
    };

    census.exact_hyperplanes = checked_bounded_add(
        EXACT_HYPERPLANES,
        census.exact_hyperplanes,
        roots.len(),
        limits.max_exact_hyperplanes,
    )?;
    let mut retained_roots = try_vec(EXACT_HYPERPLANES, roots.len())?;
    for root in &roots {
        let position = root.index_position();
        let Some(bound) = bounds.get(position) else {
            return Err(SpiredCoordinateGuardCaseError::RootCoordinateOutOfRange {
                position,
                arity: bounds.len(),
            }
            .into());
        };
        let Some(value) = root.root().to_i64() else {
            return Err(SpiredCoordinateGuardCaseError::RootNotRepresentable { position }.into());
        };
        if !bound.contains(value) {
            return Err(SpiredCoordinateGuardCaseError::Invariant {
                detail: "zero-locus resolver returned a root outside its filtered domain",
            }
            .into());
        }
        retained_roots.push((position, value));
    }
    retained_roots.sort_unstable();
    retained_roots.dedup();

    let prospective_cases = checked_bounded_add(
        OUTPUT_CASES,
        census.output_cases,
        retained_roots.len(),
        limits.max_output_cases,
    )?;
    let coordinate_cells = retained_roots
        .len()
        .checked_mul(bounds.len())
        .and_then(|value| value.checked_mul(2))
        .ok_or(SpiredCoordinateGuardCaseIncomplete::ResourceCountOverflow {
            resource: OUTPUT_COORDINATE_CELLS,
        })?;
    let prospective_coordinate_cells = checked_bounded_add(
        OUTPUT_COORDINATE_CELLS,
        census.output_coordinate_cells,
        coordinate_cells,
        limits.max_output_coordinate_cells,
    )?;
    let mut cases = try_vec(OUTPUT_CASES, retained_roots.len())?;
    let zero_shift = try_zero_shift(bounds.len())?;
    for (position, value) in retained_roots {
        let mut child_bounds = try_copy_bounds(bounds)?;
        child_bounds[position] = InteriorBounds::new(value, value);
        if child_bounds == bounds {
            return Err(Stop::CandidateUnusable(
                SpiredCoordinateGuardCaseRejection::GuardIdenticallyZero {
                    required_predicate_ordinal,
                },
            ));
        }
        let domain = SectorMonotoneDomain::try_new_for_rule(
            parent_stratum.domain().sector().clone(),
            child_bounds,
            &zero_shift,
            &[] as &[&[i64]],
        )
        .map_err(|error| Stop::Error(SpiredCoordinateGuardCaseError::Sector(error)))?;
        let stratum = DecoratedStratum::try_guard_blind(
            parent_stratum.family_fingerprint(),
            parent_stratum.context_fingerprint(),
            domain,
            limits.strata,
        )
        .map_err(classify_stratum_error)?;
        census.output_identity_bytes = checked_bounded_add(
            OUTPUT_IDENTITY_BYTES,
            census.output_identity_bytes,
            stratum.id().as_str().len(),
            limits.max_output_identity_bytes,
        )?;
        let obligation =
            SpiredCoordinateCaseObligation::try_new_child(parent, stratum).map_err(|_| {
                Stop::Error(SpiredCoordinateGuardCaseError::Invariant {
                    detail: "a guard-blind stratum was rejected by the equality-case boundary",
                })
            })?;
        cases.push(obligation);
    }
    census.output_cases = prospective_cases;
    census.output_coordinate_cells = prospective_coordinate_cells;
    Ok(cases)
}

fn canonicalize_cases(
    mut cases: Vec<SpiredCoordinateCaseObligation>,
    mut census: SpiredCoordinateGuardCaseCensus,
    limits: SpiredCoordinateGuardCaseLimits,
) -> Result<SpiredCoordinateGuardCases, Stop> {
    cases.sort_unstable_by(|left, right| {
        left.stratum()
            .domain()
            .bounds()
            .iter()
            .map(|bounds| (bounds.lower(), bounds.upper()))
            .cmp(
                right
                    .stratum()
                    .domain()
                    .bounds()
                    .iter()
                    .map(|bounds| (bounds.lower(), bounds.upper())),
            )
            .then_with(|| {
                left.stratum()
                    .id()
                    .as_str()
                    .cmp(right.stratum().id().as_str())
            })
    });
    for pair in cases.windows(2) {
        if pair[0].stratum().id() == pair[1].stratum().id() && pair[0] != pair[1] {
            return Err(SpiredCoordinateGuardCaseError::IdentityCollision.into());
        }
    }
    let generated = cases.len();
    cases.dedup();
    census.exact_duplicate_cases =
        generated
            .checked_sub(cases.len())
            .ok_or(SpiredCoordinateGuardCaseError::Invariant {
                detail: "canonical guard-case deduplication increased the output count",
            })?;
    census.output_cases = cases.len();
    census.output_coordinate_cells = cases
        .len()
        .checked_mul(
            cases
                .first()
                .map_or(0, |case| case.stratum().domain().arity()),
        )
        .and_then(|value| value.checked_mul(2))
        .ok_or(SpiredCoordinateGuardCaseIncomplete::ResourceCountOverflow {
            resource: OUTPUT_COORDINATE_CELLS,
        })?;
    check_limit(
        OUTPUT_COORDINATE_CELLS,
        census.output_coordinate_cells,
        limits.max_output_coordinate_cells,
    )?;
    census.output_identity_bytes = 0;
    for case in &cases {
        census.output_identity_bytes = checked_bounded_add(
            OUTPUT_IDENTITY_BYTES,
            census.output_identity_bytes,
            case.stratum().id().as_str().len(),
            limits.max_output_identity_bytes,
        )?;
    }
    Ok(SpiredCoordinateGuardCases::from_parts(cases, census))
}

#[cfg(test)]
pub(super) fn try_materialize_guards_for_test(
    context: &IndexedCoefficientContext,
    guards: &[IndexedPolynomial],
    parent: &SpiredCoordinateCaseObligation,
    limits: SpiredCoordinateGuardCaseLimits,
) -> Result<SpiredCoordinateGuardCaseOutcome, SpiredCoordinateGuardCaseError> {
    let result = (|| {
        let parent_stratum = parent.stratum();
        require_guard_blind(parent_stratum)?;
        if parent_stratum.context_fingerprint() != context.fingerprint()
            || parent_stratum.domain().arity() != context.index_count()
        {
            return Err(SpiredCoordinateGuardCaseError::WrongContext.into());
        }
        check_limit(
            REQUIRED_PREDICATES,
            guards.len(),
            limits.max_required_predicates,
        )?;
        let mut census = SpiredCoordinateGuardCaseCensus {
            required_predicates: guards.len(),
            guard_ordinal_references: guards.len(),
            ..Default::default()
        };
        let mut cases = try_vec(OUTPUT_CASES, 0)?;
        for (ordinal, guard) in guards.iter().enumerate() {
            let mut generated =
                try_materialize_one_guard(context, guard, ordinal, parent, limits, &mut census)?;
            try_reserve(&mut cases, generated.len(), OUTPUT_CASES)?;
            cases.append(&mut generated);
            census.resolved_required_predicates =
                checked_add(REQUIRED_PREDICATES, census.resolved_required_predicates, 1)?;
        }
        canonicalize_cases(cases, census, limits)
    })();
    match result {
        Ok(cases) => Ok(SpiredCoordinateGuardCaseOutcome::Exact(cases)),
        Err(Stop::CandidateUnusable(rejection)) => Ok(
            SpiredCoordinateGuardCaseOutcome::CandidateUnusable(rejection),
        ),
        Err(Stop::Incomplete(incomplete)) => {
            Ok(SpiredCoordinateGuardCaseOutcome::Incomplete(incomplete))
        }
        Err(Stop::Error(error)) => Err(error),
    }
}

fn try_zero_shift(arity: usize) -> Result<Vec<i64>, Stop> {
    let mut result = try_vec(OUTPUT_COORDINATE_CELLS, arity)?;
    result.resize(arity, 0);
    Ok(result)
}

fn try_copy_bounds(bounds: &[InteriorBounds]) -> Result<Vec<InteriorBounds>, Stop> {
    let mut result = try_vec(OUTPUT_COORDINATE_CELLS, bounds.len())?;
    result.extend_from_slice(bounds);
    Ok(result)
}

fn classify_indexed_error(error: IndexedAlgebraError) -> Stop {
    match error {
        IndexedAlgebraError::ResourceCountOverflow { resource } => {
            Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::ResourceCountOverflow {
                resource,
            })
        }
        IndexedAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::ResourceLimit {
            resource,
            requested,
            limit,
        }),
        IndexedAlgebraError::AllocationFailure {
            resource,
            requested,
        } => Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::AllocationFailure {
            resource,
            requested,
        }),
        error => Stop::Error(SpiredCoordinateGuardCaseError::IndexedAlgebra(error)),
    }
}

fn classify_stratum_error(error: StratumRegistryError) -> Stop {
    match error {
        StratumRegistryError::ResourceCountOverflow { resource } => {
            Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::ResourceCountOverflow {
                resource,
            })
        }
        StratumRegistryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::ResourceLimit {
            resource,
            requested,
            limit,
        }),
        StratumRegistryError::AllocationFailure {
            resource,
            requested,
        } => Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::AllocationFailure {
            resource,
            requested,
        }),
        StratumRegistryError::IndexedAlgebra(error) => classify_indexed_error(error),
        error => Stop::Error(SpiredCoordinateGuardCaseError::Stratum(error)),
    }
}

fn checked_add(resource: &'static str, left: usize, right: usize) -> Result<usize, Stop> {
    left.checked_add(right).ok_or(Stop::Incomplete(
        SpiredCoordinateGuardCaseIncomplete::ResourceCountOverflow { resource },
    ))
}

fn checked_bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, Stop> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(resource: &'static str, requested: usize, limit: usize) -> Result<(), Stop> {
    if requested > limit {
        Err(Stop::Incomplete(
            SpiredCoordinateGuardCaseIncomplete::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, Stop> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::AllocationFailure {
            resource,
            requested: capacity,
        })
    })?;
    Ok(values)
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), Stop> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        Stop::Incomplete(SpiredCoordinateGuardCaseIncomplete::AllocationFailure {
            resource,
            requested,
        })
    })
}

impl From<SpiredCoordinateGuardCaseError> for Stop {
    fn from(error: SpiredCoordinateGuardCaseError) -> Self {
        Self::Error(error)
    }
}

impl From<SpiredCoordinateGuardCaseIncomplete> for Stop {
    fn from(incomplete: SpiredCoordinateGuardCaseIncomplete) -> Self {
        Self::Incomplete(incomplete)
    }
}
