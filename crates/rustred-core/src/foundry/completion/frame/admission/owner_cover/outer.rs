//! Cold proof that replayed finite-discovery circuits extend geometrically to a ray.

use std::fmt;
use std::sync::Arc;

use crate::foundry::completion::frame::admission::semantic::ExactCircuitSemanticDag;
use crate::foundry::completion::stratum::{
    DecoratedStratumId, ImmutableOwnerSnapshotId, TargetColumnPartition,
};
use crate::foundry::completion::{CompletionGeometryError, LatticePoint};
use crate::sector::{Error as SectorError, Mask, OrderingPolicy, SectorMonotoneDomain};

/// Opaque evidence that the finite carrier domain was only a discovery
/// envelope and that every retained exact candidate remains same-sector and
/// descending on the resulting infinite orthant.
///
/// Construction rejects tightened domains and every pre-existing decorated
/// guard branch. Exact circuit guards remain handled by the semantic DAG. The
/// leading origin is tightened until every RHS stays in the same sector, so
/// no lower-sector owner is extrapolated beyond its finite proof domain.
/// This witness alone grants no ownership: semantic guard totality is proved
/// separately by the owner-cover compiler.
#[derive(Debug)]
pub(crate) struct ExactCircuitOuterExtensionWitness {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) sector: Mask,
    pub(super) ordering: OrderingPolicy,
    pub(super) stratum_id: DecoratedStratumId,
    pub(super) owner_snapshot_id: ImmutableOwnerSnapshotId,
    pub(super) target_column: usize,
    pub(super) leading: LatticePoint,
    pub(super) semantic: Arc<ExactCircuitSemanticDag>,
}

impl ExactCircuitOuterExtensionWitness {
    pub(crate) fn try_prove(
        partition: &TargetColumnPartition<'_>,
        semantic: Arc<ExactCircuitSemanticDag>,
    ) -> Result<Self, ExactCircuitOuterExtensionError> {
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
        if &maximal != partition.stratum().domain() {
            return Err(ExactCircuitOuterExtensionError::TightenedCarrierDomain);
        }
        if semantic.candidates().is_empty() {
            return Err(ExactCircuitOuterExtensionError::EmptySemanticDag);
        }

        let arity = partition.frame().sector().arity();
        let mut leading = Vec::new();
        leading.try_reserve_exact(arity).map_err(|_| {
            ExactCircuitOuterExtensionError::AllocationFailure {
                resource: "outer-extension leading coordinates",
                requested: arity,
            }
        })?;
        for (position, ((&bounds, &pivot), &active)) in partition
            .stratum()
            .domain()
            .bounds()
            .iter()
            .zip(pivot)
            .zip(partition.frame().sector().active_bits())
            .enumerate()
        {
            let target_boundary = if active {
                i128::from(bounds.lower()) + i128::from(pivot) - 1
            } else {
                -(i128::from(bounds.upper()) + i128::from(pivot))
            };
            let coordinate = u64::try_from(target_boundary)
                .map_err(|_| ExactCircuitOuterExtensionError::InvalidTargetBoundary { position })?;
            leading.push(coordinate);
        }

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
            for (term_ordinal, term) in circuit.residual_terms().iter().enumerate() {
                let descent = term.descent();
                if !descent.verify()
                    || descent.policy() != partition.ordering()
                    || descent.domain().sector() != partition.frame().sector()
                    || !key_matches_shift(descent.pivot(), pivot)
                    || !key_matches_shift(descent.target(), term.shift().values())
                    || descent.target() >= descent.pivot()
                {
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

        Ok(Self {
            family_fingerprint: Arc::new(partition.frame().family_fingerprint().to_owned()),
            context_fingerprint: Arc::new(partition.frame().context_fingerprint().to_owned()),
            sector: partition.frame().sector().clone(),
            ordering: partition.ordering(),
            stratum_id: partition.stratum_id().clone(),
            owner_snapshot_id: partition.snapshot_id().clone(),
            target_column: partition.target_column(),
            leading: LatticePoint::try_new(leading)?,
            semantic,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitOuterExtensionError {
    WrongContext,
    PreexistingStratumPredicates {
        count: usize,
    },
    TightenedCarrierDomain,
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
            Self::WrongContext => {
                formatter.write_str("outer-extension proof uses another coefficient context")
            }
            Self::PreexistingStratumPredicates { count } => write!(
                formatter,
                "outer-extension proof cannot extrapolate {count} pre-existing stratum predicates"
            ),
            Self::TightenedCarrierDomain => formatter.write_str(
                "outer-extension proof requires the reconstructed maximal finite carrier domain",
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
