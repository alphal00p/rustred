//! Opt-in reconstruction of a canonical target row and its source weights.
//!
//! Native modular harder-prefix rank supplies a nonzero-minor certificate;
//! native exact `W*A == r` proves every output column. Together they establish
//! equality to the ordinary first-target GPLU row, without exact GPLU here.
//! This is not an original-IBP applicability or publication certificate.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::poly::reconstruction::{
    ReconstructionMethod, ReconstructionOptions, reconstruct_rational_function_over_q,
};

use crate::algebra::Coefficient;

use super::{
    ExactRow, FrameVariables, Integral, IntegralOrder, MaterializationError, MaterializationEvent,
    ProbeFrame, Term,
};

mod cache;
mod probe;
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
        probes: usize,
        primes: usize,
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
    if max_degree == 0 || max_probes == 0 || max_attempts == 0 || max_primes < 2 {
        return Err(invalid("invalid native reconstruction options"));
    }
    if max_cached_images == 0 || max_cached_values == 0 || max_weight_slots == 0 {
        return Err(invalid(
            "cache and source-weight slot budgets must be positive",
        ));
    }
    let limits = Limits {
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
            } => MaterializationEvent::SemiNumericalCoefficient {
                column,
                probes,
                primes,
            },
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
    let mut cache = ImageCache::new(frame, target_column, limits);
    // Deterministic independent coordinates, never an affine-line sampling
    // pattern. These images propose a prefix/support, not exact zero claims.
    let mut random = StdRng::seed_from_u64(0x7372635f77656967);
    let mut admitted = None;
    for prime in PrimeIteratorU64::new(1 << 61).take(limits.max_primes.min(2)) {
        let field = Zp64::new(prime);
        for _ in 0..limits.max_probes.min(8) {
            let point: Vec<_> = (0..variables.active_len())
                .map(|_| field.to_element(random.random_range(1..prime)))
                .collect();
            if let Some(image) = cache.image(&field, &point)? {
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
    }
    let (chronology, support, lower_nonzeros) = admitted.ok_or_else(|| {
        invalid("no valid modular first-hit prefix has independent harder rows (admission miss)")
    })?;
    let prefix_len = chronology.len();
    // Includes original source positions, including None entries. No source is
    // silently removed to obtain a more convenient rank certificate.
    cache.freeze(chronology);
    let options = ReconstructionOptions {
        max_degree: limits.max_degree,
        max_probes: limits.max_probes,
        max_attempts: limits.max_attempts,
        verification_points: 3,
        ..Default::default()
    };
    let active = variables.active_variables();
    let mut row = Vec::new();
    for column in support {
        let reconstruction = reconstruct_rational_function_over_q(
            active.clone(),
            |field, point| cache.coefficient(field, point, cache::Slot::Target(column)),
            ReconstructionMethod::Automatic,
            &options,
            limits.max_primes,
        );
        cache.check_failure()?;
        let (coefficient, stats) = reconstruction.map_err(|error| {
            invalid(format!(
                "target column {column} reconstruction failed: {error}",
            ))
        })?;
        observe(Event::Coefficient {
            column: column as usize,
            probes: stats.probes,
            primes: stats.primes,
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
        let reconstruction = reconstruct_rational_function_over_q(
            active.clone(),
            |field, point| cache.coefficient(field, point, cache::Slot::Weight(source)),
            ReconstructionMethod::Automatic,
            &options,
            limits.max_primes,
        );
        cache.check_failure()?;
        weights.push(
            reconstruction
                .map_err(|error| {
                    invalid(format!(
                        "source weight {source} reconstruction failed: {error}",
                    ))
                })?
                .0,
        );
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
    Ok(output)
}

#[cfg(test)]
mod adversarial_tests;
