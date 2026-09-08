use crate::foundry::completion::stratum::{DecoratedStratum, ImmutableOwnerSnapshot};
use crate::identity::IntegralShift;
use crate::sector::{Error as SectorError, OrderingPolicy};

use super::SpiredFoundationError;

/// One currently executable axis-aligned case.
///
/// The decorated stratum already owns the exact family/context identity,
/// coordinate bounds, sector, and coefficient-guard branches. SpIRed retains
/// that authority instead of introducing a second case representation.
#[derive(Clone, Debug)]
pub(crate) struct SpiredCoordinateFace {
    stratum: DecoratedStratum,
}

impl SpiredCoordinateFace {
    pub(crate) const fn new(stratum: DecoratedStratum) -> Self {
        Self { stratum }
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }
}

/// Placeholder request for coupled affine index equalities.
///
/// Retaining only the ambient arity is intentional: no partial or lossy
/// equation representation may accidentally enter execution before the exact
/// affine-lattice capability exists end to end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredAffineLatticeCase {
    ambient_arity: usize,
}

impl SpiredAffineLatticeCase {
    pub(crate) const fn new(ambient_arity: usize) -> Self {
        Self { ambient_arity }
    }

    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }
}

/// Geometry requested for one targeted completion execution.
#[derive(Clone, Debug)]
pub(crate) enum SpiredCase {
    CoordinateFace(SpiredCoordinateFace),
    AffineLattice(SpiredAffineLatticeCase),
}

/// One self-contained immutable execution identity.
///
/// Construction is the fail-closed join between case geometry, target,
/// ordering, and the actual lower-owner authority. An owner ID is never
/// accepted as a substitute for the retained snapshot.
#[derive(Clone, Debug)]
pub(crate) struct SpiredExecutionCase {
    coordinate_face: SpiredCoordinateFace,
    target_shift: IntegralShift,
    ordering: OrderingPolicy,
    owner_snapshot: ImmutableOwnerSnapshot,
}

impl SpiredExecutionCase {
    pub(crate) fn try_new(
        case: SpiredCase,
        target_shift: IntegralShift,
        ordering: OrderingPolicy,
        owner_snapshot: ImmutableOwnerSnapshot,
    ) -> Result<Self, SpiredFoundationError> {
        let coordinate_face = match case {
            SpiredCase::CoordinateFace(coordinate_face) => coordinate_face,
            SpiredCase::AffineLattice(affine) => {
                return Err(SpiredFoundationError::UnsupportedAffineLattice {
                    ambient_arity: affine.ambient_arity(),
                });
            }
        };
        let stratum = coordinate_face.stratum();
        let arity = stratum.domain().arity();
        if target_shift.len() != arity {
            return Err(SpiredFoundationError::WrongTargetArity {
                expected: arity,
                actual: target_shift.len(),
            });
        }
        if owner_snapshot.arity() != arity {
            return Err(SpiredFoundationError::WrongOwnerArity {
                expected: arity,
                actual: owner_snapshot.arity(),
            });
        }
        if owner_snapshot.family_fingerprint() != stratum.family_fingerprint() {
            return Err(SpiredFoundationError::WrongOwnerFamily);
        }
        if owner_snapshot.context_fingerprint() != stratum.context_fingerprint() {
            return Err(SpiredFoundationError::WrongOwnerContext);
        }
        if let Some(actual) = ordering.coordinate_priority_arity()
            && actual != arity
        {
            return Err(SpiredFoundationError::WrongOrderingArity {
                expected: arity,
                actual,
            });
        }
        if let Some(snapshot_ordering) = owner_snapshot.canonicalizer_ordering()
            && snapshot_ordering != ordering
        {
            return Err(SpiredFoundationError::OwnerOrderingMismatch {
                requested: ordering,
                snapshot: snapshot_ordering,
            });
        }
        match ordering.prove_sector_monotone_shift_descent(
            stratum.domain(),
            target_shift.values(),
            target_shift.values(),
        ) {
            Err(SectorError::NotStrictDescent) => {}
            Err(error) => return Err(SpiredFoundationError::InvalidTargetShift(error)),
            Ok(_) => {
                return Err(SpiredFoundationError::Invariant {
                    detail: "an ordering proved one target shift strictly below itself",
                });
            }
        }
        Ok(Self {
            coordinate_face,
            target_shift,
            ordering,
            owner_snapshot,
        })
    }

    pub(crate) const fn coordinate_face(&self) -> &SpiredCoordinateFace {
        &self.coordinate_face
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        self.coordinate_face.stratum()
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn owner_snapshot(&self) -> &ImmutableOwnerSnapshot {
        &self.owner_snapshot
    }

    pub(crate) fn arity(&self) -> usize {
        self.stratum().domain().arity()
    }
}
