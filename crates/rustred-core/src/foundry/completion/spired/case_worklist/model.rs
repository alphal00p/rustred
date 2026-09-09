use std::sync::Arc;

use crate::foundry::completion::stratum::DecoratedStratum;

use super::error::SpiredCoordinateCaseWorklistError;

/// One exact axis-aligned logical obligation.
///
/// Discovery cases encode coordinate equalities only. Guard inequalities and
/// first-zero branch chronology belong to exact owner-domain compilation and
/// cannot cross this constructor. The obligation carries no target, terminal
/// policy, solver result, or closure capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateCaseObligation {
    /// Explicit logical carrier declared by the caller. A free case axis must
    /// retain exactly this axis interval; only exact singleton equalities may
    /// tighten it. In particular, finite-depth search and representability
    /// envelopes can never be smuggled into the logical worklist.
    declared_carrier: Arc<DecoratedStratum>,
    stratum: DecoratedStratum,
}

impl SpiredCoordinateCaseObligation {
    /// Declare a guard-blind stratum as the complete logical carrier and its
    /// initial unconstrained equality case.
    ///
    /// A deliberately finite diagnostic carrier is valid, but it must enter
    /// through this explicit root constructor. It is therefore never confused
    /// with an execution-only depth or i64-representability envelope.
    pub(crate) fn try_new_root(
        declared_carrier: DecoratedStratum,
    ) -> Result<Self, SpiredCoordinateCaseWorklistError> {
        require_guard_blind(&declared_carrier)?;
        Ok(Self {
            stratum: declared_carrier.clone(),
            declared_carrier: Arc::new(declared_carrier),
        })
    }

    /// Bind an exact coordinate-equality child to an existing declared
    /// carrier. Every axis must be either carrier-full or one contained
    /// singleton; arbitrary proper intervals are rejected.
    pub(crate) fn try_new_child(
        parent: &Self,
        stratum: DecoratedStratum,
    ) -> Result<Self, SpiredCoordinateCaseWorklistError> {
        require_guard_blind(&stratum)?;
        validate_same_scope(parent.declared_carrier(), &stratum)?;
        validate_coordinate_equality_geometry(parent.declared_carrier(), &stratum)?;
        if !domain_contains(parent.stratum(), &stratum) {
            return Err(SpiredCoordinateCaseWorklistError::ChildOutsideParent);
        }
        Ok(Self {
            declared_carrier: Arc::clone(&parent.declared_carrier),
            stratum,
        })
    }

    pub(crate) fn declared_carrier(&self) -> &DecoratedStratum {
        &self.declared_carrier
    }

    pub(crate) fn shares_declared_carrier(&self, other: &Self) -> bool {
        self.declared_carrier == other.declared_carrier
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }

    /// Coordinate dimension of the exact equality case. An axis is free only
    /// when it retains its complete declared-carrier interval.
    pub(crate) fn free_dimension(&self) -> usize {
        self.stratum
            .domain()
            .bounds()
            .iter()
            .zip(self.declared_carrier.domain().bounds())
            .filter(|&(case, carrier)| case == carrier && case.lower() != case.upper())
            .count()
    }
}

fn domain_contains(outer: &DecoratedStratum, inner: &DecoratedStratum) -> bool {
    outer.domain().sector() == inner.domain().sector()
        && outer
            .domain()
            .bounds()
            .iter()
            .zip(inner.domain().bounds())
            .all(|(&outer, &inner)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
}

fn require_guard_blind(
    stratum: &DecoratedStratum,
) -> Result<(), SpiredCoordinateCaseWorklistError> {
    if !stratum.guards().is_empty() {
        return Err(SpiredCoordinateCaseWorklistError::GuardedDiscoveryCase {
            guard_branches: stratum.guards().len(),
        });
    }
    Ok(())
}

fn validate_same_scope(
    carrier: &DecoratedStratum,
    stratum: &DecoratedStratum,
) -> Result<(), SpiredCoordinateCaseWorklistError> {
    if carrier.family_fingerprint() != stratum.family_fingerprint() {
        return Err(SpiredCoordinateCaseWorklistError::CaseCarrierFamilyMismatch);
    }
    if carrier.context_fingerprint() != stratum.context_fingerprint() {
        return Err(SpiredCoordinateCaseWorklistError::CaseCarrierContextMismatch);
    }
    if carrier.domain().sector() != stratum.domain().sector() {
        return Err(SpiredCoordinateCaseWorklistError::CaseCarrierSectorMismatch);
    }
    Ok(())
}

fn validate_coordinate_equality_geometry(
    carrier: &DecoratedStratum,
    stratum: &DecoratedStratum,
) -> Result<(), SpiredCoordinateCaseWorklistError> {
    for (position, (&carrier_bound, &case_bound)) in carrier
        .domain()
        .bounds()
        .iter()
        .zip(stratum.domain().bounds())
        .enumerate()
    {
        if case_bound == carrier_bound {
            continue;
        }
        if case_bound.lower() == case_bound.upper() && carrier_bound.contains(case_bound.lower()) {
            continue;
        }
        return Err(SpiredCoordinateCaseWorklistError::NonEqualityCaseAxis {
            position,
            carrier_lower: carrier_bound.lower(),
            carrier_upper: carrier_bound.upper(),
            case_lower: case_bound.lower(),
            case_upper: case_bound.upper(),
        });
    }
    Ok(())
}

/// Result of adding one exact obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateCaseEnqueueOutcome {
    Inserted { removed_subsumed: usize },
    ExactDuplicate,
    SubsumedByPending,
}

/// Result of requesting the next exact obligation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateCasePopOutcome {
    Case(SpiredCoordinateCaseObligation),
    Empty,
}

/// Exact queue and cumulative-work census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredCoordinateCaseWorklistCensus {
    pub(super) enqueue_attempts: usize,
    pub(super) inserted_cases: usize,
    pub(super) exact_duplicates: usize,
    pub(super) subsumed_incoming: usize,
    pub(super) removed_subsumed: usize,
    pub(super) subsumption_checks: usize,
    pub(super) popped_cases: usize,
    /// Cases atomically retired after a completed driver step.
    ///
    /// Unlike `popped_cases`, retiring a case may enqueue refined children in
    /// the same transaction. Exactly one retirement is charged per committed
    /// replacement, independently of how many children survive subsumption.
    pub(super) retired_cases: usize,
    pub(super) empty_pops: usize,
    pub(super) pending_cases: usize,
    pub(super) pending_coordinate_cells: usize,
    pub(super) pending_identity_bytes: usize,
}

impl SpiredCoordinateCaseWorklistCensus {
    pub(crate) const fn enqueue_attempts(self) -> usize {
        self.enqueue_attempts
    }

    pub(crate) const fn inserted_cases(self) -> usize {
        self.inserted_cases
    }

    pub(crate) const fn exact_duplicates(self) -> usize {
        self.exact_duplicates
    }

    pub(crate) const fn subsumed_incoming(self) -> usize {
        self.subsumed_incoming
    }

    pub(crate) const fn removed_subsumed(self) -> usize {
        self.removed_subsumed
    }

    pub(crate) const fn subsumption_checks(self) -> usize {
        self.subsumption_checks
    }

    pub(crate) const fn popped_cases(self) -> usize {
        self.popped_cases
    }

    pub(crate) const fn retired_cases(self) -> usize {
        self.retired_cases
    }

    pub(crate) const fn empty_pops(self) -> usize {
        self.empty_pops
    }

    pub(crate) const fn pending_cases(self) -> usize {
        self.pending_cases
    }

    pub(crate) const fn pending_coordinate_cells(self) -> usize {
        self.pending_coordinate_cells
    }

    pub(crate) const fn pending_identity_bytes(self) -> usize {
        self.pending_identity_bytes
    }
}
