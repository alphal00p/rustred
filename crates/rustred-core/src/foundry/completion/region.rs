use crate::foundry::cell::RuleCell;
use crate::sector::SectorMonotoneDomain;

use super::model::try_vec;
use super::{CompletionGeometryError, LatticeBox, SectorChart};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OuterPowerDirection {
    Increasing,
    Decreasing,
}

/// An unresolved obligation to extend one finite carrier endpoint into a
/// genuine mathematical ray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OuterExtensionObligation {
    position: usize,
    power_direction: OuterPowerDirection,
    target_endpoint: i64,
    reaches_maximal_rule_application_endpoint: bool,
    reaches_carrier_endpoint: bool,
}

impl OuterExtensionObligation {
    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn power_direction(self) -> OuterPowerDirection {
        self.power_direction
    }

    pub(crate) const fn target_endpoint(self) -> i64 {
        self.target_endpoint
    }

    /// Maximal rule-safe application is a candidate for an asymptotic proof,
    /// never the proof itself.
    pub(crate) const fn reaches_maximal_rule_application_endpoint(self) -> bool {
        self.reaches_maximal_rule_application_endpoint
    }

    /// Whether this target endpoint is the corresponding finite chart-carrier
    /// endpoint. Carrier contact is never a proof of mathematical infinity.
    pub(crate) const fn reaches_carrier_endpoint(self) -> bool {
        self.reaches_carrier_endpoint
    }
}

/// Exact guard-blind structural target box of a RuleCell on the finite i64
/// carrier.
///
/// Guards are counted but deliberately not interpreted here. The box is only
/// a potential ownership domain; guard zero loci can add uncovered points.
/// Every outer endpoint remains an explicit asymptotic-extension obligation
/// even when it reaches the maximal representable carrier domain.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GuardBlindCarrierRegion {
    structural_target_box: LatticeBox,
    outer_extensions: Box<[OuterExtensionObligation]>,
    guard_count: usize,
}

impl GuardBlindCarrierRegion {
    pub(crate) fn try_from_cell(
        chart: &SectorChart,
        cell: &RuleCell,
    ) -> Result<Self, CompletionGeometryError> {
        let domain = cell.application_domain();
        if chart.sector() != domain.sector() {
            return Err(CompletionGeometryError::RuleCellSectorMismatch);
        }
        let arity = domain.arity();
        let pivot = cell.rule().pivot().values();
        if pivot.len() != arity {
            return Err(CompletionGeometryError::WrongArity {
                object: "rule-cell pivot",
                expected: arity,
                actual: pivot.len(),
            });
        }

        let mut retained_rhs = try_vec("retained rule-cell right-hand sides", cell.terms().len())?;
        for term in cell.terms() {
            retained_rhs.push(
                cell.rule().right_hand_side()[term.source_rhs_ordinal()]
                    .shift()
                    .values(),
            );
        }
        let maximal = SectorMonotoneDomain::try_maximal_for_rule(
            domain.sector().clone(),
            pivot,
            &retained_rhs,
        )
        .map_err(|_| CompletionGeometryError::RuleDomainReconstruction)?;

        let mut lower = Vec::new();
        let mut upper = Vec::new();
        let mut outer_extensions = Vec::new();
        lower
            .try_reserve_exact(arity)
            .map_err(|_| CompletionGeometryError::AllocationFailure {
                resource: "rule-region lower endpoints",
                requested: arity,
            })?;
        upper
            .try_reserve_exact(arity)
            .map_err(|_| CompletionGeometryError::AllocationFailure {
                resource: "rule-region upper endpoints",
                requested: arity,
            })?;
        outer_extensions.try_reserve_exact(arity).map_err(|_| {
            CompletionGeometryError::AllocationFailure {
                resource: "rule-region outer-extension obligations",
                requested: arity,
            }
        })?;

        for (position, (((&bounds, &maximal_bounds), &pivot), &active)) in domain
            .bounds()
            .iter()
            .zip(maximal.bounds())
            .zip(pivot)
            .zip(domain.sector().active_bits())
            .enumerate()
        {
            let target_lower = checked_target_endpoint(bounds.lower(), pivot, position, "lower")?;
            let target_upper = checked_target_endpoint(bounds.upper(), pivot, position, "upper")?;
            if active {
                lower.push(u64::try_from(i128::from(target_lower) - 1).map_err(|_| {
                    CompletionGeometryError::TargetEndpointNotRepresentable {
                        position,
                        endpoint: "lower",
                    }
                })?);
                upper.push(Some(u64::try_from(i128::from(target_upper) - 1).map_err(
                    |_| CompletionGeometryError::TargetEndpointNotRepresentable {
                        position,
                        endpoint: "upper",
                    },
                )?));
                outer_extensions.push(OuterExtensionObligation {
                    position,
                    power_direction: OuterPowerDirection::Increasing,
                    target_endpoint: target_upper,
                    reaches_maximal_rule_application_endpoint: bounds.upper()
                        == maximal_bounds.upper(),
                    reaches_carrier_endpoint: target_upper == i64::MAX,
                });
            } else {
                lower.push(u64::try_from(-i128::from(target_upper)).map_err(|_| {
                    CompletionGeometryError::TargetEndpointNotRepresentable {
                        position,
                        endpoint: "upper",
                    }
                })?);
                upper.push(Some(u64::try_from(-i128::from(target_lower)).map_err(
                    |_| CompletionGeometryError::TargetEndpointNotRepresentable {
                        position,
                        endpoint: "lower",
                    },
                )?));
                outer_extensions.push(OuterExtensionObligation {
                    position,
                    power_direction: OuterPowerDirection::Decreasing,
                    target_endpoint: target_lower,
                    reaches_maximal_rule_application_endpoint: bounds.lower()
                        == maximal_bounds.lower(),
                    reaches_carrier_endpoint: target_lower == i64::MIN,
                });
            }
        }

        Ok(Self {
            structural_target_box: LatticeBox::try_from_preallocated(lower, upper)?,
            outer_extensions: outer_extensions.into_boxed_slice(),
            guard_count: cell.guards().len(),
        })
    }

    pub(crate) fn structural_target_box(&self) -> &LatticeBox {
        &self.structural_target_box
    }

    pub(crate) fn into_structural_target_box(self) -> LatticeBox {
        self.structural_target_box
    }

    pub(crate) fn outer_extensions(&self) -> &[OuterExtensionObligation] {
        &self.outer_extensions
    }

    pub(crate) const fn guard_count(&self) -> usize {
        self.guard_count
    }
}

fn checked_target_endpoint(
    assignment: i64,
    pivot: i64,
    position: usize,
    endpoint: &'static str,
) -> Result<i64, CompletionGeometryError> {
    i64::try_from(i128::from(assignment) + i128::from(pivot))
        .map_err(|_| CompletionGeometryError::TargetEndpointNotRepresentable { position, endpoint })
}
