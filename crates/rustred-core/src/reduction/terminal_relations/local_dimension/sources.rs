//! Native local Gram polynomials and regenerated ordinary-source certificates.
use super::*;
use crate::algebra::{IndexedAlgebraLimits, IndexedContextLimits};
use crate::identity::tangent::polynomial::{PolynomialWork, RawPolynomial};
use crate::identity::{
    IdentityConditionLimits, ParametricIbpConfig, RelationLimits, TangentSourceLimits,
};
use crate::reduction::terminal_relations::sources::Sources;
use crate::solver::SectorExecutor;
use geometry::{GeometryError, LinearMomentum, LocalGeometry};
use model::SkippedWitness;
use rayon::prelude::*;
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use symbolica::prelude::{Matrix, PolynomialRing, Rational};

fn native<T>(operation: impl FnOnce() -> T) -> Result<T, Error> {
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| Error::Algebra("native local polynomial operation panicked".into()))
}

fn geometry_error(error: GeometryError) -> Error {
    match error {
        GeometryError::ParameterValuedGeometry { .. } => {
            Error::UnsupportedGeometry(error.to_string())
        }
        GeometryError::InvalidInput(detail) => Error::InvalidInput(detail.into()),
        GeometryError::NativeRepresentation(detail) => Error::Overflow(detail),
        GeometryError::Algebra(detail) => Error::Algebra(detail),
        GeometryError::Cancelled => Error::Cancelled,
    }
}

fn polynomial_limits(options: LocalDimensionRecurrenceOptions) -> TangentSourceLimits {
    TangentSourceLimits {
        exact_algebra: options.exact_algebra,
        max_denominators: usize::MAX,
        max_polynomial_terms: usize::MAX,
        max_polynomial_degree: u16::MAX,
        max_exponent_entries: usize::MAX,
        max_term_operations: usize::MAX,
        max_selected_sources: usize::MAX,
        max_product_terms: usize::MAX,
        max_conditions: usize::MAX,
        max_exact_operations: usize::MAX,
        max_retained_coordinate_cells: usize::MAX,
    }
}

fn indexed_limits(options: LocalDimensionRecurrenceOptions) -> IndexedAlgebraLimits {
    IndexedAlgebraLimits {
        exact_algebra: options.exact_algebra,
        ..IndexedAlgebraLimits::unbounded_resources()
    }
}

fn ordinary_sources(
    family: &IntegralFamily,
    options: LocalDimensionRecurrenceOptions,
) -> Result<Sources, Error> {
    Sources::new_with_config(
        family,
        ParametricIbpConfig {
            context_limits: IndexedContextLimits {
                diagnostics_enabled: false,
                max_index_variables: usize::MAX,
                max_fingerprint_bytes: usize::MAX,
                max_native_symbol_name_bytes: usize::MAX,
            },
            relation_limits: RelationLimits {
                arithmetic: indexed_limits(options),
                identity_conditions: IdentityConditionLimits {
                    diagnostics_enabled: false,
                    max_sources: usize::MAX,
                },
            },
        },
    )
    .map_err(algebra)
}

fn scalar_product(
    work: &mut PolynomialWork<'_>,
    left: &LinearMomentum,
    right: &LinearMomentum,
) -> Result<RawPolynomial, Error> {
    let (constant, coefficients) =
        geometry::scalar_product_coefficients(work.family, left, right, work.limits.exact_algebra)
            .map_err(geometry_error)?;
    work.affine_parts(&constant, &coefficients).map_err(algebra)
}

fn determinant(
    work: &mut PolynomialWork<'_>,
    momenta: &[LinearMomentum],
) -> Result<RawPolynomial, Error> {
    if momenta.is_empty() {
        return Ok(work
            .template
            .constant(work.family.coefficient_context().one()));
    }
    // Each entry is affine. This is a native representation check, not a
    // selected spectator-rank or work limit.
    if momenta.len() > usize::from(u16::MAX) {
        return Err(Error::Overflow("Gram determinant polynomial degree"));
    }
    let size =
        u32::try_from(momenta.len()).map_err(|_| Error::Overflow("Gram matrix dimension"))?;
    let cells = momenta
        .len()
        .checked_mul(momenta.len())
        .ok_or(Error::Overflow("Gram matrix entries"))?;
    let mut entries = Vec::with_capacity(cells);
    for left in momenta {
        for right in momenta {
            entries.push(scalar_product(work, left, right)?);
        }
    }
    let matrix = Matrix::from_linear(
        entries,
        size,
        size,
        PolynomialRing::from_poly(&work.template),
    )
    .map_err(algebra)?;
    native(|| matrix.det())?.map_err(algebra)
}

struct PolynomialGeometry {
    gram: RawPolynomial,
    extended_gram: RawPolynomial,
    vector: Vec<RawPolynomial>,
}

fn polynomials(
    family: &IntegralFamily,
    geometry: &LocalGeometry,
    options: LocalDimensionRecurrenceOptions,
) -> Result<PolynomialGeometry, Error> {
    let mut work = PolynomialWork::new(family, polynomial_limits(options)).map_err(algebra)?;
    let gram = determinant(&mut work, &geometry.spectators)?;
    let mut momenta = Vec::with_capacity(geometry.spectators.len().saturating_add(1));
    momenta.push(LinearMomentum {
        loops: geometry.kappa.clone(),
        external: vec![Rational::from(0); family.external_count()],
    });
    momenta.extend(geometry.spectators.iter().cloned());
    let extended_gram = determinant(&mut work, &momenta)?;
    let mut vector = vec![
        work.template.zero();
        family
            .loop_count()
            .checked_add(family.external_count())
            .ok_or(Error::Overflow("vector contractions"))?
    ];
    for (slot, gradient) in geometry.gradient_contractions.iter().enumerate() {
        let derivative = native(|| extended_gram.derivative(slot))?;
        if derivative.is_zero() {
            continue;
        }
        for (component, factor) in gradient.loops.iter().chain(&gradient.external).enumerate() {
            if factor.is_zero() {
                continue;
            }
            let coefficient = geometry::rational_coefficient(
                factor,
                family.coefficient_context(),
                options.exact_algebra,
            )
            .map_err(geometry_error)?;
            let scaled = work
                .multiply(&derivative, &work.template.constant(coefficient))
                .map_err(algebra)?;
            vector[component] = work.sum(&vector[component], &scaled).map_err(algebra)?;
        }
    }
    Ok(PolynomialGeometry {
        gram,
        extended_gram,
        vector,
    })
}

fn shifted_key(
    key: &IntegralKey,
    exponents: &[u16],
    raising: Option<usize>,
) -> Result<IntegralKey, Error> {
    let mut powers = Vec::with_capacity(key.powers().len());
    for (axis, (&power, &exponent)) in key.powers().iter().zip(exponents).enumerate() {
        let shifted = power
            .checked_sub(i64::from(exponent))
            .and_then(|value| {
                if raising == Some(axis) {
                    value.checked_add(1)
                } else {
                    Some(value)
                }
            })
            .ok_or(Error::Overflow("signed recurrence power"))?;
        powers.push(shifted);
    }
    IntegralKey::try_new(powers).map_err(algebra)
}

fn accumulate(
    family: &IntegralFamily,
    row: &mut TerminalRelationRow,
    key: IntegralKey,
    value: Coefficient,
    limits: ExactAlgebraLimits,
) -> Result<(), Error> {
    if value.is_zero() {
        return Ok(());
    }
    let value = if let Some(previous) = row.remove(&key) {
        family
            .coefficient_context()
            .try_add(&previous, &value, limits)
            .map_err(algebra)?
    } else {
        value
    };
    if !value.is_zero() {
        row.insert(key, value);
    }
    Ok(())
}

fn equation(
    family: &IntegralFamily,
    source: &IntegralKey,
    geometry: &LocalGeometry,
    poly: &PolynomialGeometry,
    sources: &Sources,
    options: LocalDimensionRecurrenceOptions,
    control: &LocalDimensionRecurrenceControl<'_>,
    regenerated: &AtomicUsize,
) -> Result<Witness, Error> {
    let context = family.coefficient_context();
    // Monomial multiplication changes the source seed BEFORE specialization:
    // the regenerated ordinary source automatically includes the product rule.
    let mut requests: BTreeMap<(usize, IntegralKey), Coefficient> = BTreeMap::new();
    for (contraction, vector) in poly.vector.iter().enumerate() {
        for (coefficient, exponents) in vector.coefficients.iter().zip(vector.exponents_iter()) {
            control.checkpoint()?;
            let seed = shifted_key(source, exponents, None)?;
            for (differentiated, direction) in geometry.direction.iter().enumerate() {
                if direction.is_zero() {
                    continue;
                }
                let factor =
                    geometry::rational_coefficient(direction, context, options.exact_algebra)
                        .map_err(geometry_error)?;
                let weight = context
                    .try_mul(coefficient, &factor, options.exact_algebra)
                    .map_err(algebra)?;
                let ordinal = contraction
                    .checked_mul(family.loop_count())
                    .and_then(|base| base.checked_add(differentiated))
                    .ok_or(Error::Overflow("ordinary source ordinal"))?;
                let id = (ordinal, seed.clone());
                let weight = if let Some(previous) = requests.remove(&id) {
                    context
                        .try_add(&previous, &weight, options.exact_algebra)
                        .map_err(algebra)?
                } else {
                    weight
                };
                if !weight.is_zero() {
                    requests.insert(id, weight);
                }
            }
        }
    }
    let mut weighted = TerminalRelationRow::new();
    let mut conditions = Vec::new();
    let mut seen_conditions = HashSet::new();
    let mut contributions = Vec::with_capacity(requests.len());
    for ((ordinal, seed), weight) in requests {
        control.checkpoint()?;
        let (row, source_conditions) = sources
            .row(&seed, ordinal, indexed_limits(options))
            .map_err(algebra)?;
        regenerated.fetch_add(1, Ordering::Relaxed);
        for condition in source_conditions {
            if seen_conditions.insert(condition.clone()) {
                conditions.push(condition);
            }
        }
        if !weight.denominator.is_constant() {
            let pole: Coefficient = weight.denominator.clone().into();
            if seen_conditions.insert(pole.clone()) {
                conditions.push(pole);
            }
        }
        for (key, coefficient) in row {
            let value = context
                .try_mul(&coefficient, &weight, options.exact_algebra)
                .map_err(algebra)?;
            accumulate(family, &mut weighted, key, value, options.exact_algebra)?;
        }
        contributions.push(WeightedSource {
            ordinal,
            seed,
            weight,
        });
    }
    // Independently assemble 2 U P - (d-E) G from the actual effective powers.
    // Here U is the local index-raising operator, NOT a parent Symanzik U.
    let mut expected = TerminalRelationRow::new();
    for (slot, alpha) in geometry.alpha.iter().enumerate() {
        control.checkpoint()?;
        if alpha.is_zero() {
            continue;
        }
        let effective = context
            .try_add(
                &context.integer(source.powers()[slot]),
                &family.power_shifts()[slot],
                options.exact_algebra,
            )
            .map_err(algebra)?;
        if effective.is_zero() {
            continue;
        }
        let twice_alpha = geometry::rational_coefficient(
            &(alpha * &Rational::from(2)),
            context,
            options.exact_algebra,
        )
        .map_err(geometry_error)?;
        let raising = context
            .try_mul(&twice_alpha, &effective, options.exact_algebra)
            .map_err(algebra)?;
        for (coefficient, exponents) in poly
            .extended_gram
            .coefficients
            .iter()
            .zip(poly.extended_gram.exponents_iter())
        {
            let value = context
                .try_mul(coefficient, &raising, options.exact_algebra)
                .map_err(algebra)?;
            accumulate(
                family,
                &mut expected,
                shifted_key(source, exponents, Some(slot))?,
                value,
                options.exact_algebra,
            )?;
        }
    }
    let rank = i64::try_from(geometry.spectators.len())
        .map_err(|_| Error::Overflow("spectator rank coefficient"))?;
    let d_minus_e = context
        .try_sub(
            family.dimension(),
            &context.integer(rank),
            options.exact_algebra,
        )
        .map_err(algebra)?;
    for (coefficient, exponents) in poly
        .gram
        .coefficients
        .iter()
        .zip(poly.gram.exponents_iter())
    {
        let value = context
            .try_neg(
                &context
                    .try_mul(coefficient, &d_minus_e, options.exact_algebra)
                    .map_err(algebra)?,
                options.exact_algebra,
            )
            .map_err(algebra)?;
        accumulate(
            family,
            &mut expected,
            shifted_key(source, exponents, None)?,
            value,
            options.exact_algebra,
        )?;
    }
    let mut sum = expected.clone();
    for (key, coefficient) in weighted {
        accumulate(family, &mut sum, key, coefficient, options.exact_algebra)?;
    }
    if !sum.is_empty() {
        return Err(Error::IdentityMismatch);
    }
    Ok(Witness {
        source: source.clone(),
        direction: geometry
            .direction
            .iter()
            .map(|value| {
                geometry::rational_coefficient(value, context, options.exact_algebra)
                    .map_err(geometry_error)
            })
            .collect::<Result<_, _>>()?,
        spectator_rank: geometry.spectators.len(),
        contributions,
        equation: TerminalEquation {
            terms: expected,
            nonzero_conditions: conditions,
        },
    })
}

struct WitnessJob {
    source: IntegralKey,
    geometry: Arc<(LocalGeometry, PolynomialGeometry)>,
}

/// Reuse the existing licensed pool and caller-thread observer pattern.
/// Workers borrow frozen sources and see only atomic controls, not callbacks.
fn prepare_witnesses(
    pool: &SectorExecutor,
    family: &IntegralFamily,
    sources: &Sources,
    jobs: &[WitnessJob],
    options: LocalDimensionRecurrenceOptions,
    control: &LocalDimensionRecurrenceControl<'_>,
    mut progress: LocalDimensionRecurrenceProgress,
) -> Result<Vec<Witness>, Error> {
    let cancel = AtomicBool::new(false);
    let regenerated = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    progress.total_witnesses = jobs.len();
    control.report(progress);
    control.checkpoint()?;
    let execute = || {
        pool.install(|| {
            jobs.par_iter()
                .map(|job| {
                    let cancelled = || cancel.load(Ordering::Relaxed);
                    let worker_control = LocalDimensionRecurrenceControl {
                        cancelled: Some(&cancelled),
                        progress: None,
                    };
                    let result = equation(
                        family,
                        &job.source,
                        &job.geometry.0,
                        &job.geometry.1,
                        sources,
                        options,
                        &worker_control,
                        &regenerated,
                    );
                    completed.fetch_add(1, Ordering::Relaxed);
                    result
                })
                .collect::<Vec<_>>()
        })
    };
    let results = if pool.workers() == 1 {
        // SectorExecutor's width-one lane intentionally stays on the caller
        // thread, including restricted Symbolica's thread-local permit.
        let mut results = Vec::with_capacity(jobs.len());
        for job in jobs {
            results.push(equation(
                family,
                &job.source,
                &job.geometry.0,
                &job.geometry.1,
                sources,
                options,
                control,
                &regenerated,
            ));
            completed.fetch_add(1, Ordering::Relaxed);
            progress.regenerated_source_rows = regenerated.load(Ordering::Relaxed);
            progress.completed_witnesses = completed.load(Ordering::Relaxed);
            control.report(progress);
            if results.last().is_some_and(Result::is_err) {
                break;
            }
        }
        results
    } else {
        std::thread::scope(|scope| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            scope.spawn(move || {
                let _ = sender.send(execute());
            });
            loop {
                match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(result) => break Ok(result),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if control.cancelled.is_some_and(|cancelled| cancelled()) {
                            cancel.store(true, Ordering::Relaxed);
                        }
                        progress.regenerated_source_rows = regenerated.load(Ordering::Relaxed);
                        progress.completed_witnesses = completed.load(Ordering::Relaxed);
                        control.report(progress);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break Err(Error::Algebra(
                            "local witness coordinator disconnected".into(),
                        ));
                    }
                }
            }
        })?
    };
    progress.regenerated_source_rows = regenerated.load(Ordering::Relaxed);
    progress.completed_witnesses = completed.load(Ordering::Relaxed);
    control.report(progress);
    // Worker errors never set cancellation: ordinal merge must preserve a real
    // exact-proof failure rather than mask it as an interrupted preparation.
    let witnesses = join_witnesses(results)?;
    control.checkpoint()?;
    Ok(witnesses)
}

fn join_witnesses(results: Vec<Result<Witness, Error>>) -> Result<Vec<Witness>, Error> {
    // Preserve the first genuine proof failure in original job order even if
    // a simultaneous USER cancellation interrupted another earlier job.
    if let Some(error) = results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .find(|error| !matches!(error, Error::Cancelled))
    {
        return Err(error.clone());
    }
    results.into_iter().collect()
}

impl LocalDimensionRecurrencePlan {
    pub fn prepare(
        family: Arc<IntegralFamily>,
        input: &BTreeSet<IntegralKey>,
        options: LocalDimensionRecurrenceOptions,
        control: &LocalDimensionRecurrenceControl<'_>,
    ) -> Result<Self, Error> {
        control.checkpoint()?;
        let pool = SectorExecutor::new(options.workers)
            .map_err(|error| Error::InvalidInput(error.to_string()))?;
        if input
            .iter()
            .any(|key| key.powers().len() != family.denominator_count())
        {
            return Err(Error::InvalidInput(
                "source key arity differs from family".into(),
            ));
        }
        let sources = ordinary_sources(&family, options)?;
        let mut plan = Self {
            family: Arc::clone(&family),
            input: input.clone(),
            inventory: input.clone(),
            options,
            witnesses: Vec::new(),
            skips: Vec::new(),
            equations: BTreeMap::new(),
            statistics: LocalDimensionRecurrenceStatistics {
                input_keys: input.len(),
                ..Default::default()
            },
        };
        let mut progress = LocalDimensionRecurrenceProgress {
            input_keys: input.len(),
            ..Default::default()
        };
        // Direction proposals depend on positive effective support; spectator
        // geometry depends on complete effective support, including numerators.
        // Reuse immutable native Gram/vector tables across dotted powers.
        let mut direction_cache: BTreeMap<Vec<bool>, Vec<Vec<Rational>>> = BTreeMap::new();
        let mut geometry_cache: BTreeMap<
            (Vec<bool>, Vec<Rational>),
            Arc<(LocalGeometry, PolynomialGeometry)>,
        > = BTreeMap::new();
        let mut jobs = Vec::new();
        for source in input {
            control.checkpoint()?;
            let mut effective_support = Vec::with_capacity(family.denominator_count());
            for (&power, shift) in source.powers().iter().zip(family.power_shifts()) {
                effective_support.push(
                    !family
                        .coefficient_context()
                        .try_add(
                            &family.coefficient_context().integer(power),
                            shift,
                            options.exact_algebra,
                        )
                        .map_err(algebra)?
                        .is_zero(),
                );
            }
            let positive_support: Vec<_> = source
                .powers()
                .iter()
                .zip(&effective_support)
                .map(|(&power, &effective)| power > 0 && effective)
                .collect();
            if !direction_cache.contains_key(&positive_support) {
                let directions = match geometry::structural_directions(
                    &family,
                    source,
                    options.exact_algebra,
                    control.cancelled,
                ) {
                    Ok(directions) => directions,
                    Err(error @ GeometryError::ParameterValuedGeometry { .. }) => {
                        plan.record_skip(source, Vec::new(), error.to_string());
                        progress.processed_keys += 1;
                        control.report(progress);
                        continue;
                    }
                    Err(error) => return Err(geometry_error(error)),
                };
                direction_cache.insert(positive_support.clone(), directions);
            }
            for direction in &direction_cache[&positive_support] {
                control.checkpoint()?;
                plan.statistics.nominated_directions += 1;
                progress.nominated_directions += 1;
                let geometry_id = (effective_support.clone(), direction.clone());
                if !geometry_cache.contains_key(&geometry_id) {
                    let geometry = geometry::prepare_geometry(
                        &family,
                        source,
                        direction,
                        options.exact_algebra,
                        control.cancelled,
                    )
                    .map_err(geometry_error)?;
                    let poly = polynomials(&family, &geometry, options)?;
                    if !poly.gram.is_zero() {
                        plan.statistics.supported_geometries += 1;
                        progress.supported_geometries += 1;
                    }
                    geometry_cache.insert(geometry_id.clone(), Arc::new((geometry, poly)));
                }
                let (_, poly) = &*geometry_cache[&geometry_id];
                if poly.gram.is_zero() {
                    plan.statistics.degenerate_grams += 1;
                    let encoded = direction
                        .iter()
                        .map(|value| {
                            geometry::rational_coefficient(
                                value,
                                family.coefficient_context(),
                                options.exact_algebra,
                            )
                            .map_err(geometry_error)
                        })
                        .collect::<Result<_, _>>()?;
                    plan.record_skip(
                        source,
                        encoded,
                        "zero conditional spectator Gram determinant".into(),
                    );
                    continue;
                }
                jobs.push(WitnessJob {
                    source: source.clone(),
                    geometry: Arc::clone(&geometry_cache[&geometry_id]),
                });
                control.report(progress);
            }
            progress.processed_keys += 1;
            control.report(progress);
        }
        for witness in
            prepare_witnesses(&pool, &family, &sources, &jobs, options, control, progress)?
        {
            plan.statistics.weighted_source_rows = plan
                .statistics
                .weighted_source_rows
                .checked_add(witness.contributions.len())
                .ok_or(Error::Overflow("weighted source rows"))?;
            if !witness.equation.terms.is_empty() {
                plan.inventory
                    .extend(witness.equation.terms.keys().cloned());
                plan.statistics.equations += 1;
                plan.statistics.output_terms = plan
                    .statistics
                    .output_terms
                    .checked_add(witness.equation.terms.len())
                    .ok_or(Error::Overflow("equation terms"))?;
                plan.equations
                    .entry(witness.source.clone())
                    .or_default()
                    .push(witness.equation.clone());
            }
            plan.witnesses.push(witness);
        }
        plan.statistics.inventory_keys = plan.inventory.len();
        Ok(plan)
    }

    fn record_skip(&mut self, source: &IntegralKey, direction: Vec<Coefficient>, reason: String) {
        self.statistics.skipped_geometries += 1;
        *self
            .statistics
            .skipped_reasons
            .entry(reason.clone())
            .or_default() += 1;
        self.skips.push(SkippedWitness {
            source: source.clone(),
            direction,
            reason,
        });
    }
}

#[cfg(test)]
mod parallel_error_tests {
    use super::*;
    #[test]
    fn ordinal_join_never_masks_exact_failure_as_user_cancellation() {
        assert!(matches!(
            join_witnesses(vec![Err(Error::Cancelled), Err(Error::IdentityMismatch)]),
            Err(Error::IdentityMismatch)
        ));
        assert!(
            matches!(join_witnesses(vec![Err(Error::InvalidInput("first".into())), Err(Error::IdentityMismatch)]), Err(Error::InvalidInput(detail)) if detail == "first")
        );
        assert!(matches!(
            join_witnesses(vec![Err(Error::Cancelled)]),
            Err(Error::Cancelled)
        ));
    }
}
