//! Fixed-task and finite-field schedule admission.

use std::cmp::Ordering;

use symbolica::prelude::Integer;

use crate::foundry::completion::stratum::{CampaignStratumAnchor, ImmutableOwnerSnapshot};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};

use super::super::super::CampaignModularProbe;
use super::super::{ProbeLocalSchedulerError, ProbeLocalSchedulerLimits};
use super::budget::{check_limit, checked_add, try_vec};

const PROBES: &str = "probe-local obstruction probes";
const PROBE_COORDINATES: &str = "probe-local retained probe coordinate cells";
const PROBE_KEY_ORDER: &str = "probe-local canonical probe-key order";
const OUTCOMES: &str = "probe-local retained outcomes";
const RESIDUAL_PROPOSALS: &str = "probe-local residual proposals per iteration";

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_fixed_task(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_shift: &IntegralShift,
    stratum: &CampaignStratumAnchor,
    owners: &ImmutableOwnerSnapshot,
    limits: ProbeLocalSchedulerLimits,
) -> Result<(), ProbeLocalSchedulerError> {
    // A zero proposal cap would turn a complete, nonempty residual census into
    // an artificial stall.  Reject that policy at the task boundary rather
    // than allowing it to resemble discovery evidence later.
    check_limit(
        RESIDUAL_PROPOSALS,
        1,
        limits.max_residual_proposals_per_iteration,
    )?;
    if !completed.is_complete_ordinary() {
        return Err(ProbeLocalSchedulerError::WrongSourceLayout {
            actual: completed.layout_name(),
        });
    }
    let arity = generator.context().index_count();
    if target_shift.len() != arity {
        return Err(ProbeLocalSchedulerError::WrongTargetArity {
            expected: arity,
            actual: target_shift.len(),
        });
    }
    if stratum.arity() != arity || owners.arity() != arity {
        return Err(ProbeLocalSchedulerError::WrongTaskScope {
            detail: "target, decorated stratum, and immutable owners have different arities",
        });
    }
    if stratum.context_fingerprint() != generator.context().fingerprint()
        || owners.context_fingerprint() != generator.context().fingerprint()
        || owners.family_fingerprint() != stratum.family_fingerprint()
    {
        return Err(ProbeLocalSchedulerError::WrongTaskScope {
            detail: "generator, decorated stratum, and immutable owners have different identities",
        });
    }
    match stratum.initial().try_verify(limits.campaign.stratum) {
        Ok(true) => {}
        Ok(false) => {
            return Err(ProbeLocalSchedulerError::Invariant {
                detail: "campaign decorated-stratum anchor failed cold verification",
            });
        }
        Err(error) => return Err(ProbeLocalSchedulerError::Stratum(error)),
    }
    match owners.try_verify(limits.campaign.stratum) {
        Ok(true) => {}
        Ok(false) => {
            return Err(ProbeLocalSchedulerError::Invariant {
                detail: "immutable owner snapshot failed cold verification",
            });
        }
        Err(error) => return Err(ProbeLocalSchedulerError::Stratum(error)),
    }
    Ok(())
}

pub(super) fn admit_probes(
    generator: &ParametricIbpGenerator<'_>,
    stratum: &CampaignStratumAnchor,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: ProbeLocalSchedulerLimits,
) -> Result<Vec<CampaignModularProbe>, ProbeLocalSchedulerError> {
    let expected_base = generator.context().base().parameter_names().len();
    let expected_chart = generator.context().index_count();
    let mut retained = Vec::new();
    let mut coordinate_cells = 0usize;
    for probe in probes {
        let probe_ordinal = retained.len();
        let requested = checked_add(PROBES, probe_ordinal, 1)?;
        check_limit(PROBES, requested, limits.max_probes)?;
        validate_modulus(probe_ordinal, probe.modulus())?;
        if probe.base_parameters().len() != expected_base {
            return Err(ProbeLocalSchedulerError::WrongBaseParameterArity {
                probe_ordinal,
                expected: expected_base,
                actual: probe.base_parameters().len(),
            });
        }
        if probe.chart_coordinates().len() != expected_chart {
            return Err(ProbeLocalSchedulerError::WrongChartCoordinateArity {
                probe_ordinal,
                expected: expected_chart,
                actual: probe.chart_coordinates().len(),
            });
        }
        let probe_cells = checked_add(
            PROBE_COORDINATES,
            probe.base_parameters().len(),
            probe.chart_coordinates().len(),
        )?;
        coordinate_cells = checked_add(PROBE_COORDINATES, coordinate_cells, probe_cells)?;
        check_limit(
            PROBE_COORDINATES,
            coordinate_cells,
            limits.max_retained_probe_coordinate_cells,
        )?;
        retained
            .try_reserve(1)
            .map_err(|_| ProbeLocalSchedulerError::AllocationFailure {
                resource: PROBES,
                requested,
            })?;
        retained.push(probe);
    }
    if retained.is_empty() {
        return Err(ProbeLocalSchedulerError::EmptyProbeSchedule);
    }
    check_limit(OUTCOMES, retained.len(), limits.max_retained_outcomes)?;
    reject_duplicate_probes(&retained, stratum.initial().domain().sector().active_bits())?;
    Ok(retained)
}

fn validate_modulus(probe_ordinal: usize, modulus: u64) -> Result<(), ProbeLocalSchedulerError> {
    if modulus.is_multiple_of(2) {
        return Err(ProbeLocalSchedulerError::UnsupportedEvenModulus {
            probe_ordinal,
            modulus,
        });
    }
    if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        return Err(ProbeLocalSchedulerError::NonPrimeModulus {
            probe_ordinal,
            modulus,
        });
    }
    Ok(())
}

fn reject_duplicate_probes(
    probes: &[CampaignModularProbe],
    active_bits: &[bool],
) -> Result<(), ProbeLocalSchedulerError> {
    let mut order = try_vec(PROBE_KEY_ORDER, probes.len())?;
    order.extend(0..probes.len());
    order.sort_unstable_by(|&left, &right| {
        compare_probe_key(&probes[left], &probes[right], active_bits).then_with(|| left.cmp(&right))
    });
    for pair in order.windows(2) {
        if compare_probe_key(&probes[pair[0]], &probes[pair[1]], active_bits) == Ordering::Equal {
            return Err(ProbeLocalSchedulerError::DuplicateProbe {
                first_ordinal: pair[0],
                duplicate_ordinal: pair[1],
            });
        }
    }
    Ok(())
}

fn compare_probe_key(
    left: &CampaignModularProbe,
    right: &CampaignModularProbe,
    active_bits: &[bool],
) -> Ordering {
    left.modulus()
        .cmp(&right.modulus())
        .then_with(|| {
            compare_base_residues(
                left.base_parameters(),
                right.base_parameters(),
                left.modulus(),
            )
        })
        .then_with(|| {
            compare_chart_index_residues(
                left.chart_coordinates(),
                right.chart_coordinates(),
                active_bits,
                left.modulus(),
            )
        })
}

fn compare_base_residues(left: &[i64], right: &[i64], modulus: u64) -> Ordering {
    for (&left, &right) in left.iter().zip(right) {
        let ordering = signed_residue(left, modulus).cmp(&signed_residue(right, modulus));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_chart_index_residues(
    left: &[u64],
    right: &[u64],
    active_bits: &[bool],
    modulus: u64,
) -> Ordering {
    for ((&left, &right), &active) in left.iter().zip(right).zip(active_bits) {
        let ordering = chart_index_residue(left, active, modulus)
            .cmp(&chart_index_residue(right, active, modulus));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn signed_residue(value: i64, modulus: u64) -> u64 {
    if value >= 0 {
        value.unsigned_abs() % modulus
    } else {
        let magnitude = value.unsigned_abs() % modulus;
        if magnitude == 0 {
            0
        } else {
            modulus - magnitude
        }
    }
}

fn chart_index_residue(coordinate: u64, active: bool, modulus: u64) -> u64 {
    let coordinate = coordinate % modulus;
    if active {
        if coordinate == modulus - 1 {
            0
        } else {
            coordinate + 1
        }
    } else if coordinate == 0 {
        0
    } else {
        modulus - coordinate
    }
}
