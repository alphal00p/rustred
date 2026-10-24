//! Opt-in reconstruction of a canonical target row and its source weights.
//!
//! Native modular harder-prefix rank supplies a nonzero-minor certificate;
//! native exact `W*A == r` proves every output column. Together they establish
//! equality to the ordinary first-target GPLU row, without exact GPLU here.
//! This is not an original-IBP applicability or publication certificate.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::poly::reconstruction::{
    RationalReconstructionStats, ReconstructionError, ReconstructionMethod, ReconstructionOptions,
    reconstruct_rational_function_over_q_fallible,
};

use crate::algebra::Coefficient;

use super::{
    ExactRow, FrameVariables, Integral, IntegralOrder, MaterializationError, MaterializationEvent,
    ProbeFrame, Term,
};

mod cache;
mod probe;
pub(super) mod projected;
mod validation;
use cache::ImageCache;
#[cfg(test)]
use probe::probe_image;
use validation::validate_product;

pub(super) fn invalid(message: impl Into<String>) -> MaterializationError {
    MaterializationError::SemiNumericalReconstruction(format!(
        "source-weight materialization: {}",
        message.into()
    ))
}

#[derive(Clone, Copy)]
struct Limits {
    diagnostics_enabled: bool,
    max_degree: u16,
    max_probes: usize,
    max_attempts: usize,
    max_primes: usize,
    max_cached_images: usize,
    max_cached_values: usize,
    max_weight_slots: usize,
}

enum Event {
    Coefficient {
        column: usize,
        probes: Option<usize>,
        primes: Option<usize>,
        passes: usize,
    },
    WeightsStarted {
        rows: usize,
        lower_nonzeros: usize,
    },
    WeightsFinished {
        nonzero_weights: usize,
    },
    ProductStarted {
        rows: usize,
        columns: usize,
    },
    ProductFinished {
        output_terms: usize,
    },
}

/// The sole arity-dependent adapter. No characteristic-zero reducer is called,
/// even after a reconstruction miss or an inadmissible source prefix.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    diagnostics_enabled: bool,
    max_degree: u16,
    max_probes: usize,
    max_attempts: usize,
    max_primes: usize,
    max_cached_images: usize,
    max_cached_values: usize,
    max_weight_slots: usize,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    if rows.is_empty() || columns.is_empty() || target_column >= columns.len() {
        return Err(MaterializationError::TargetAbsent);
    }
    // Validate the WHOLE input before freezing a first-hit prefix. A malformed
    // post-hit or explicit-zero coefficient must not disappear through a shortcut.
    for term in rows.iter().flatten() {
        if term.coefficient.denominator.is_zero() {
            return Err(invalid("zero input denominator"));
        }
    }
    let frame = ProbeFrame::new(rows, columns, order, variables)?;
    if variables.active_len() == 0 {
        return Err(invalid("at least one coefficient variable is required"));
    }
    if diagnostics_enabled
        && (max_degree == 0 || max_probes == 0 || max_attempts == 0 || max_primes < 2)
    {
        return Err(invalid("invalid native reconstruction options"));
    }
    if diagnostics_enabled
        && (max_cached_images == 0 || max_cached_values == 0 || max_weight_slots == 0)
    {
        return Err(invalid(
            "cache and source-weight slot budgets must be positive",
        ));
    }
    let limits = Limits {
        diagnostics_enabled,
        max_degree,
        max_probes,
        max_attempts,
        max_primes,
        max_cached_images,
        max_cached_values,
        max_weight_slots,
    };
    observe(MaterializationEvent::SemiNumericalStarted {
        rows: rows.len(),
        columns: columns.len(),
        variables: variables.active_len(),
    });
    let lifted = materialize_frame(&frame, target_column, variables, limits, &mut |event| {
        observe(match event {
            Event::Coefficient {
                column,
                probes,
                primes,
                ..
            } => {
                // The legacy event has no unavailable-statistics variant.
                // Never mislabel one successful pass as all-pass work.
                let (Some(probes), Some(primes)) = (probes, primes) else {
                    return;
                };
                MaterializationEvent::SemiNumericalCoefficient {
                    column,
                    probes,
                    primes,
                }
            }
            Event::WeightsStarted {
                rows,
                lower_nonzeros,
            } => MaterializationEvent::TargetWeightsStarted {
                rows,
                lower_nonzeros,
            },
            Event::WeightsFinished { nonzero_weights } => {
                MaterializationEvent::TargetWeightsFinished { nonzero_weights }
            }
            Event::ProductStarted { rows, columns } => {
                MaterializationEvent::TargetReconstructionStarted { rows, columns }
            }
            Event::ProductFinished { output_terms } => {
                MaterializationEvent::TargetReconstructionFinished { output_terms }
            }
        });
    })?;
    let output: ExactRow<N> = lifted
        .into_iter()
        .map(|(column, coefficient)| Term {
            integral: columns[column as usize],
            coefficient,
        })
        .collect();
    observe(MaterializationEvent::SemiNumericalFinished {
        output_terms: output.len(),
    });
    Ok(output)
}

fn materialize_frame(
    frame: &ProbeFrame,
    target_column: usize,
    variables: &FrameVariables,
    limits: Limits,
    observe: &mut dyn FnMut(Event),
) -> Result<Vec<(u32, Coefficient)>, MaterializationError> {
    reconstruct_frame(
        frame,
        target_column,
        variables,
        limits,
        observe,
        &mut |_| Ok(()),
    )
    .map(|result| result.row)
}

struct ReconstructedFrame {
    row: Vec<(u32, Coefficient)>,
    /// Original source positions, including every reconstructed zero slot.
    weights: Vec<Coefficient>,
    prefix_len: usize,
}

fn validate_oracle_grants(_limits: Limits) -> Result<(), MaterializationError> {
    if _limits.diagnostics_enabled && _limits.max_degree > u16::MAX / 2 {
        return Err(invalid(
            "Symbolica's native doubled-degree exponent representation requires max_degree <= u16::MAX/2",
        ));
    }
    Ok(())
}

// The checked oracle distinguishes unusable specializations from permanent
// native errors. Keep the original typed error locally, rather than round-trip
// its scientific/resource identity through a numerical callback string.
fn reconstruct_pass(
    cache: &mut ImageCache<'_>,
    variables: &FrameVariables,
    slot: cache::Slot,
    options: &ReconstructionOptions,
    max_primes: usize,
    observed_probes: &mut Option<usize>,
) -> Result<
    Result<(Coefficient, RationalReconstructionStats), ReconstructionError>,
    MaterializationError,
> {
    let mut oracle_failure = None;
    let result = reconstruct_rational_function_over_q_fallible(
        variables.active_variables(),
        |field, point| {
            *observed_probes = observed_probes.and_then(|count| count.checked_add(1));
            let value = cache.coefficient(field, point, slot);
            if let Err(error) = cache.check_failure() {
                let signal = match crate::reduction::resource_limit_error(&error) {
                    Some(resource) => ReconstructionError::ResourceLimit(resource.clone()),
                    None => ReconstructionError::OracleFailure(error.to_string()),
                };
                oracle_failure = Some(error);
                return Err(signal);
            }
            Ok(value)
        },
        ReconstructionMethod::Automatic,
        options,
        max_primes,
    );
    if let Some(error) = oracle_failure {
        return Err(error);
    }
    if let Err(ReconstructionError::ResourceLimit(error)) = result {
        return Err(MaterializationError::Resources(error));
    }
    Ok(result)
}

struct ScalarWork {
    probes: Option<usize>,
    primes: Option<usize>,
    passes: usize,
}

fn reconstruct_scalar(
    cache: &mut ImageCache<'_>,
    variables: &FrameVariables,
    slot: cache::Slot,
    options: &ReconstructionOptions,
    max_primes: usize,
) -> Result<(Coefficient, ScalarWork), MaterializationError> {
    let label = match slot {
        cache::Slot::Target(column) => format!("target column {column}"),
        cache::Slot::Weight(source) => format!("source weight {source}"),
    };
    if !options.diagnostics_enabled {
        let mut observed_probes = Some(0);
        return reconstruct_pass(cache, variables, slot, options, 2, &mut observed_probes)?
            .map(|(value, stats)| {
                (
                    value,
                    ScalarWork {
                        probes: observed_probes,
                        primes: Some(stats.primes),
                        passes: 1,
                    },
                )
            })
            .map_err(|error| invalid(format!("{label} reconstruction failed: {error}")));
    }
    let mut window = options.clone();
    let growing_passes = options.max_probes == usize::MAX
        || options.max_attempts == usize::MAX
        || max_primes == usize::MAX;
    // Only caller-unbounded dimensions use finite growing pass windows.
    // Bounded callers retain their exact one-pass grants and behavior.
    if window.max_probes == usize::MAX {
        window.max_probes = options.verification_points.saturating_add(1);
    }
    if window.max_attempts == usize::MAX {
        window.max_attempts = 1;
    }
    if growing_passes {
        // The native degree allowance can affect BMA exponent-box planning.
        // Start at its positive minimum and dovetail it up to the caller's
        // intrinsic/explicit ceiling together with the work windows.
        window.max_degree = 1;
    }
    let mut prime_window = if max_primes == usize::MAX {
        2
    } else {
        max_primes
    };
    let mut observed_probes = Some(0usize);
    let mut passes = 0usize;
    loop {
        passes += 1;
        let result = reconstruct_pass(
            cache,
            variables,
            slot,
            &window,
            prime_window,
            &mut observed_probes,
        )?;
        match result {
            Ok((coefficient, stats)) => {
                return Ok((
                    coefficient,
                    ScalarWork {
                        probes: observed_probes,
                        // Native errors omit failed-pass statistics. The callback
                        // count above is exact, but total prime work is unavailable.
                        primes: (passes == 1).then_some(stats.primes),
                        passes,
                    },
                ));
            }
            Err(error) => {
                let retryable = matches!(
                    error,
                    ReconstructionError::PrimeLimit
                        | ReconstructionError::ProbeLimit
                        | ReconstructionError::AttemptsExhausted
                );
                let can_grow = window.max_probes < options.max_probes
                    || window.max_attempts < options.max_attempts
                    || prime_window < max_primes
                    || window.max_degree < options.max_degree;
                if !retryable || !can_grow {
                    return Err(invalid(format!("{label} reconstruction failed: {error}")));
                }
                // This is resumable scheduling, never a reconstruction/search
                // cutoff. Symbolica still owns interpolation, lifting and CRT.
                // All three windows remain at or below the caller's grants;
                // every failed intermediate pass is retried while work can grow.
                window.max_probes = window.max_probes.saturating_mul(2).min(options.max_probes);
                window.max_attempts = window
                    .max_attempts
                    .saturating_mul(2)
                    .min(options.max_attempts);
                prime_window = prime_window.saturating_mul(2).min(max_primes);
                window.max_degree = window.max_degree.saturating_mul(2).min(options.max_degree);
            }
        }
    }
}

fn reconstruct_frame(
    frame: &ProbeFrame,
    target_column: usize,
    variables: &FrameVariables,
    limits: Limits,
    observe: &mut dyn FnMut(Event),
    admit: &mut dyn FnMut(&Coefficient) -> Result<(), MaterializationError>,
) -> Result<ReconstructedFrame, MaterializationError> {
    validate_oracle_grants(limits)?;
    let mut cache = ImageCache::new(frame, target_column, limits);
    // Deterministic independent coordinates, never an affine-line sampling
    // pattern. These images propose a prefix/support, not exact zero claims.
    let mut random = StdRng::seed_from_u64(0x7372635f77656967);
    let mut admitted = None;
    let mut primes = PrimeIteratorU64::new(1 << 61);
    let mut primes_started = 0usize;
    let mut primes_exhausted = false;
    let mut active_primes = Vec::new();
    loop {
        symbolica::utils::resources::checkpoint().map_err(MaterializationError::Resources)?;
        if !primes_exhausted && (!limits.diagnostics_enabled || primes_started < limits.max_primes)
        {
            if let Some(prime) = primes.next() {
                active_primes.push((Zp64::new(prime), 0usize));
                primes_started = primes_started.saturating_add(1);
            } else {
                primes_exhausted = true;
            }
        }
        // Fair diagonal scheduling, not an admission cutoff: each round adds
        // one prime and samples each still-active prime once. An unlimited
        // point grant at a globally bad prime cannot starve every later prime.
        // Every explicit per-prime point grant is still available in full.
        for (field, probes) in &mut active_primes {
            *probes = probes.saturating_add(1);
            let prime = field.get_prime();
            let point: Vec<_> = (0..variables.active_len())
                .map(|_| field.to_element(random.random_range(1..prime)))
                .collect();
            if let Some(image) = cache.image(field, &point)? {
                if image.harder_rank == image.pivots.len() - 1 {
                    admitted = Some((
                        image.pivots.clone(),
                        image
                            .row
                            .iter()
                            .map(|(column, _)| *column)
                            .collect::<Vec<_>>(),
                        image.lower_nonzeros,
                    ));
                    break;
                }
            }
        }
        if admitted.is_some() {
            break;
        }
        if limits.diagnostics_enabled {
            active_primes.retain(|(_, probes)| *probes < limits.max_probes);
        }
        if active_primes.is_empty()
            && (primes_exhausted
                || (limits.diagnostics_enabled && primes_started == limits.max_primes))
        {
            break;
        }
    }
    let (chronology, support, lower_nonzeros) = admitted.ok_or_else(|| {
        invalid("no valid modular first-hit prefix has independent harder rows (admission miss)")
    })?;
    let prefix_len = chronology.len();
    // Includes original source positions, including None entries. No source is
    // silently removed to obtain a more convenient rank certificate.
    cache.freeze(chronology);
    let options = ReconstructionOptions {
        diagnostics_enabled: limits.diagnostics_enabled,
        max_degree: if limits.diagnostics_enabled {
            limits.max_degree
        } else {
            ReconstructionOptions::default().max_degree
        },
        max_probes: limits.max_probes,
        max_attempts: limits.max_attempts,
        verification_points: 3,
        ..Default::default()
    };
    let mut row = Vec::new();
    for column in support {
        let (coefficient, stats) = reconstruct_scalar(
            &mut cache,
            variables,
            cache::Slot::Target(column),
            &options,
            limits.max_primes,
        )?;
        admit(&coefficient)?;
        observe(Event::Coefficient {
            column: column as usize,
            probes: stats.probes,
            primes: stats.primes,
            passes: stats.passes,
        });
        if !coefficient.is_zero() {
            row.push((column, coefficient));
        }
    }
    observe(Event::WeightsStarted {
        rows: prefix_len,
        lower_nonzeros,
    });
    let mut weights = Vec::with_capacity(prefix_len);
    // Reconstruct every source slot, including zero weights. A failed native
    // zero reconstruction is a failure, not permission to assume that slot zero.
    for source in 0..prefix_len {
        let coefficient = reconstruct_scalar(
            &mut cache,
            variables,
            cache::Slot::Weight(source),
            &options,
            limits.max_primes,
        )?
        .0;
        admit(&coefficient)?;
        weights.push(coefficient);
    }
    observe(Event::WeightsFinished {
        nonzero_weights: weights.iter().filter(|value| !value.is_zero()).count(),
    });
    // Release modular cache and reducer images before exact multiplication.
    drop(cache);
    observe(Event::ProductStarted {
        rows: prefix_len,
        columns: (frame.columns_with_sentinel() - 1) as usize,
    });
    let output = validate_product(frame, prefix_len, target_column, &weights, &row, variables)?;
    observe(Event::ProductFinished {
        output_terms: output.len(),
    });
    Ok(ReconstructedFrame {
        row: output,
        weights,
        prefix_len,
    })
}

#[cfg(test)]
mod adversarial_tests;
