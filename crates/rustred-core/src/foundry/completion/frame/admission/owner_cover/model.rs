//! Sealed owner inputs, deterministic owners, and typed cover outcomes.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshotId, TargetColumnPartition};
use crate::foundry::completion::{LatticePoint, SectorChart, UncoveredPartition};
use crate::sector::{Mask, OrderingPolicy};

use super::super::semantic::{
    ExactCircuitSemanticCandidate, ExactCircuitSemanticDag, ExactCircuitSemanticSelection,
};
use super::{ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCoverError};

/// One semantic DAG paired with the exact partition against which it was
/// compiled. The cover compiler performs the cold join before retaining it.
#[derive(Debug)]
pub(crate) struct ExactCircuitOwnerInput<'partition, 'frame> {
    pub(super) partition: &'partition TargetColumnPartition<'frame>,
    pub(super) outer_extension: ExactCircuitOuterExtensionWitness<'frame>,
}

impl<'partition, 'frame> ExactCircuitOwnerInput<'partition, 'frame> {
    pub(crate) const fn new(
        partition: &'partition TargetColumnPartition<'frame>,
        outer_extension: ExactCircuitOuterExtensionWitness<'frame>,
    ) -> Self {
        Self {
            partition,
            outer_extension,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExactCircuitOwnerId(pub(super) usize);

impl ExactCircuitOwnerId {
    pub(crate) const fn ordinal(self) -> usize {
        self.0
    }
}

/// One canonically ordered exact-rule region. A guard-total owner covers its
/// entire leading orthant; a partial owner is usable only pointwise.
#[derive(Debug)]
pub(crate) struct ExactCircuitOwner {
    pub(super) id: ExactCircuitOwnerId,
    pub(super) leading: LatticePoint,
    pub(super) semantic: Arc<ExactCircuitSemanticDag>,
    pub(super) guard_total: bool,
}

impl ExactCircuitOwner {
    pub(crate) const fn id(&self) -> ExactCircuitOwnerId {
        self.id
    }

    pub(crate) const fn leading(&self) -> &LatticePoint {
        &self.leading
    }

    pub(crate) const fn is_guard_total(&self) -> bool {
        self.guard_total
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExactFiniteTerminalOwnerId(pub(super) usize);

/// One explicitly declared finite terminal. Merely being in a finite
/// complement never constructs this value.
#[derive(Debug)]
pub(crate) struct ExactFiniteTerminalOwner {
    pub(super) id: ExactFiniteTerminalOwnerId,
    pub(super) integral: IntegralKey,
    pub(super) point: LatticePoint,
}

impl ExactFiniteTerminalOwner {
    pub(crate) const fn integral(&self) -> &IntegralKey {
        &self.integral
    }

    pub(crate) const fn point(&self) -> &LatticePoint {
        &self.point
    }
}

/// Exact descending ownership found by evaluating a partial guard DAG at one
/// enumerated point of an otherwise finite complement.
#[derive(Debug)]
#[allow(dead_code)] // Retained proof view awaits the staged foundry orchestrator.
pub(crate) struct ExactFinitePointOwner {
    pub(super) point: LatticePoint,
    pub(super) owner: ExactCircuitOwnerId,
    pub(super) candidate_ordinal: usize,
    pub(super) circuit: Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit>,
}

#[allow(dead_code)] // Retained proof view awaits the staged foundry orchestrator.
impl ExactFinitePointOwner {
    pub(crate) const fn point(&self) -> &LatticePoint {
        &self.point
    }

    pub(crate) const fn owner(&self) -> ExactCircuitOwnerId {
        self.owner
    }

    pub(crate) const fn candidate_ordinal(&self) -> usize {
        self.candidate_ordinal
    }

    pub(crate) const fn circuit(
        &self,
    ) -> &Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit> {
        &self.circuit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactOwnerCoverObstructionKind {
    /// Even guard-blind leading orthants leave an unbounded geometric residue.
    NonFinite,
    /// Guard-blind geometry is finite, but at least one required unbounded
    /// region reaches a semantic `Incomplete` branch.
    GuardIncomplete,
    /// The exact residue is finite, but one or more points lack an explicit
    /// terminal declaration and any selected exact circuit.
    FiniteTerminalOwnership,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactOwnerCoverStatus {
    Closed,
    Incomplete(ExactOwnerCoverObstructionKind),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ExactOwnerCoverSelection<'a> {
    Descending {
        owner: &'a ExactCircuitOwner,
        candidate: &'a ExactCircuitSemanticCandidate,
    },
    Terminal(&'a ExactFiniteTerminalOwner),
    Incomplete,
}

/// One scope-bound deterministic cover and its exact finite/nonfinite verdict.
#[derive(Debug)]
#[allow(dead_code)] // Scope payload is publication evidence for the next stage.
pub(crate) struct ExactCircuitOwnerCover {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) sector: Mask,
    pub(super) ordering: OrderingPolicy,
    pub(super) owner_snapshot_id: ImmutableOwnerSnapshotId,
    pub(super) owners: Box<[ExactCircuitOwner]>,
    pub(super) terminals: Box<[ExactFiniteTerminalOwner]>,
    pub(super) finite_point_owners: Box<[ExactFinitePointOwner]>,
    pub(super) uncovered: UncoveredPartition,
    pub(super) missing_terminals: Box<[LatticePoint]>,
    pub(super) guard_incomplete_owners: Box<[ExactCircuitOwnerId]>,
    pub(super) status: ExactOwnerCoverStatus,
}

#[allow(dead_code)] // Scope accessors are consumed by the next staged boundary.
impl ExactCircuitOwnerCover {
    pub(crate) const fn status(&self) -> ExactOwnerCoverStatus {
        self.status
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn owner_snapshot_id(&self) -> &ImmutableOwnerSnapshotId {
        &self.owner_snapshot_id
    }

    pub(crate) fn owners(&self) -> &[ExactCircuitOwner] {
        &self.owners
    }

    pub(crate) fn terminals(&self) -> &[ExactFiniteTerminalOwner] {
        &self.terminals
    }

    pub(crate) fn finite_point_owners(&self) -> &[ExactFinitePointOwner] {
        &self.finite_point_owners
    }

    pub(crate) const fn uncovered_partition(&self) -> &UncoveredPartition {
        &self.uncovered
    }

    pub(crate) fn missing_terminals(&self) -> &[LatticePoint] {
        &self.missing_terminals
    }

    pub(crate) fn guard_incomplete_owners(&self) -> &[ExactCircuitOwnerId] {
        &self.guard_incomplete_owners
    }

    /// Select the first exact applicable owner at one target key. Explicit
    /// terminals are consulted only after every overlapping rule declined.
    pub(crate) fn try_select_at(
        &self,
        context: &IndexedCoefficientContext,
        target: &IntegralKey,
        limits: GuardDecisionEvaluationLimits,
    ) -> Result<ExactOwnerCoverSelection<'_>, ExactCircuitOwnerCoverError> {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(ExactCircuitOwnerCoverError::WrongContext);
        }
        let point = SectorChart::new(self.sector.clone()).to_lattice(target)?;
        for owner in &self.owners {
            if !orthant_contains(owner.leading(), &point) {
                continue;
            }
            match owner
                .semantic
                .try_select_at(context, target.powers(), limits)
                .map_err(|error| ExactCircuitOwnerCoverError::SemanticSelection {
                    owner: owner.id.ordinal(),
                    error,
                })? {
                ExactCircuitSemanticSelection::Selected(candidate) => {
                    return Ok(ExactOwnerCoverSelection::Descending { owner, candidate });
                }
                ExactCircuitSemanticSelection::Incomplete => {}
            }
        }
        match self
            .terminals
            .binary_search_by(|terminal| terminal.point.cmp(&point))
        {
            Ok(ordinal) => Ok(ExactOwnerCoverSelection::Terminal(&self.terminals[ordinal])),
            Err(_) => Ok(ExactOwnerCoverSelection::Incomplete),
        }
    }
}

pub(super) fn orthant_contains(origin: &LatticePoint, point: &LatticePoint) -> bool {
    origin.arity() == point.arity()
        && origin
            .coordinates()
            .iter()
            .zip(point.coordinates())
            .all(|(&origin, &point)| origin <= point)
}
