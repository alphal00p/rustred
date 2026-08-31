//! Canonical structural ordering for replayed exact-circuit proof payloads.

use std::cmp::Ordering;

use symbolica::prelude::{Integer, IntegerRing, MultivariatePolynomial};

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::completion::frame::exact::{
    ExactCircuitGuard, ExactCircuitGuardOrigin, ExactCircuitPivotGuard, ExactCircuitTerm,
    ExactFrameSourceContribution, ExactTargetCircuit,
};
use crate::foundry::completion::stratum::{ImmutableOwnerWitness, ProperSubsectorOwner};
use crate::sector::{
    ComplexityComponent, SectorInteriorDomain, SectorMonotoneDomain,
    SectorMonotoneShiftDescentWitness, ShiftStrictDescentWitness,
};

/// Full structural comparison of exact proof content. Modular sample and
/// modular diagnostics are intentionally excluded after join validation.
pub(in crate::foundry::completion::frame::admission) fn compare_exact_content(
    left: &ExactTargetCircuit,
    right: &ExactTargetCircuit,
) -> Ordering {
    left.stratum_id()
        .as_str()
        .cmp(right.stratum_id().as_str())
        .then_with(|| {
            left.owner_snapshot_id()
                .as_str()
                .cmp(right.owner_snapshot_id().as_str())
        })
        .then_with(|| left.target_column().cmp(&right.target_column()))
        .then_with(|| left.target_shift().cmp(right.target_shift()))
        .then_with(|| cmp_slice_by(left.residual_terms(), right.residual_terms(), cmp_term))
        .then_with(|| {
            cmp_slice_by(
                left.source_combination(),
                right.source_combination(),
                cmp_source_contribution,
            )
        })
        .then_with(|| cmp_slice_by(left.pivot_guards(), right.pivot_guards(), cmp_pivot))
        .then_with(|| cmp_slice_by(left.nonzero_guards(), right.nonzero_guards(), cmp_guard))
        .then_with(|| {
            let left = left.replay();
            let right = right.replay();
            left.source_contributions()
                .cmp(&right.source_contributions())
                .then_with(|| left.source_terms().cmp(&right.source_terms()))
                .then_with(|| left.physical_columns().cmp(&right.physical_columns()))
                .then_with(|| left.exact_operations().cmp(&right.exact_operations()))
        })
}

fn cmp_term(left: &ExactCircuitTerm, right: &ExactCircuitTerm) -> Ordering {
    left.physical_column()
        .cmp(&right.physical_column())
        .then_with(|| left.shift().cmp(right.shift()))
        .then_with(|| cmp_coefficient(left.coefficient(), right.coefficient()))
        .then_with(|| cmp_descent(left.descent(), right.descent()))
        .then_with(|| {
            cmp_slice_by(
                left.proper_subsector_owners(),
                right.proper_subsector_owners(),
                cmp_proper_owner,
            )
        })
}

fn cmp_source_contribution(
    left: &ExactFrameSourceContribution,
    right: &ExactFrameSourceContribution,
) -> Ordering {
    left.frame_row_ordinal()
        .cmp(&right.frame_row_ordinal())
        .then_with(|| left.source_instance().cmp(right.source_instance()))
        .then_with(|| cmp_coefficient(left.coefficient(), right.coefficient()))
}

fn cmp_pivot(left: &ExactCircuitPivotGuard, right: &ExactCircuitPivotGuard) -> Ordering {
    left.frame_row_ordinal()
        .cmp(&right.frame_row_ordinal())
        .then_with(|| left.source_instance().cmp(right.source_instance()))
        .then_with(|| {
            left.physical_pivot_column()
                .cmp(&right.physical_pivot_column())
        })
        .then_with(|| cmp_coefficient(left.coefficient(), right.coefficient()))
        .then_with(|| cmp_polynomial(left.nonzero_polynomial(), right.nonzero_polynomial()))
}

fn cmp_guard(left: &ExactCircuitGuard, right: &ExactCircuitGuard) -> Ordering {
    cmp_polynomial(left.polynomial(), right.polynomial())
        .then_with(|| cmp_slice_by(left.origins(), right.origins(), cmp_origin))
}

fn cmp_origin(left: &ExactCircuitGuardOrigin, right: &ExactCircuitGuardOrigin) -> Ordering {
    origin_rank(left)
        .cmp(&origin_rank(right))
        .then_with(|| match (left, right) {
            (
                ExactCircuitGuardOrigin::SourceCondition {
                    frame_row_ordinal: lrow,
                    source_instance: lsource,
                    condition_ordinal: lordinal,
                    condition_sources: lsources,
                },
                ExactCircuitGuardOrigin::SourceCondition {
                    frame_row_ordinal: rrow,
                    source_instance: rsource,
                    condition_ordinal: rordinal,
                    condition_sources: rsources,
                },
            ) => lrow
                .cmp(rrow)
                .then_with(|| lsource.cmp(rsource))
                .then_with(|| lordinal.cmp(rordinal))
                .then_with(|| lsources.cmp(rsources)),
            (
                ExactCircuitGuardOrigin::SourceCoefficientDenominator {
                    frame_row_ordinal: lrow,
                    source_instance: lsource,
                    physical_column: lcolumn,
                },
                ExactCircuitGuardOrigin::SourceCoefficientDenominator {
                    frame_row_ordinal: rrow,
                    source_instance: rsource,
                    physical_column: rcolumn,
                },
            ) => lrow
                .cmp(rrow)
                .then_with(|| lsource.cmp(rsource))
                .then_with(|| lcolumn.cmp(rcolumn)),
            (
                ExactCircuitGuardOrigin::ReducerPivotNumerator {
                    frame_row_ordinal: lrow,
                    source_instance: lsource,
                    physical_pivot_column: lcolumn,
                }
                | ExactCircuitGuardOrigin::ReducerPivotDenominator {
                    frame_row_ordinal: lrow,
                    source_instance: lsource,
                    physical_pivot_column: lcolumn,
                },
                ExactCircuitGuardOrigin::ReducerPivotNumerator {
                    frame_row_ordinal: rrow,
                    source_instance: rsource,
                    physical_pivot_column: rcolumn,
                }
                | ExactCircuitGuardOrigin::ReducerPivotDenominator {
                    frame_row_ordinal: rrow,
                    source_instance: rsource,
                    physical_pivot_column: rcolumn,
                },
            ) => lrow
                .cmp(rrow)
                .then_with(|| lsource.cmp(rsource))
                .then_with(|| lcolumn.cmp(rcolumn)),
            (
                ExactCircuitGuardOrigin::SourceMultiplierDenominator {
                    frame_row_ordinal: lrow,
                    source_instance: lsource,
                },
                ExactCircuitGuardOrigin::SourceMultiplierDenominator {
                    frame_row_ordinal: rrow,
                    source_instance: rsource,
                },
            ) => lrow.cmp(rrow).then_with(|| lsource.cmp(rsource)),
            (
                ExactCircuitGuardOrigin::ResidualCoefficientDenominator {
                    physical_column: left,
                },
                ExactCircuitGuardOrigin::ResidualCoefficientDenominator {
                    physical_column: right,
                },
            ) => left.cmp(right),
            _ => Ordering::Equal,
        })
}

fn origin_rank(origin: &ExactCircuitGuardOrigin) -> u8 {
    match origin {
        ExactCircuitGuardOrigin::SourceCondition { .. } => 0,
        ExactCircuitGuardOrigin::SourceCoefficientDenominator { .. } => 1,
        ExactCircuitGuardOrigin::ReducerPivotNumerator { .. } => 2,
        ExactCircuitGuardOrigin::ReducerPivotDenominator { .. } => 3,
        ExactCircuitGuardOrigin::SourceMultiplierDenominator { .. } => 4,
        ExactCircuitGuardOrigin::ResidualCoefficientDenominator { .. } => 5,
    }
}

fn cmp_coefficient(left: &IndexedCoefficient, right: &IndexedCoefficient) -> Ordering {
    cmp_raw_polynomial(&left.raw().numerator, &right.raw().numerator)
        .then_with(|| cmp_raw_polynomial(&left.raw().denominator, &right.raw().denominator))
}

fn cmp_polynomial(left: &IndexedPolynomial, right: &IndexedPolynomial) -> Ordering {
    cmp_raw_polynomial(left.raw(), right.raw())
}

fn cmp_raw_polynomial(
    left: &MultivariatePolynomial<IntegerRing, u16>,
    right: &MultivariatePolynomial<IntegerRing, u16>,
) -> Ordering {
    left.exponents
        .cmp(&right.exponents)
        .then_with(|| cmp_slice_by(&left.coefficients, &right.coefficients, Integer::cmp))
}

fn cmp_descent(
    left: &SectorMonotoneShiftDescentWitness,
    right: &SectorMonotoneShiftDescentWitness,
) -> Ordering {
    left.policy()
        .cmp(&right.policy())
        .then_with(|| cmp_monotone_domain(left.domain(), right.domain()))
        .then_with(|| left.pivot().cmp(right.pivot()))
        .then_with(|| left.target().cmp(right.target()))
        .then_with(|| {
            cmp_option_by(
                left.same_sector_descent(),
                right.same_sector_descent(),
                cmp_strict,
            )
        })
        .then_with(|| {
            cmp_slice_by(left.thresholds(), right.thresholds(), |left, right| {
                left.position()
                    .cmp(&right.position())
                    .then_with(|| left.pinched_upper().cmp(&right.pinched_upper()))
                    .then_with(|| left.same_sector_lower().cmp(&right.same_sector_lower()))
            })
        })
}

fn cmp_monotone_domain(left: &SectorMonotoneDomain, right: &SectorMonotoneDomain) -> Ordering {
    left.sector()
        .cmp(right.sector())
        .then_with(|| cmp_bounds(left.bounds(), right.bounds()))
}

fn cmp_interior_domain(left: &SectorInteriorDomain, right: &SectorInteriorDomain) -> Ordering {
    left.sector()
        .cmp(right.sector())
        .then_with(|| cmp_bounds(left.bounds(), right.bounds()))
}

fn cmp_bounds(
    left: &[crate::sector::InteriorBounds],
    right: &[crate::sector::InteriorBounds],
) -> Ordering {
    cmp_slice_by(left, right, |left, right| {
        left.lower()
            .cmp(&right.lower())
            .then_with(|| left.upper().cmp(&right.upper()))
    })
}

fn cmp_strict(left: &ShiftStrictDescentWitness, right: &ShiftStrictDescentWitness) -> Ordering {
    left.policy()
        .cmp(&right.policy())
        .then_with(|| cmp_interior_domain(left.domain(), right.domain()))
        .then_with(|| left.source().cmp(right.source()))
        .then_with(|| left.target().cmp(right.target()))
        .then_with(|| cmp_component(left.decisive_component(), right.decisive_component()))
}

fn cmp_component(left: ComplexityComponent, right: ComplexityComponent) -> Ordering {
    component_key(left).cmp(&component_key(right))
}

fn component_key(component: ComplexityComponent) -> (u8, usize) {
    match component {
        ComplexityComponent::Arity => (0, 0),
        ComplexityComponent::PropagatorCount => (1, 0),
        ComplexityComponent::SectorBit { position } => (2, position),
        ComplexityComponent::CornerDistance => (3, 0),
        ComplexityComponent::DotPower => (4, 0),
        ComplexityComponent::NumeratorPower => (5, 0),
        ComplexityComponent::IndexExcess { position } => (6, position),
    }
}

fn cmp_proper_owner(left: &ProperSubsectorOwner, right: &ProperSubsectorOwner) -> Ordering {
    left.cell_ordinal()
        .cmp(&right.cell_ordinal())
        .then_with(|| cmp_owner_witness(left.owner(), right.owner()))
}

fn cmp_owner_witness(left: ImmutableOwnerWitness, right: ImmutableOwnerWitness) -> Ordering {
    left.owner_ordinal()
        .cmp(&right.owner_ordinal())
        .then_with(|| left.kind().cmp(&right.kind()))
}

fn cmp_slice_by<T>(left: &[T], right: &[T], mut cmp: impl FnMut(&T, &T) -> Ordering) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = cmp(left, right);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn cmp_option_by<T>(
    left: Option<&T>,
    right: Option<&T>,
    cmp: impl FnOnce(&T, &T) -> Ordering,
) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => cmp(left, right),
    }
}

#[cfg(test)]
pub(crate) fn exact_content_equal_excluding_modular_telemetry(
    left: &ExactTargetCircuit,
    right: &ExactTargetCircuit,
) -> bool {
    left.stratum_id() == right.stratum_id()
        && left.owner_snapshot_id() == right.owner_snapshot_id()
        && left.target_column() == right.target_column()
        && left.target_shift() == right.target_shift()
        && left.residual_terms() == right.residual_terms()
        && left.source_combination() == right.source_combination()
        && left.pivot_guards() == right.pivot_guards()
        && left.nonzero_guards() == right.nonzero_guards()
        && left.replay() == right.replay()
}
