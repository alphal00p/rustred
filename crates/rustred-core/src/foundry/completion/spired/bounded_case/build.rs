use crate::foundry::completion::spired::SpiredCoordinateCaseObligation;
use crate::foundry::completion::stratum::DecoratedStratum;
use crate::identity::{CompletedIbpSourceRows, IntegralShift};
use crate::sector::{InteriorBounds, SectorMonotoneDomain};

use super::{
    SpiredBoundedCaseAxisDisposition, SpiredBoundedCaseAxisEnvelope, SpiredBoundedCaseEnvelope,
    SpiredBoundedCaseEnvelopeCensus, SpiredBoundedCaseEnvelopeError,
    SpiredBoundedCaseEnvelopeLimits, SpiredBoundedCaseEqualityFace, SpiredBoundedCaseShiftEnvelope,
    SpiredBoundedWorkingCase,
};

const ARITY: &str = "case-envelope arity";
const SOURCE_ROWS: &str = "ordinary source rows";
const SOURCE_TERMS: &str = "ordinary source terms";
const SOURCE_COORDINATE_CELLS: &str = "ordinary source coordinate cells";
const SOURCE_DEPTH: &str = "signed-L1 source depth";
const AXIS_ENVELOPES: &str = "axis envelopes";
const BOUNDARY_VALUES_PER_AXIS: &str = "boundary values per axis";
const EQUALITY_FACES: &str = "equality faces";
const RETAINED_DOMAINS: &str = "retained domains";
const RETAINED_DOMAIN_BOUND_CELLS: &str = "retained domain bound cells";

/// Build the exact coordinate proposal covering a finite signed-L1 search.
///
/// For ordinary source-term shift `q`, target shift `p`, and every translated
/// source offset in shells `|s|_1 <= d`, each component satisfies
/// `q_i + s_i - p_i <= q_i - p_i + d`.  Consequently
/// `M_i=max_q max(0,q_i-p_i+d)` bounds every positive target-relative shift.
/// Since the target's physical index is `n_i+p_i`, restricting a free inactive
/// base coordinate to `n_i <= -p_i-M_i` creates a safe bulk for every row the
/// bounded search may inspect. (The sector ceiling clips positive safe bounds
/// back to zero.) The finitely many excluded values become broad equality
/// faces. No sampled evidence enters this proof and the result carries no
/// discovery or closure authority.
pub(crate) fn try_build_spired_bounded_case_envelope(
    logical_parent: &SpiredCoordinateCaseObligation,
    sources: &CompletedIbpSourceRows,
    target_shift: &IntegralShift,
    max_source_depth: usize,
    limits: SpiredBoundedCaseEnvelopeLimits,
) -> Result<SpiredBoundedCaseEnvelope, SpiredBoundedCaseEnvelopeError> {
    let parent = logical_parent.stratum();
    validate_fixed_scope(parent, sources, target_shift, max_source_depth, limits)?;
    let depth = i64::try_from(max_source_depth).map_err(|_| {
        SpiredBoundedCaseEnvelopeError::DepthNotRepresentable {
            depth: max_source_depth,
        }
    })?;

    let arity = parent.domain().arity();
    let mut maxima = try_vec(AXIS_ENVELOPES, arity)?;
    maxima.resize(arity, 0_i64);
    let mut source_terms = 0usize;
    for (source_ordinal, relation) in sources.relations().iter().enumerate() {
        for (term_ordinal, shift) in relation.terms().keys().enumerate() {
            source_terms =
                checked_bounded_add(SOURCE_TERMS, source_terms, 1, limits.max_source_terms)?;
            let actual = shift.values().len();
            if actual != arity {
                return Err(SpiredBoundedCaseEnvelopeError::WrongSourceTermArity {
                    source_ordinal,
                    term_ordinal,
                    expected: arity,
                    actual,
                });
            }
            for (position, ((&source_component, &target_component), maximum)) in shift
                .values()
                .iter()
                .zip(target_shift.values())
                .zip(maxima.iter_mut())
                .enumerate()
            {
                let translated_upper = i128::from(source_component) + i128::from(depth);
                let translated_lower = i128::from(source_component) - i128::from(depth);
                if translated_lower < i128::from(i64::MIN)
                    || translated_upper > i128::from(i64::MAX)
                {
                    return Err(SpiredBoundedCaseEnvelopeError::TranslatedShiftOverflow {
                        source_ordinal,
                        term_ordinal,
                        position,
                        source_shift: source_component,
                        depth: max_source_depth,
                    });
                }
                let relative_upper = translated_upper
                    .checked_sub(i128::from(target_component))
                    .ok_or(
                        SpiredBoundedCaseEnvelopeError::RelativeShiftEnvelopeOverflow {
                            source_ordinal,
                            term_ordinal,
                            position,
                            source_shift: source_component,
                            target_shift: target_component,
                            depth: max_source_depth,
                        },
                    )?;
                if relative_upper > i128::from(i64::MAX) {
                    return Err(
                        SpiredBoundedCaseEnvelopeError::RelativeShiftEnvelopeOverflow {
                            source_ordinal,
                            term_ordinal,
                            position,
                            source_shift: source_component,
                            target_shift: target_component,
                            depth: max_source_depth,
                        },
                    );
                }
                if relative_upper > 0 {
                    let candidate = i64::try_from(relative_upper).map_err(|_| {
                        SpiredBoundedCaseEnvelopeError::RelativeShiftEnvelopeOverflow {
                            source_ordinal,
                            term_ordinal,
                            position,
                            source_shift: source_component,
                            target_shift: target_component,
                            depth: max_source_depth,
                        }
                    })?;
                    *maximum = (*maximum).max(candidate);
                }
            }
        }
    }
    let source_coordinate_cells = checked_mul(SOURCE_COORDINATE_CELLS, source_terms, arity)?;
    check_limit(
        SOURCE_COORDINATE_CELLS,
        source_coordinate_cells,
        limits.max_source_coordinate_cells,
    )?;

    let parent_bounds = parent.domain().bounds();
    let mut bulk_bounds = try_copy_bounds(parent_bounds)?;
    let mut axes = try_vec(AXIS_ENVELOPES, arity)?;
    let mut face_specs = try_vec(EQUALITY_FACES, 0)?;
    let mut census = SpiredBoundedCaseEnvelopeCensus {
        source_rows: sources.source_row_count(),
        source_terms,
        source_coordinate_cells,
        ..Default::default()
    };

    for (position, (((&active, &bounds), &maximum), retained_bulk)) in parent
        .domain()
        .sector()
        .active_bits()
        .iter()
        .zip(parent_bounds)
        .zip(&maxima)
        .zip(bulk_bounds.iter_mut())
        .enumerate()
    {
        let base_safe_upper = if active {
            None
        } else {
            let raw = -(i128::from(target_shift.values()[position]) + i128::from(maximum));
            Some(i64::try_from(raw.min(0)).map_err(|_| {
                SpiredBoundedCaseEnvelopeError::BaseSafeUpperOverflow {
                    position,
                    target_shift: target_shift.values()[position],
                    max_positive_relative_shift: maximum,
                }
            })?)
        };
        let (disposition, excluded, face_count) = if active {
            census.active_axes = checked_add("active axes", census.active_axes, 1)?;
            (SpiredBoundedCaseAxisDisposition::Active, None, 0)
        } else if bounds.lower() == bounds.upper() {
            census.fixed_inactive_axes =
                checked_add("fixed inactive axes", census.fixed_inactive_axes, 1)?;
            (SpiredBoundedCaseAxisDisposition::FixedInactive, None, 0)
        } else {
            let safe_upper = bounds.upper().min(base_safe_upper.ok_or(
                SpiredBoundedCaseEnvelopeError::Invariant {
                    detail: "an inactive axis has no base-coordinate safety bound",
                },
            )?);
            if safe_upper < bounds.lower() {
                census.no_nonempty_bulk_axes = checked_add(
                    "inactive axes without a nonempty bulk",
                    census.no_nonempty_bulk_axes,
                    1,
                )?;
                (SpiredBoundedCaseAxisDisposition::NoNonEmptyBulk, None, 0)
            } else if safe_upper == bounds.upper() {
                census.full_parent_bulk_axes =
                    checked_add("full-parent bulk axes", census.full_parent_bulk_axes, 1)?;
                (SpiredBoundedCaseAxisDisposition::FullParentBulk, None, 0)
            } else {
                let excluded_lower = safe_upper.checked_add(1).ok_or(
                    SpiredBoundedCaseEnvelopeError::ResourceCountOverflow {
                        resource: BOUNDARY_VALUES_PER_AXIS,
                    },
                )?;
                let face_count = inclusive_value_count(
                    BOUNDARY_VALUES_PER_AXIS,
                    excluded_lower,
                    bounds.upper(),
                )?;
                check_limit(
                    BOUNDARY_VALUES_PER_AXIS,
                    face_count,
                    limits.max_boundary_values_per_axis,
                )?;
                let next_faces = checked_bounded_add(
                    EQUALITY_FACES,
                    face_specs.len(),
                    face_count,
                    limits.max_equality_faces,
                )?;
                // All aggregate geometry caps are preflighted before this
                // axis allocates or materializes even its compact face specs.
                let prospective_domains = checked_add(RETAINED_DOMAINS, next_faces, 1)?;
                check_limit(
                    RETAINED_DOMAINS,
                    prospective_domains,
                    limits.max_retained_domains,
                )?;
                let prospective_bound_cells =
                    checked_mul(RETAINED_DOMAIN_BOUND_CELLS, prospective_domains, arity)?;
                check_limit(
                    RETAINED_DOMAIN_BOUND_CELLS,
                    prospective_bound_cells,
                    limits.max_retained_domain_bound_cells,
                )?;
                try_reserve(&mut face_specs, face_count, EQUALITY_FACES)?;
                let mut value = excluded_lower;
                loop {
                    face_specs.push((position, value));
                    if value == bounds.upper() {
                        break;
                    }
                    value = value.checked_add(1).ok_or(
                        SpiredBoundedCaseEnvelopeError::ResourceCountOverflow {
                            resource: BOUNDARY_VALUES_PER_AXIS,
                        },
                    )?;
                }
                debug_assert_eq!(face_specs.len(), next_faces);
                *retained_bulk = InteriorBounds::new(bounds.lower(), safe_upper);
                census.split_axes = checked_add("split inactive axes", census.split_axes, 1)?;
                (
                    SpiredBoundedCaseAxisDisposition::SplitBoundaryFaces,
                    Some(InteriorBounds::new(excluded_lower, bounds.upper())),
                    face_count,
                )
            }
        };
        axes.push(SpiredBoundedCaseAxisEnvelope::new(
            position,
            bounds,
            maximum,
            base_safe_upper,
            *retained_bulk,
            excluded,
            face_count,
            disposition,
        ));
    }

    let retained_domains = checked_add(RETAINED_DOMAINS, face_specs.len(), 1)?;
    check_limit(
        RETAINED_DOMAINS,
        retained_domains,
        limits.max_retained_domains,
    )?;
    let retained_domain_bound_cells =
        checked_mul(RETAINED_DOMAIN_BOUND_CELLS, retained_domains, arity)?;
    check_limit(
        RETAINED_DOMAIN_BOUND_CELLS,
        retained_domain_bound_cells,
        limits.max_retained_domain_bound_cells,
    )?;

    let bulk = if bulk_bounds == parent_bounds {
        SpiredBoundedWorkingCase::new(parent.clone())
    } else {
        build_working_case(parent, bulk_bounds, limits)?
    };
    let mut equality_faces = try_vec(EQUALITY_FACES, face_specs.len())?;
    for (position, value) in face_specs {
        let mut face_bounds = try_copy_bounds(parent_bounds)?;
        let bound =
            face_bounds
                .get_mut(position)
                .ok_or(SpiredBoundedCaseEnvelopeError::Invariant {
                    detail: "a preflighted equality face addressed an absent coordinate",
                })?;
        if !bound.contains(value) {
            return Err(SpiredBoundedCaseEnvelopeError::Invariant {
                detail: "a preflighted equality face escaped its parent interval",
            });
        }
        *bound = InteriorBounds::new(value, value);
        equality_faces.push(SpiredBoundedCaseEqualityFace::new(
            position,
            value,
            build_working_case(parent, face_bounds, limits)?,
        ));
    }
    census.equality_faces = equality_faces.len();
    census.retained_domains = retained_domains;
    census.retained_domain_bound_cells = retained_domain_bound_cells;

    Ok(SpiredBoundedCaseEnvelope::from_parts(
        logical_parent.declared_carrier().id().clone(),
        parent.id().clone(),
        bulk,
        equality_faces,
        SpiredBoundedCaseShiftEnvelope::from_parts(max_source_depth, axes),
        census,
    ))
}

fn validate_fixed_scope(
    parent: &DecoratedStratum,
    sources: &CompletedIbpSourceRows,
    target_shift: &IntegralShift,
    max_source_depth: usize,
    limits: SpiredBoundedCaseEnvelopeLimits,
) -> Result<(), SpiredBoundedCaseEnvelopeError> {
    if !parent.guards().is_empty() {
        return Err(SpiredBoundedCaseEnvelopeError::GuardedParent {
            guard_branches: parent.guards().len(),
        });
    }
    if !parent.try_verify(limits.strata)? {
        return Err(SpiredBoundedCaseEnvelopeError::InvalidParentIdentity);
    }
    if !sources.is_complete_ordinary() {
        return Err(SpiredBoundedCaseEnvelopeError::WrongSourceLayout {
            actual: sources.layout_name(),
        });
    }
    if sources.source_row_count() == 0 {
        return Err(SpiredBoundedCaseEnvelopeError::EmptySourceRows);
    }
    if sources.family_fingerprint() != parent.family_fingerprint() {
        return Err(SpiredBoundedCaseEnvelopeError::WrongSourceFamily);
    }
    if sources.context_fingerprint() != parent.context_fingerprint() {
        return Err(SpiredBoundedCaseEnvelopeError::WrongSourceContext);
    }
    let arity = parent.domain().arity();
    if target_shift.len() != arity {
        return Err(SpiredBoundedCaseEnvelopeError::WrongTargetArity {
            expected: arity,
            actual: target_shift.len(),
        });
    }
    i64::try_from(max_source_depth).map_err(|_| {
        SpiredBoundedCaseEnvelopeError::DepthNotRepresentable {
            depth: max_source_depth,
        }
    })?;
    check_limit(ARITY, arity, limits.max_arity)?;
    check_limit(
        SOURCE_ROWS,
        sources.source_row_count(),
        limits.max_source_rows,
    )?;
    check_limit(SOURCE_DEPTH, max_source_depth, limits.max_source_depth)
}

fn build_working_case(
    parent: &DecoratedStratum,
    bounds: Vec<InteriorBounds>,
    limits: SpiredBoundedCaseEnvelopeLimits,
) -> Result<SpiredBoundedWorkingCase, SpiredBoundedCaseEnvelopeError> {
    let zero = try_vec("zero pivot components", parent.domain().arity())?;
    let mut zero = zero;
    zero.resize(parent.domain().arity(), 0_i64);
    let domain = SectorMonotoneDomain::try_new_for_rule(
        parent.domain().sector().clone(),
        bounds,
        &zero,
        &[] as &[&[i64]],
    )?;
    let child = DecoratedStratum::try_guard_blind(
        parent.family_fingerprint(),
        parent.context_fingerprint(),
        domain,
        limits.strata,
    )?;
    Ok(SpiredBoundedWorkingCase::new(child))
}

fn try_copy_bounds(
    bounds: &[InteriorBounds],
) -> Result<Vec<InteriorBounds>, SpiredBoundedCaseEnvelopeError> {
    let mut copied = try_vec("domain bounds", bounds.len())?;
    copied.extend_from_slice(bounds);
    Ok(copied)
}

fn inclusive_value_count(
    resource: &'static str,
    lower: i64,
    upper: i64,
) -> Result<usize, SpiredBoundedCaseEnvelopeError> {
    if lower > upper {
        return Err(SpiredBoundedCaseEnvelopeError::Invariant {
            detail: "an equality-face interval is empty",
        });
    }
    usize::try_from(i128::from(upper) - i128::from(lower) + 1)
        .map_err(|_| SpiredBoundedCaseEnvelopeError::ResourceCountOverflow { resource })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredBoundedCaseEnvelopeError> {
    left.checked_add(right)
        .ok_or(SpiredBoundedCaseEnvelopeError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredBoundedCaseEnvelopeError> {
    left.checked_mul(right)
        .ok_or(SpiredBoundedCaseEnvelopeError::ResourceCountOverflow { resource })
}

fn checked_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, SpiredBoundedCaseEnvelopeError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredBoundedCaseEnvelopeError> {
    if requested > limit {
        Err(SpiredBoundedCaseEnvelopeError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, SpiredBoundedCaseEnvelopeError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        SpiredBoundedCaseEnvelopeError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SpiredBoundedCaseEnvelopeError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        SpiredBoundedCaseEnvelopeError::AllocationFailure {
            resource,
            requested,
        }
    })
}
