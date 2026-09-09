use crate::algebra::{IndexedAlgebraError, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::DecoratedStratum;
use crate::sector::{InteriorBounds, SectorInteriorDomain};

use super::{
    SpiredInactiveActivationAnalysis, SpiredInactiveActivationAnalysisCensus,
    SpiredInactiveActivationAnalysisLimits, SpiredInactiveActivationApplicationCell,
    SpiredInactiveActivationError, SpiredReplayedInactiveActivationTerm,
    SpiredSurvivingInactiveActivationFace, try_decompose_inactive_activation,
};

const CANDIDATE_TERMS: &str = "candidate terms";
const FACE_SPECIALIZATIONS: &str = "activation-face specializations";
const SURVIVING_FACE_SOURCES: &str = "surviving activation-face sources";
const UNIQUE_SURVIVING_FACES: &str = "unique surviving activation faces";
const PARTITION_FACE_VALUES: &str = "activation partition face values";
const APPLICATION_CELL_PRODUCT_STATES: &str = "activation application-cell product states";
const APPLICATION_CELLS: &str = "activation application cells";
const APPLICATION_CELL_BOUND_CELLS: &str = "activation application-cell bound cells";
const PRUNED_PHYSICAL_COLUMN_REFERENCES: &str = "pruned activation physical-column references";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RawSurvivingFace {
    position: usize,
    value: i64,
    physical_column: usize,
}

struct PreparedActivatingTerm<'candidate> {
    term: SpiredReplayedInactiveActivationTerm<'candidate>,
    numerator: IndexedPolynomial,
}

/// Restrict replayed exact coefficients to every finite inactive-line
/// activation slice and compile checked proposal geometry.
///
/// This bridge is intentionally below rule publication. Its application-cell
/// plans do not carry descent or nonzero-guard authority, and its equality
/// face specifications do not carry worklist or terminal authority. A
/// surviving first-activation slice causes its complete coordinate face to be
/// excluded from the application cells. That broadening is conservative and
/// keeps discovery identities free of inequality prefixes. The caller must
/// attach those face specifications to the canonical logical parent case,
/// never to the temporary finite-depth search envelope supplied here.
pub(crate) fn try_analyze_replayed_inactive_activations(
    context: &IndexedCoefficientContext,
    parent: &DecoratedStratum,
    terms: &[SpiredReplayedInactiveActivationTerm<'_>],
    limits: SpiredInactiveActivationAnalysisLimits,
) -> Result<SpiredInactiveActivationAnalysis, SpiredInactiveActivationError> {
    validate_parent_and_terms(context, parent, terms, limits)?;

    let mut census = SpiredInactiveActivationAnalysisCensus {
        candidate_terms: terms.len(),
        ..Default::default()
    };
    let mut raw_surviving_faces = try_vec(SURVIVING_FACE_SOURCES, 0)?;
    let mut partition_faces = try_vec(PARTITION_FACE_VALUES, 0)?;
    let mut activating_terms = try_vec(CANDIDATE_TERMS, terms.len())?;

    for &term in terms {
        let Some(geometry) =
            try_decompose_inactive_activation(parent.domain(), term.shift(), limits.geometry)?
        else {
            continue;
        };
        census.activating_terms = checked_add("activating terms", census.activating_terms, 1)?;

        // The exact coefficient has already been combined by physical column
        // at replay.  Symbolica supplies its canonical numerator polynomial;
        // the separate semantic guard machinery remains responsible for the
        // denominator's nonzero domain.
        let bound = context
            .bind_sealed(term.coefficient())
            .map_err(classify_indexed_error)?;
        let numerator = context
            .numerator_condition_from_bound(bound)
            .map_err(classify_indexed_error)?;

        for slice in geometry.activation_slices() {
            let requested = checked_bounded_add(
                PARTITION_FACE_VALUES,
                partition_faces.len(),
                1,
                limits.max_partition_face_values,
            )?;
            try_reserve(&mut partition_faces, 1, PARTITION_FACE_VALUES)?;
            partition_faces.push((slice.activation_position(), slice.activation_value()));
            debug_assert_eq!(partition_faces.len(), requested);
        }
        if numerator.is_zero() {
            census.vanishing_activation_slices = checked_add(
                "vanishing activation slices",
                census.vanishing_activation_slices,
                geometry.activation_slices().len(),
            )?;
            activating_terms.push(PreparedActivatingTerm { term, numerator });
            continue;
        }

        for slice in geometry.activation_slices() {
            census.face_specializations = checked_bounded_add(
                FACE_SPECIALIZATIONS,
                census.face_specializations,
                1,
                limits.max_face_specializations,
            )?;
            // A broad equality-face specification must be valid independently
            // of every other bound in the working envelope. In particular,
            // an unrelated singleton may be a representability accident
            // rather than a logical case equality. Restrict only the face
            // coordinate here. Cell-local pruning below may safely use every
            // singleton because its proposed application domain retains them.
            let fixed = [(slice.activation_position(), slice.activation_value())];
            let restricted = context
                .specialize_fixed_polynomial_sealed(&numerator, &fixed, limits.indexed_algebra)
                .map_err(classify_indexed_error)?;
            if restricted.is_zero() {
                census.vanishing_activation_slices = checked_add(
                    "vanishing activation slices",
                    census.vanishing_activation_slices,
                    1,
                )?;
                continue;
            }

            census.surviving_activation_slices = checked_add(
                "surviving activation slices",
                census.surviving_activation_slices,
                1,
            )?;
            let requested = checked_bounded_add(
                SURVIVING_FACE_SOURCES,
                raw_surviving_faces.len(),
                1,
                limits.max_surviving_face_sources,
            )?;
            try_reserve(&mut raw_surviving_faces, 1, SURVIVING_FACE_SOURCES)?;
            raw_surviving_faces.push(RawSurvivingFace {
                position: slice.activation_position(),
                value: slice.activation_value(),
                physical_column: term.physical_column(),
            });
            debug_assert_eq!(raw_surviving_faces.len(), requested);
        }
        activating_terms.push(PreparedActivatingTerm { term, numerator });
    }

    partition_faces.sort_unstable();
    partition_faces.dedup();
    census.partition_face_values = partition_faces.len();
    raw_surviving_faces.sort_unstable();
    raw_surviving_faces.dedup();
    let face_keys = canonical_face_keys(&raw_surviving_faces, limits, &mut census)?;
    let application_cells = build_application_cells(
        context,
        parent,
        &activating_terms,
        &partition_faces,
        &face_keys,
        limits,
        &mut census,
    )?;
    let surviving_faces = build_surviving_faces(&raw_surviving_faces, &face_keys)?;

    Ok(SpiredInactiveActivationAnalysis::from_parts(
        parent.id().clone(),
        application_cells,
        surviving_faces,
        census,
    ))
}

fn validate_parent_and_terms(
    context: &IndexedCoefficientContext,
    parent: &DecoratedStratum,
    terms: &[SpiredReplayedInactiveActivationTerm<'_>],
    limits: SpiredInactiveActivationAnalysisLimits,
) -> Result<(), SpiredInactiveActivationError> {
    if parent.context_fingerprint() != context.fingerprint()
        || parent.domain().arity() != context.index_count()
    {
        return Err(SpiredInactiveActivationError::WrongContext);
    }
    if !parent.guards().is_empty() {
        return Err(SpiredInactiveActivationError::GuardedParentGeometry {
            guard_branches: parent.guards().len(),
        });
    }
    check_limit(
        "index-space arity",
        parent.domain().arity(),
        limits.geometry.max_arity,
    )?;
    check_limit(CANDIDATE_TERMS, terms.len(), limits.max_candidate_terms)?;

    let mut columns = try_vec(CANDIDATE_TERMS, terms.len())?;
    for &term in terms {
        let actual = term.shift().len();
        if actual != parent.domain().arity() {
            return Err(SpiredInactiveActivationError::WrongShiftArity {
                expected: parent.domain().arity(),
                actual,
            });
        }
        columns.push(term.physical_column());
    }
    columns.sort_unstable();
    if let Some(pair) = columns.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(SpiredInactiveActivationError::DuplicatePhysicalColumn {
            physical_column: pair[0],
        });
    }
    // Authenticate all coefficients before any face specialization so a
    // foreign late term cannot leave observable partial output.
    for &term in terms {
        context
            .validate_with_limits(term.coefficient(), limits.indexed_algebra.exact_algebra)
            .map_err(classify_indexed_error)?;
    }
    Ok(())
}

fn canonical_face_keys(
    faces: &[RawSurvivingFace],
    limits: SpiredInactiveActivationAnalysisLimits,
    census: &mut SpiredInactiveActivationAnalysisCensus,
) -> Result<Vec<(usize, i64)>, SpiredInactiveActivationError> {
    let mut unique_count = 0usize;
    let mut previous = None;
    for face in faces {
        let key = (face.position, face.value);
        if previous != Some(key) {
            unique_count = checked_bounded_add(
                UNIQUE_SURVIVING_FACES,
                unique_count,
                1,
                limits.max_unique_surviving_faces,
            )?;
            previous = Some(key);
        }
    }
    let mut keys = try_vec(UNIQUE_SURVIVING_FACES, unique_count)?;
    for face in faces {
        let key = (face.position, face.value);
        if keys.last().copied() != Some(key) {
            keys.push(key);
        }
    }
    census.unique_surviving_faces = keys.len();
    census.duplicate_surviving_face_sources =
        faces
            .len()
            .checked_sub(keys.len())
            .ok_or(SpiredInactiveActivationError::Invariant {
                detail: "canonical activation-face grouping increased the source count",
            })?;
    Ok(keys)
}

fn build_application_cells(
    context: &IndexedCoefficientContext,
    parent: &DecoratedStratum,
    terms: &[PreparedActivatingTerm<'_>],
    partition_faces: &[(usize, i64)],
    surviving_faces: &[(usize, i64)],
    limits: SpiredInactiveActivationAnalysisLimits,
    census: &mut SpiredInactiveActivationAnalysisCensus,
) -> Result<Vec<SpiredInactiveActivationApplicationCell>, SpiredInactiveActivationError> {
    let arity = parent.domain().arity();
    let mut choices = try_vec(APPLICATION_CELLS, arity)?;
    for (position, &bounds) in parent.domain().bounds().iter().enumerate() {
        let partition_start =
            partition_faces.partition_point(|&(face_position, _)| face_position < position);
        let partition_end =
            partition_faces.partition_point(|&(face_position, _)| face_position <= position);
        let surviving_start =
            surviving_faces.partition_point(|&(face_position, _)| face_position < position);
        let surviving_end =
            surviving_faces.partition_point(|&(face_position, _)| face_position <= position);
        let intervals = retained_axis_cells(
            bounds,
            &partition_faces[partition_start..partition_end],
            &surviving_faces[surviving_start..surviving_end],
        )?;
        choices.push(intervals);
    }

    let mut cell_count = 1usize;
    let mut product_states = 0usize;
    for intervals in &choices {
        cell_count = checked_mul(APPLICATION_CELLS, cell_count, intervals.len())?;
        product_states = checked_bounded_add(
            APPLICATION_CELL_PRODUCT_STATES,
            product_states,
            cell_count,
            limits.max_application_cell_product_states,
        )?;
    }
    check_limit(APPLICATION_CELLS, cell_count, limits.max_application_cells)?;
    let bound_cells = checked_mul(APPLICATION_CELL_BOUND_CELLS, cell_count, arity)?;
    check_limit(
        APPLICATION_CELL_BOUND_CELLS,
        bound_cells,
        limits.max_application_cell_bound_cells,
    )?;
    let maximum_pruned_references =
        checked_mul(PRUNED_PHYSICAL_COLUMN_REFERENCES, cell_count, terms.len())?;
    check_limit(
        PRUNED_PHYSICAL_COLUMN_REFERENCES,
        maximum_pruned_references,
        limits.max_pruned_physical_column_references,
    )?;

    let mut cells = try_vec(APPLICATION_CELLS, cell_count)?;
    for ordinal in 0..cell_count {
        let mut remainder = ordinal;
        let mut bounds = try_vec(APPLICATION_CELL_BOUND_CELLS, arity)?;
        bounds.resize(arity, InteriorBounds::new(0, 0));
        for position in (0..arity).rev() {
            let axis_choices = &choices[position];
            let choice = remainder % axis_choices.len();
            remainder /= axis_choices.len();
            bounds[position] = axis_choices[choice];
        }
        let domain = SectorInteriorDomain::try_new(parent.domain().sector().clone(), bounds)?;
        let fixed = try_singleton_assignments(&domain)?;
        let mut pruned = try_vec(PRUNED_PHYSICAL_COLUMN_REFERENCES, terms.len())?;
        for term in terms {
            if !term_activates_on_cell(&domain, term.term.shift())? {
                continue;
            }
            let restricted = context
                .specialize_fixed_polynomial_sealed(&term.numerator, &fixed, limits.indexed_algebra)
                .map_err(classify_indexed_error)?;
            if !restricted.is_zero() {
                return Err(SpiredInactiveActivationError::Invariant {
                    detail: "an application cell retained a nonzero inactive-line activation",
                });
            }
            let requested = checked_bounded_add(
                PRUNED_PHYSICAL_COLUMN_REFERENCES,
                census.pruned_physical_column_references,
                1,
                limits.max_pruned_physical_column_references,
            )?;
            try_reserve(&mut pruned, 1, PRUNED_PHYSICAL_COLUMN_REFERENCES)?;
            pruned.push(term.term.physical_column());
            census.pruned_physical_column_references = requested;
        }
        pruned.sort_unstable();
        cells.push(SpiredInactiveActivationApplicationCell::from_parts(
            domain, pruned,
        ));
    }
    census.application_cell_product_states = product_states;
    census.application_cells = cells.len();
    census.application_cell_bound_cells = bound_cells;
    Ok(cells)
}

fn retained_axis_cells(
    bounds: InteriorBounds,
    partition_faces: &[(usize, i64)],
    surviving_faces: &[(usize, i64)],
) -> Result<Vec<InteriorBounds>, SpiredInactiveActivationError> {
    let capacity = checked_mul(APPLICATION_CELLS, partition_faces.len(), 2)
        .and_then(|count| checked_add(APPLICATION_CELLS, count, 1))?;
    let mut intervals = try_vec(APPLICATION_CELLS, capacity)?;
    let mut lower = i128::from(bounds.lower());
    let upper = i128::from(bounds.upper());
    for &(_, raw_value) in partition_faces {
        if !bounds.contains(raw_value) {
            return Err(SpiredInactiveActivationError::Invariant {
                detail: "an activation partition face escaped its parent bounds",
            });
        }
        let value = i128::from(raw_value);
        if lower < value {
            intervals.push(InteriorBounds::new(
                i64::try_from(lower).map_err(|_| SpiredInactiveActivationError::Invariant {
                    detail: "application-cell lower endpoint escaped i64",
                })?,
                i64::try_from(value - 1).map_err(|_| SpiredInactiveActivationError::Invariant {
                    detail: "application-cell upper endpoint escaped i64",
                })?,
            ));
        }
        if !surviving_faces
            .iter()
            .any(|&(_, surviving)| surviving == raw_value)
        {
            intervals.push(InteriorBounds::new(raw_value, raw_value));
        }
        lower = value + 1;
    }
    if lower <= upper {
        intervals.push(InteriorBounds::new(
            i64::try_from(lower).map_err(|_| SpiredInactiveActivationError::Invariant {
                detail: "application-cell lower endpoint escaped i64",
            })?,
            bounds.upper(),
        ));
    }
    Ok(intervals)
}

fn term_activates_on_cell(
    domain: &SectorInteriorDomain,
    shift: &crate::identity::IntegralShift,
) -> Result<bool, SpiredInactiveActivationError> {
    let mut activates = false;
    for ((&active, &bounds), &component) in domain
        .sector()
        .active_bits()
        .iter()
        .zip(domain.bounds())
        .zip(shift.values())
    {
        if active || component <= 0 {
            continue;
        }
        let lower = i128::from(bounds.lower()) + i128::from(component);
        let upper = i128::from(bounds.upper()) + i128::from(component);
        if upper <= 0 {
            continue;
        }
        if lower <= 0 {
            return Err(SpiredInactiveActivationError::Invariant {
                detail: "activation partition did not make term status uniform on a cell",
            });
        }
        activates = true;
    }
    Ok(activates)
}

fn build_surviving_faces(
    raw_faces: &[RawSurvivingFace],
    face_keys: &[(usize, i64)],
) -> Result<Vec<SpiredSurvivingInactiveActivationFace>, SpiredInactiveActivationError> {
    let mut result = try_vec(UNIQUE_SURVIVING_FACES, face_keys.len())?;
    let mut source_start = 0usize;

    for &(position, value) in face_keys {
        let source_end = raw_faces[source_start..]
            .partition_point(|face| (face.position, face.value) == (position, value))
            + source_start;
        let mut source_columns = try_vec(
            SURVIVING_FACE_SOURCES,
            source_end.saturating_sub(source_start),
        )?;
        source_columns.extend(
            raw_faces[source_start..source_end]
                .iter()
                .map(|face| face.physical_column),
        );
        source_columns.sort_unstable();
        source_columns.dedup();
        source_start = source_end;
        result.push(SpiredSurvivingInactiveActivationFace::from_parts(
            position,
            value,
            source_columns,
        ));
    }
    if source_start != raw_faces.len() {
        return Err(SpiredInactiveActivationError::Invariant {
            detail: "canonical activation-face grouping did not consume every source",
        });
    }
    Ok(result)
}

fn try_singleton_assignments(
    domain: &SectorInteriorDomain,
) -> Result<Vec<(usize, i64)>, SpiredInactiveActivationError> {
    let mut fixed = try_vec(FACE_SPECIALIZATIONS, domain.arity())?;
    fixed.extend(
        domain
            .bounds()
            .iter()
            .enumerate()
            .filter_map(|(position, bounds)| {
                (bounds.lower() == bounds.upper()).then_some((position, bounds.lower()))
            }),
    );
    Ok(fixed)
}

fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, SpiredInactiveActivationError> {
    let mut result = Vec::new();
    result.try_reserve_exact(capacity).map_err(|_| {
        SpiredInactiveActivationError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(result)
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SpiredInactiveActivationError> {
    values
        .try_reserve(additional)
        .map_err(|_| SpiredInactiveActivationError::AllocationFailure {
            resource,
            requested: values.len().saturating_add(additional),
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredInactiveActivationError> {
    left.checked_add(right)
        .ok_or(SpiredInactiveActivationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredInactiveActivationError> {
    left.checked_mul(right)
        .ok_or(SpiredInactiveActivationError::ResourceCountOverflow { resource })
}

fn checked_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, SpiredInactiveActivationError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredInactiveActivationError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(SpiredInactiveActivationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn classify_indexed_error(error: IndexedAlgebraError) -> SpiredInactiveActivationError {
    match error {
        IndexedAlgebraError::WrongContext => SpiredInactiveActivationError::WrongContext,
        IndexedAlgebraError::ResourceCountOverflow { resource } => {
            SpiredInactiveActivationError::ResourceCountOverflow { resource }
        }
        IndexedAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } => SpiredInactiveActivationError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        IndexedAlgebraError::AllocationFailure {
            resource,
            requested,
        } => SpiredInactiveActivationError::AllocationFailure {
            resource,
            requested,
        },
        error => SpiredInactiveActivationError::IndexedAlgebra(error),
    }
}
