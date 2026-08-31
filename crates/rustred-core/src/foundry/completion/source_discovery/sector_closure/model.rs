use std::sync::Arc;

use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{Mask, OrderingPolicy};

use super::super::ClosedSectorLayer;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct StagedSectorKey {
    pub(super) sector: Mask,
    pub(super) ordering: OrderingPolicy,
}

/// Exact cold evidence retained with a normal incomplete-cover stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StagedSectorClosureStopEvidence {
    sector: Mask,
    ordering: OrderingPolicy,
    owner_count: usize,
    terminal_count: usize,
    uncovered_box_count: usize,
    missing_terminal_count: usize,
    guard_incomplete_owner_count: usize,
}

impl StagedSectorClosureStopEvidence {
    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn owner_count(&self) -> usize {
        self.owner_count
    }

    pub(crate) const fn terminal_count(&self) -> usize {
        self.terminal_count
    }

    pub(crate) const fn uncovered_box_count(&self) -> usize {
        self.uncovered_box_count
    }

    pub(crate) const fn missing_terminal_count(&self) -> usize {
        self.missing_terminal_count
    }

    pub(crate) const fn guard_incomplete_owner_count(&self) -> usize {
        self.guard_incomplete_owner_count
    }

    pub(super) fn new(
        key: &StagedSectorKey,
        owner_count: usize,
        terminal_count: usize,
        uncovered_box_count: usize,
        missing_terminal_count: usize,
        guard_incomplete_owner_count: usize,
    ) -> Self {
        Self {
            sector: key.sector.clone(),
            ordering: key.ordering,
            owner_count,
            terminal_count,
            uncovered_box_count,
            missing_terminal_count,
            guard_incomplete_owner_count,
        }
    }
}

/// Normal exact reason why a staged sector cannot enter the wave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StagedSectorClosureStop {
    NonFinite(StagedSectorClosureStopEvidence),
    GuardIncomplete(StagedSectorClosureStopEvidence),
    FiniteTerminalOwnership(StagedSectorClosureStopEvidence),
}

impl StagedSectorClosureStop {
    pub(crate) const fn evidence(&self) -> &StagedSectorClosureStopEvidence {
        match self {
            Self::NonFinite(evidence)
            | Self::GuardIncomplete(evidence)
            | Self::FiniteTerminalOwnership(evidence) => evidence,
        }
    }
}

/// Successfully published same-rank layer wave and its immutable successor.
#[derive(Debug)]
pub(crate) struct ClosedSectorClosureWave {
    predecessor: ImmutableOwnerSnapshot,
    successor: ImmutableOwnerSnapshot,
    layers: Box<[Arc<ClosedSectorLayer>]>,
}

impl ClosedSectorClosureWave {
    pub(crate) const fn predecessor(&self) -> &ImmutableOwnerSnapshot {
        &self.predecessor
    }

    pub(crate) const fn successor(&self) -> &ImmutableOwnerSnapshot {
        &self.successor
    }

    pub(crate) fn layers(&self) -> &[Arc<ClosedSectorLayer>] {
        &self.layers
    }

    pub(super) fn new(
        predecessor: ImmutableOwnerSnapshot,
        successor: ImmutableOwnerSnapshot,
        layers: Vec<Arc<ClosedSectorLayer>>,
    ) -> Self {
        Self {
            predecessor,
            successor,
            layers: layers.into_boxed_slice(),
        }
    }
}

/// Transactional wave result. The stopped branch contains no sealed or
/// published layer and therefore cannot mutate the predecessor snapshot.
#[derive(Debug)]
pub(crate) enum StagedSectorClosureOutcome {
    Closed(ClosedSectorClosureWave),
    Stopped(Box<[StagedSectorClosureStop]>),
}
