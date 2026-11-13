//! Bounded arbitrary-column adapter to the existing source-weight materializer.
//!
//! This supplies no integral ordering, chart, source provenance or nonvanishing
//! authority. The caller must preserve ALL original source/domain conditions,
//! including zero-weight and post-hit inputs, and the returned weight poles.
//! A sampled nonzero harder minor is a generic-field witness, not a statement
//! that it is nonzero everywhere on the caller's chart. The fresh certificate
//! here is the exact full-column product, not those computational pivots.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use symbolica::domains::SelfRing;

use crate::algebra::{
    Coefficient, ExactAlgebraLimits, IndexedAlgebraError, IndexedCoefficient,
    IndexedCoefficientContext,
};
use crate::identity::IndexShift;

use super::{Event, FrameVariables, Limits, MaterializationError, ProbeFrame};

/// Explicit bounded-reconstruction policy; no defaults or exact fallback.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSourceWeightLimits {
    /// Authenticated input/reconstructed/output coefficient admission limits.
    /// Native reconstruction and the native sparse full product have internal
    /// scratch work not bounded by these retained-result admission ceilings.
    pub arithmetic: ExactAlgebraLimits,
    pub max_rows: usize,
    /// Includes structurally absent forbidden columns, excludes the sentinel.
    pub max_columns: usize,
    pub max_input_nonzeros: usize,
    /// Cumulative numerator+denominator terms in input, reconstructed target
    /// scalars and all reconstructed weight slots (including zero slots).
    pub max_coefficient_terms: usize,
    pub max_degree: u16,
    /// Native per-pass, per-scalar, per-prime allowance, NOT total probe work.
    /// Unbounded dimensions use growing finite native pass windows so an
    /// unusable first prime cannot starve later primes. The shared cache spans
    /// passes; an outer CPU/RSS policy remains the caller's responsibility.
    pub max_probes: usize,
    pub max_attempts: usize,
    pub max_primes: usize,
    pub max_cached_images: usize,
    pub max_cached_values: usize,
    pub max_weight_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectedSourceWeightEvent {
    FramePrepared {
        rows: usize,
        columns: usize,
        target_column: usize,
        original_variables: usize,
        active_variables: usize,
    },
    Coefficient {
        column: usize,
        probes: usize,
        primes: usize,
    },
    /// Work after multiple native passes. Oracle callbacks are counted across
    /// all passes; prime totals are unavailable because failed native passes
    /// return no statistics. None also denotes a diagnostic counter overflow.
    CoefficientPasses {
        column: usize,
        oracle_probes: Option<usize>,
        native_passes: usize,
        primes: Option<usize>,
    },
    WeightsStarted {
        rows: usize,
        lower_nonzeros: usize,
    },
    WeightsFinished {
        nonzero_weights: usize,
    },
    ExactProductStarted {
        rows: usize,
        columns: usize,
    },
    ExactProductFinished {
        output_terms: usize,
    },
}

/// Exact full-image identity with ORIGINAL caller source ordinals. Every zero
/// weight slot is natively reconstructed before omission from this sparse map.
/// This is not a checked rule or a source/applicability certificate.
#[derive(Debug)]
pub struct ProjectedSourceWeightProposal {
    pub weights: BTreeMap<usize, IndexedCoefficient>,
    pub image: BTreeMap<IndexShift, IndexedCoefficient>,
    pub prefix_rows: usize,
    pub original_variables: usize,
    pub active_variables: usize,
}

#[derive(Debug)]
pub enum ProjectedSourceWeightError {
    Invalid(&'static str),
    Budget(&'static str),
    Algebra(IndexedAlgebraError),
    /// Includes sampled admission miss, selected-frame/prefix-only full-rank
    /// ineligibility, native reconstruction failure and exact product mismatch.
    /// None of these establishes absence in a larger source space or family.
    Native(MaterializationError),
}

impl fmt::Display for ProjectedSourceWeightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid projected source frame: {message}"),
            Self::Budget(resource) => write!(f, "projected source-weight budget: {resource}"),
            Self::Algebra(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ProjectedSourceWeightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Algebra(error) => Some(error),
            Self::Native(error) => Some(error),
            _ => None,
        }
    }
}
impl From<IndexedAlgebraError> for ProjectedSourceWeightError {
    fn from(error: IndexedAlgebraError) -> Self {
        Self::Algebra(error)
    }
}
impl From<MaterializationError> for ProjectedSourceWeightError {
    fn from(error: MaterializationError) -> Self {
        Self::Native(error)
    }
}

type Result<T> = std::result::Result<T, ProjectedSourceWeightError>;

fn charge(total: &mut usize, added: usize, maximum: usize, name: &'static str) -> Result<()> {
    *total = total
        .checked_add(added)
        .ok_or(ProjectedSourceWeightError::Budget(name))?;
    if *total > maximum {
        return Err(ProjectedSourceWeightError::Budget(name));
    }
    Ok(())
}

fn charge_terms(
    total: &mut usize,
    value: &Coefficient,
    limits: ProjectedSourceWeightLimits,
) -> Result<()> {
    for count in [value.numerator.nterms(), value.denominator.nterms()] {
        charge(
            total,
            count,
            limits.max_coefficient_terms,
            "cumulative coefficient terms",
        )?;
    }
    Ok(())
}

/// Reconstruct a first-target original-source circuit on explicit `[F,target]`
/// columns, retaining every remaining physical image column for exact replay.
///
/// Source order is unchanged. F is ordered lexicographically, followed by the
/// unique target, then all other actual keys lexicographically. Absent F keys
/// are exact zero columns, not removed obligations. Every input is authenticated
/// before the engine can stop at a prefix. There is no exact GPLU fallback.
/// The existing engine requires an independent harder prefix (including every
/// original source position); refusal of a dependent prefix is an eligibility
/// miss, not a theorem of nonexistence. Constant-only frames are unsupported.
///
/// The observer is diagnostic and cannot alter the mathematics. Native library
/// panics and transient allocations require a caller's outer process boundary.
pub fn reconstruct_projected_source_weights(
    context: &IndexedCoefficientContext,
    rows: &[BTreeMap<IndexShift, IndexedCoefficient>],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    limits: ProjectedSourceWeightLimits,
    mut observe: impl FnMut(ProjectedSourceWeightEvent),
) -> Result<ProjectedSourceWeightProposal> {
    if rows.is_empty() {
        return Err(ProjectedSourceWeightError::Invalid("empty source frame"));
    }
    if rows.len() > limits.max_rows {
        return Err(ProjectedSourceWeightError::Budget("source rows"));
    }
    if limits.max_degree == 0
        || limits.max_probes == 0
        || limits.max_attempts == 0
        || limits.max_primes < 2
        || limits.max_cached_images == 0
        || limits.max_cached_values == 0
        || limits.max_weight_slots == 0
    {
        return Err(ProjectedSourceWeightError::Invalid(
            "native reconstruction allowances",
        ));
    }
    context.validate_index_arity(target.values())?;
    if forbidden.contains(target) {
        return Err(ProjectedSourceWeightError::Invalid("target is forbidden"));
    }
    let column_limit = limits.max_columns.min((u32::MAX - 1) as usize);
    let mut column_count = forbidden
        .len()
        .checked_add(1)
        .ok_or(ProjectedSourceWeightError::Budget("full physical columns"))?;
    if column_count > column_limit
        || u32::try_from(column_count)
            .ok()
            .and_then(|n| n.checked_add(1))
            .is_none()
    {
        return Err(ProjectedSourceWeightError::Budget("full physical columns"));
    }
    let mut universe = BTreeSet::new();
    for key in forbidden {
        context.validate_index_arity(key.values())?;
    }
    let mut nonzeros = 0;
    let mut terms = 0;
    for row in rows {
        for (key, value) in row {
            context.validate_index_arity(key.values())?;
            context.validate_with_limits(value, limits.arithmetic)?;
            if value.raw().is_zero() {
                return Err(ProjectedSourceWeightError::Invalid(
                    "explicit zero source entry",
                ));
            }
            charge(
                &mut nonzeros,
                1,
                limits.max_input_nonzeros,
                "input nonzeros",
            )?;
            charge_terms(&mut terms, value.raw(), limits)?;
            if !universe.contains(key) {
                if key != target && !forbidden.contains(key) {
                    charge(&mut column_count, 1, column_limit, "full physical columns")?;
                }
                universe.insert(key.clone());
            }
        }
    }
    if !universe.contains(target) {
        return Err(ProjectedSourceWeightError::Native(
            MaterializationError::TargetAbsent,
        ));
    }
    let columns: Vec<_> = forbidden
        .iter()
        .cloned()
        .chain(std::iter::once(target.clone()))
        .chain(
            universe
                .into_iter()
                .filter(|key| key != target && !forbidden.contains(key)),
        )
        .collect();
    if columns.len() > limits.max_columns {
        return Err(ProjectedSourceWeightError::Budget("full physical columns"));
    }
    let target_column = forbidden.len();
    let registry: BTreeMap<_, _> = columns
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, key)| (key, i))
        .collect();
    let variables = FrameVariables::from_coefficients(
        context.one().raw().get_variables().clone(),
        rows.iter()
            .flat_map(|row| row.values().map(IndexedCoefficient::raw)),
    )?;
    if variables.active_len() == 0 {
        return Err(ProjectedSourceWeightError::Invalid(
            "constant-only frame is unsupported",
        ));
    }
    let mut prepared = Vec::with_capacity(rows.len());
    for row in rows {
        let mut values = Vec::with_capacity(row.len());
        for (key, value) in row {
            let id = u32::try_from(registry[key])
                .map_err(|_| ProjectedSourceWeightError::Budget("native column indices"))?;
            values.push((id, variables.map_coefficient(value.raw())?));
        }
        // IndexShift order is not the [F,target,rest] registry order.
        values.sort_unstable_by_key(|(id, _)| *id);
        prepared.push(values);
    }
    let frame = ProbeFrame::from_ordered_rows(prepared, columns.len())?;
    observe(ProjectedSourceWeightEvent::FramePrepared {
        rows: rows.len(),
        columns: columns.len(),
        target_column,
        original_variables: variables.original_len(),
        active_variables: variables.active_len(),
    });
    let native_limits = Limits {
        diagnostics_enabled: true,
        max_degree: limits.max_degree,
        max_probes: limits.max_probes,
        max_attempts: limits.max_attempts,
        max_primes: limits.max_primes,
        max_cached_images: limits.max_cached_images,
        max_cached_values: limits.max_cached_values,
        max_weight_slots: limits.max_weight_slots,
    };
    let mut admission_failure = None;
    let result = super::reconstruct_frame(
        &frame,
        target_column,
        &variables,
        native_limits,
        &mut |event| {
            observe(match event {
                Event::Coefficient {
                    column,
                    probes,
                    primes,
                    passes,
                } => match (probes, primes) {
                    (Some(probes), Some(primes)) => ProjectedSourceWeightEvent::Coefficient {
                        column,
                        probes,
                        primes,
                    },
                    _ => ProjectedSourceWeightEvent::CoefficientPasses {
                        column,
                        oracle_probes: probes,
                        native_passes: passes,
                        primes,
                    },
                },
                Event::WeightsStarted {
                    rows,
                    lower_nonzeros,
                } => ProjectedSourceWeightEvent::WeightsStarted {
                    rows,
                    lower_nonzeros,
                },
                Event::WeightsFinished { nonzero_weights } => {
                    ProjectedSourceWeightEvent::WeightsFinished { nonzero_weights }
                }
                Event::ProductStarted { rows, columns } => {
                    ProjectedSourceWeightEvent::ExactProductStarted { rows, columns }
                }
                Event::ProductFinished { output_terms } => {
                    ProjectedSourceWeightEvent::ExactProductFinished { output_terms }
                }
            })
        },
        &mut |value| {
            let admitted = (|| -> Result<()> {
                let restored = variables.restore_coefficient(value)?;
                context.admit_native_result_with_limits(restored, limits.arithmetic)?;
                charge_terms(&mut terms, value, limits)
            })();
            if let Err(error) = admitted {
                admission_failure = Some(error);
                return Err(super::invalid(
                    "bounded indexed reconstruction admission refused",
                ));
            }
            Ok(())
        },
    );
    if let Some(error) = admission_failure {
        return Err(error);
    }
    let result = result?;
    let mut weights = BTreeMap::new();
    for (source, value) in result.weights.iter().enumerate() {
        let restored = variables.restore_coefficient(value)?;
        if variables.map_coefficient(&restored)? != *value {
            return Err(MaterializationError::CoefficientVariableMapMismatch.into());
        }
        let sealed = context.admit_native_result_with_limits(restored, limits.arithmetic)?;
        if !sealed.raw().is_zero() {
            weights.insert(source, sealed);
        }
    }
    let mut image = BTreeMap::new();
    for (column, value) in result.row {
        let key = columns
            .get(column as usize)
            .ok_or(ProjectedSourceWeightError::Invalid(
                "unregistered result column",
            ))?;
        image.insert(
            key.clone(),
            context.admit_native_result_with_limits(value, limits.arithmetic)?,
        );
    }
    if image.get(target).is_none_or(|value| !value.raw().is_one())
        || forbidden.iter().any(|key| image.contains_key(key))
    {
        return Err(ProjectedSourceWeightError::Invalid(
            "full target/forbidden replay",
        ));
    }
    Ok(ProjectedSourceWeightProposal {
        weights,
        image,
        prefix_rows: result.prefix_len,
        original_variables: variables.original_len(),
        active_variables: variables.active_len(),
    })
}

/// The modular first-hit circuit of a projected frame, for choosing the source
/// rows handed to [`reconstruct_projected_source_weights`] (scheme-certs).
///
/// Columns are ordered as there (`[F, target, rest]`). At one random point of
/// one large prime, a native reducer on the `[F, target]` columns keeps a row
/// only if it raises the rank on `F` or is the first row whose `F` part is
/// dependent while its reduced target entry is nonzero (the hit); every other
/// row is skipped. The modular source weights of the hit (the same native
/// probe as the reconstruction's) then name the rows of its circuit: their
/// original ordinals are returned, in source order. With a one-dimensional
/// circuit space these rows form an admissible prefix of exactly the size of
/// the circuit. `Ok(None)`: no row hits the target at this point (the target
/// is not reached by these rows on `F`), or a pole of an input coefficient at
/// the point. A choice heuristic only: the exact certificate is the full
/// product of the later reconstruction, never this sample.
pub fn select_projected_source_circuit(
    context: &IndexedCoefficientContext,
    rows: &[BTreeMap<IndexShift, IndexedCoefficient>],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    seed: u64,
) -> Result<Option<Vec<usize>>> {
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use symbolica::domains::finite_field::{
        FiniteFieldCore, PrimeIteratorU64, ToFiniteField, Zp64,
    };
    use symbolica::domains::{Field, Ring};
    use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

    if rows.is_empty() || forbidden.contains(target) {
        return Err(ProjectedSourceWeightError::Invalid(
            "empty frame or forbidden target",
        ));
    }
    let mut universe = BTreeSet::new();
    for row in rows {
        universe.extend(row.keys().cloned());
    }
    if !universe.contains(target) {
        return Ok(None);
    }
    let columns: Vec<_> = forbidden
        .iter()
        .cloned()
        .chain(std::iter::once(target.clone()))
        .chain(
            universe
                .into_iter()
                .filter(|key| key != target && !forbidden.contains(key)),
        )
        .collect();
    let target_column = forbidden.len();
    let registry: BTreeMap<_, _> = columns
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, key)| (key, i as u32))
        .collect();
    let variables = FrameVariables::from_coefficients(
        context.one().raw().get_variables().clone(),
        rows.iter()
            .flat_map(|row| row.values().map(IndexedCoefficient::raw)),
    )?;
    let mut prepared = Vec::with_capacity(rows.len());
    for row in rows {
        let mut values = Vec::with_capacity(row.len());
        for (key, value) in row {
            values.push((registry[key], variables.map_coefficient(value.raw())?));
        }
        values.sort_unstable_by_key(|(id, _)| *id);
        prepared.push(values);
    }
    let prime = PrimeIteratorU64::new(1 << 61)
        .next()
        .ok_or(ProjectedSourceWeightError::Invalid("no prime"))?;
    let field = Zp64::new(prime);
    let mut random = StdRng::seed_from_u64(seed);
    let point: Vec<_> = (0..variables.active_len())
        .map(|_| field.to_element(random.random_range(1..prime)))
        .collect();
    let target_id = target_column as u32;
    // [F, target] plus a trailing zero sentinel (no native full-rank shortcut).
    let mut reducer = SparseRowReducer::new(target_id + 2, field.clone(), LuLMode::None);
    let mut kept = Vec::new();
    let mut hit = false;
    for (source, row) in prepared.iter().enumerate() {
        let mut ids = Vec::new();
        let mut values = Vec::new();
        for (column, coefficient) in row {
            if *column > target_id {
                break;
            }
            let numerator = coefficient.numerator.evaluate_with_coeff_map(
                |value| value.to_finite_field(&field),
                &point,
                &field,
            );
            let denominator = coefficient.denominator.evaluate_with_coeff_map(
                |value| value.to_finite_field(&field),
                &point,
                &field,
            );
            if field.is_zero(&denominator) {
                return Ok(None);
            }
            let value = field.div(&numerator, &denominator);
            if !field.is_zero(&value) {
                ids.push(*column);
                values.push(value);
            }
        }
        match reducer.add_row(&values, &ids) {
            Some(pivot) if pivot < target_id => kept.push(source),
            Some(pivot) if pivot == target_id => {
                kept.push(source);
                hit = true;
                break;
            }
            _ => {}
        }
    }
    if !hit {
        return Ok(None);
    }
    let frame = ProbeFrame::from_ordered_rows(
        kept.iter()
            .map(|&source| prepared[source].clone())
            .collect(),
        columns.len(),
    )?;
    let Some(image) =
        super::probe::probe_prefix(&frame, target_column, None, kept.len(), &field, &point)?
    else {
        return Ok(None);
    };
    if image.harder_rank + 1 != image.pivots.len() {
        return Err(ProjectedSourceWeightError::Invalid(
            "the kept prefix is not independent on the forbidden columns",
        ));
    }
    Ok(Some(
        (image.weights.iter().enumerate())
            .filter(|(_, weight)| !field.is_zero(weight))
            .map(|(position, _)| kept[position])
            .collect(),
    ))
}

#[cfg(test)]
mod tests;
