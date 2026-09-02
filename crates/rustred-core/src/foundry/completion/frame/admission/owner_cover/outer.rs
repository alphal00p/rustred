//! Cold proof that replayed finite-discovery circuits extend geometrically to a ray.

use std::fmt;
use std::sync::Arc;

use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::foundry::completion::frame::admission::semantic::ExactCircuitSemanticDag;
use crate::foundry::completion::stratum::{
    DecoratedStratumId, ImmutableOwnerSnapshotId, TargetColumnPartition,
};
use crate::foundry::completion::{CompletionGeometryError, LatticeBox, LatticePoint};
use crate::sector::{Error as SectorError, Mask, OrderingPolicy, SectorMonotoneDomain};

/// Opaque evidence for the exact target region owned by one replayed stratum.
///
/// A maximal carrier whose recurrence remains globally same-sector may extend
/// to a leading orthant only along axes on which its executable application
/// domain reaches the corresponding `i64` target carrier. A tightened face/ray
/// follows the same rule, producing an exact cylinder box. Fixed, otherwise
/// tightened, or representability-limited axes remain bounded. Exact circuit
/// guards are handled separately by the semantic DAG; this witness alone
/// grants no ownership.
#[derive(Debug)]
pub(crate) struct ExactCircuitOuterExtensionWitness<'frame> {
    /// The exact physical plan against which the partition, semantic DAG,
    /// and target-column ordinal were proved.  This in-memory proof witness
    /// is deliberately pointer-bound: equal ordinals in independently built
    /// selected plans need not denote the same integral shift.
    pub(super) plan: &'frame PhysicalFramePlan,
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) sector: Mask,
    pub(super) ordering: OrderingPolicy,
    pub(super) stratum_id: DecoratedStratumId,
    pub(super) owner_snapshot_id: ImmutableOwnerSnapshotId,
    pub(super) target_column: usize,
    pub(super) leading: LatticePoint,
    pub(super) region: LatticeBox,
    pub(super) semantic: Arc<ExactCircuitSemanticDag>,
}

impl<'frame> ExactCircuitOuterExtensionWitness<'frame> {
    pub(crate) const fn region(&self) -> &LatticeBox {
        &self.region
    }

    pub(crate) fn try_prove(
        partition: &TargetColumnPartition<'frame>,
        semantic: Arc<ExactCircuitSemanticDag>,
    ) -> Result<Self, ExactCircuitOuterExtensionError> {
        if !semantic.is_bound_to(partition.frame()) {
            return Err(ExactCircuitOuterExtensionError::WrongPhysicalPlan);
        }
        if semantic.context_fingerprint() != partition.frame().context_fingerprint() {
            return Err(ExactCircuitOuterExtensionError::WrongContext);
        }
        if !partition.stratum().guards().is_empty() {
            return Err(
                ExactCircuitOuterExtensionError::PreexistingStratumPredicates {
                    count: partition.stratum().guards().len(),
                },
            );
        }
        let pivot = partition.frame().columns()[partition.target_column()].values();
        let mut frame_shifts = Vec::new();
        frame_shifts
            .try_reserve_exact(partition.frame().columns().len())
            .map_err(|_| ExactCircuitOuterExtensionError::AllocationFailure {
                resource: "outer-extension frame shifts",
                requested: partition.frame().columns().len(),
            })?;
        frame_shifts.extend(
            partition
                .frame()
                .columns()
                .iter()
                .map(|shift| shift.values()),
        );
        let maximal = SectorMonotoneDomain::try_maximal_for_rule(
            partition.frame().sector().clone(),
            pivot,
            &frame_shifts,
        )?;
        if semantic.candidates().is_empty() {
            return Err(ExactCircuitOuterExtensionError::EmptySemanticDag);
        }

        let arity = partition.frame().sector().arity();
        let mut fixed = Vec::new();
        fixed.try_reserve_exact(arity).map_err(|_| {
            ExactCircuitOuterExtensionError::AllocationFailure {
                resource: "outer-extension fixed quotient coordinates",
                requested: arity,
            }
        })?;
        fixed.extend(partition.stratum().singleton_index_assignments());
        let globally_extended = &maximal == partition.stratum().domain() && fixed.is_empty();
        for (candidate_ordinal, candidate) in semantic.candidates().iter().enumerate() {
            let circuit = candidate.circuit();
            if circuit.stratum_id() != partition.stratum_id()
                || circuit.owner_snapshot_id() != partition.snapshot_id()
                || circuit.target_column() != partition.target_column()
                || circuit.target_shift().values() != pivot
            {
                return Err(ExactCircuitOuterExtensionError::CandidateJoin {
                    candidate: candidate_ordinal,
                    detail: "semantic circuit differs from its target partition",
                });
            }
            if circuit.fixed_indices() != fixed.as_slice() {
                return Err(ExactCircuitOuterExtensionError::CandidateJoin {
                    candidate: candidate_ordinal,
                    detail: "semantic circuit fixed quotient differs from the partition singleton coordinates",
                });
            }
            for (term_ordinal, term) in circuit.residual_terms().iter().enumerate() {
                let descent = term.descent();
                if !descent.verify()
                    || descent.policy() != partition.ordering()
                    || descent.domain() != partition.stratum().domain()
                    || !key_matches_shift(descent.pivot(), pivot)
                    || !key_matches_shift(descent.target(), term.shift().values())
                {
                    return Err(ExactCircuitOuterExtensionError::InvalidGlobalDescent {
                        candidate: candidate_ordinal,
                        term: term_ordinal,
                    });
                }
            }
        }

        let (mut leading, upper) = target_region_from_stratum(
            partition.stratum().domain(),
            &maximal,
            partition.frame().sector(),
            pivot,
            &fixed,
        )?;
        if globally_extended {
            for (candidate_ordinal, candidate) in semantic.candidates().iter().enumerate() {
                for (term_ordinal, term) in candidate.circuit().residual_terms().iter().enumerate()
                {
                    if term.descent().target() >= term.descent().pivot() {
                        return Err(ExactCircuitOuterExtensionError::InvalidGlobalDescent {
                            candidate: candidate_ordinal,
                            term: term_ordinal,
                        });
                    }
                    tighten_same_sector_origin(
                        &mut leading,
                        partition.frame().sector(),
                        pivot,
                        term.shift().values(),
                    )?;
                }
            }
        }
        let region = LatticeBox::try_new(leading.iter().copied(), upper.iter().copied())?;

        Ok(Self {
            plan: partition.frame(),
            family_fingerprint: Arc::new(partition.frame().family_fingerprint().to_owned()),
            context_fingerprint: Arc::new(partition.frame().context_fingerprint().to_owned()),
            sector: partition.frame().sector().clone(),
            ordering: partition.ordering(),
            stratum_id: partition.stratum_id().clone(),
            owner_snapshot_id: partition.snapshot_id().clone(),
            target_column: partition.target_column(),
            leading: LatticePoint::try_new(leading)?,
            region,
            semantic,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitOuterExtensionError {
    WrongPhysicalPlan,
    WrongContext,
    PreexistingStratumPredicates {
        count: usize,
    },
    EmptySemanticDag,
    CandidateJoin {
        candidate: usize,
        detail: &'static str,
    },
    InvalidGlobalDescent {
        candidate: usize,
        term: usize,
    },
    InvalidTargetBoundary {
        position: usize,
    },
    Sector(SectorError),
    Geometry(CompletionGeometryError),
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
}

impl fmt::Display for ExactCircuitOuterExtensionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPhysicalPlan => formatter
                .write_str("outer-extension proof uses a semantic DAG from another physical plan"),
            Self::WrongContext => {
                formatter.write_str("outer-extension proof uses another coefficient context")
            }
            Self::PreexistingStratumPredicates { count } => write!(
                formatter,
                "outer-extension proof cannot extrapolate {count} pre-existing stratum predicates"
            ),
            Self::EmptySemanticDag => {
                formatter.write_str("outer-extension proof has no exact semantic candidate")
            }
            Self::CandidateJoin { candidate, detail } => write!(
                formatter,
                "outer-extension candidate {candidate} failed its cold join: {detail}"
            ),
            Self::InvalidGlobalDescent { candidate, term } => write!(
                formatter,
                "outer-extension candidate {candidate}, term {term} is not globally same-sector descending"
            ),
            Self::InvalidTargetBoundary { position } => write!(
                formatter,
                "outer-extension target boundary at coordinate {position} is not a nonnegative lattice coordinate"
            ),
            Self::Sector(error) => error.fmt(formatter),
            Self::Geometry(error) => error.fmt(formatter),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for {resource}"
            ),
        }
    }
}

fn target_region_from_stratum(
    domain: &SectorMonotoneDomain,
    maximal: &SectorMonotoneDomain,
    sector: &Mask,
    pivot: &[i64],
    fixed: &[(usize, i64)],
) -> Result<(Vec<u64>, Vec<Option<u64>>), ExactCircuitOuterExtensionError> {
    let arity = sector.arity();
    let mut lower = Vec::new();
    lower.try_reserve_exact(arity).map_err(|_| {
        ExactCircuitOuterExtensionError::AllocationFailure {
            resource: "outer-extension lower coordinates",
            requested: arity,
        }
    })?;
    let mut upper = Vec::new();
    upper.try_reserve_exact(arity).map_err(|_| {
        ExactCircuitOuterExtensionError::AllocationFailure {
            resource: "outer-extension upper coordinates",
            requested: arity,
        }
    })?;
    for (position, (((&bounds, &maximal), &pivot), &active)) in domain
        .bounds()
        .iter()
        .zip(maximal.bounds())
        .zip(pivot)
        .zip(sector.active_bits())
        .enumerate()
    {
        let fixed_position = fixed
            .binary_search_by_key(&position, |&(fixed_position, _)| fixed_position)
            .is_ok();
        let integral_target_lower = i128::from(bounds.lower()) + i128::from(pivot);
        let integral_target_upper = i128::from(bounds.upper()) + i128::from(pivot);
        let (target_lower, target_upper, outward_reaches_carrier) = if active {
            (
                integral_target_lower - 1,
                integral_target_upper - 1,
                !fixed_position
                    && bounds.upper() == maximal.upper()
                    && integral_target_upper == i128::from(i64::MAX),
            )
        } else {
            (
                -integral_target_upper,
                -integral_target_lower,
                !fixed_position
                    && bounds.lower() == maximal.lower()
                    && integral_target_lower == i128::from(i64::MIN),
            )
        };
        lower
            .push(u64::try_from(target_lower).map_err(|_| {
                ExactCircuitOuterExtensionError::InvalidTargetBoundary { position }
            })?);
        upper.push(if outward_reaches_carrier {
            None
        } else {
            Some(
                u64::try_from(target_upper).map_err(|_| {
                    ExactCircuitOuterExtensionError::InvalidTargetBoundary { position }
                })?,
            )
        });
    }
    Ok((lower, upper))
}

impl std::error::Error for ExactCircuitOuterExtensionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sector(error) => Some(error),
            Self::Geometry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SectorError> for ExactCircuitOuterExtensionError {
    fn from(value: SectorError) -> Self {
        Self::Sector(value)
    }
}

impl From<CompletionGeometryError> for ExactCircuitOuterExtensionError {
    fn from(value: CompletionGeometryError) -> Self {
        Self::Geometry(value)
    }
}

fn key_matches_shift(key: &crate::sector::ShiftComplexityKey, shift: &[i64]) -> bool {
    key.arity() == shift.len()
        && shift.iter().enumerate().all(|(position, &shift)| {
            key.shift_at(position)
                .is_ok_and(|retained| retained == shift)
        })
}

fn tighten_same_sector_origin(
    origin: &mut [u64],
    sector: &Mask,
    pivot: &[i64],
    residual: &[i64],
) -> Result<(), ExactCircuitOuterExtensionError> {
    for (position, ((origin, &active), (&pivot, &residual))) in origin
        .iter_mut()
        .zip(sector.active_bits())
        .zip(pivot.iter().zip(residual))
        .enumerate()
    {
        let threshold = if active {
            i128::from(pivot) - i128::from(residual)
        } else {
            i128::from(residual) - i128::from(pivot)
        }
        .max(0);
        let threshold = u64::try_from(threshold)
            .map_err(|_| ExactCircuitOuterExtensionError::InvalidTargetBoundary { position })?;
        *origin = (*origin).max(threshold);
    }
    Ok(())
}
