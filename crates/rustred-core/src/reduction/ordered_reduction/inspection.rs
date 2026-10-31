//! Optional caller inspection at the frozen phase boundaries of one ordered request.
//!
//! Inspection never runs inside a checked step or a frontier merge, and it
//! cannot change scheduled weights: it only borrows the scheduler state. At a
//! phase start that state is every pending weighted identity (including those
//! of later phases), the terminal output accumulated so far and the nonzero
//! conditions interned so far. Weights are exposed exactly when the request's
//! weight arithmetic carries exact values. Returning an error stops the request
//! through the adapter's ordinary typed error path; no partial result escapes.
use std::fmt;

use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::solver::{CandidateOrderedPhaseKind, CandidateOrderedReductionProgress};

/// Operational stage of one scheduled identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderedKeyStage<'a> {
    /// Closed and candidate programs schedule plain integrals.
    Integral,
    /// Saved application: transport the integral towards its owner sector.
    Route,
    /// Saved application: apply this owner's saved rules to the integral.
    Apply { owner_sector: &'a [bool] },
}

/// One pending frontier identity and its accumulated weight.
#[derive(Clone, Copy, Debug)]
pub struct OrderedFrontierEntry<'a> {
    /// Frozen phase that will expand this identity.
    pub phase: usize,
    pub stage: OrderedKeyStage<'a>,
    pub integral: &'a IntegralKey,
    /// Exact accumulated weight. `None` under modular weight arithmetic
    /// without the exact cross-check, whose frontier carries finite-field
    /// images rather than exact values.
    pub weight: Option<&'a Coefficient>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderedPhaseBoundary {
    /// Before the first identity of the phase is popped.
    #[default]
    Started,
    /// After the phase has drained, including empty phases.
    Finished,
}

/// Adapter description of one frozen phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OrderedPhaseDescriptor<'a> {
    pub kind: CandidateOrderedPhaseKind,
    /// Distinct positive denominator slots, when phases are support levels.
    pub support: Option<usize>,
    /// Stable zero-based owner index in the request's schedule.
    pub owner_index: Option<usize>,
    pub owner_sector: Option<&'a [bool]>,
}

/// Borrowed scheduler state behind one inspection event.
pub(crate) trait OrderedStateView {
    fn frontier(&self) -> Box<dyn Iterator<Item = OrderedFrontierEntry<'_>> + '_>;
    fn terminals(
        &self,
    ) -> Box<dyn ExactSizeIterator<Item = (&IntegralKey, Option<&Coefficient>)> + '_>;
    fn nonzero_conditions(&self) -> &[Coefficient];
}

/// One phase boundary. Counters are cumulative for the native request.
#[derive(Clone, Copy)]
pub struct OrderedPhaseEvent<'a> {
    pub boundary: OrderedPhaseBoundary,
    pub phase: usize,
    pub descriptor: OrderedPhaseDescriptor<'a>,
    pub progress: CandidateOrderedReductionProgress,
    state: &'a dyn OrderedStateView,
}

impl<'a> OrderedPhaseEvent<'a> {
    pub(crate) fn new(
        boundary: OrderedPhaseBoundary,
        phase: usize,
        progress: CandidateOrderedReductionProgress,
        state: &'a dyn OrderedStateView,
    ) -> Self {
        Self {
            boundary,
            phase,
            descriptor: OrderedPhaseDescriptor::default(),
            progress,
            state,
        }
    }

    pub(crate) fn with_descriptor(self, descriptor: OrderedPhaseDescriptor<'a>) -> Self {
        Self { descriptor, ..self }
    }

    /// Every pending identity in pop order: the current phase first, then the
    /// later phases. Iteration borrows the frontier; nothing is cloned.
    pub fn frontier(&self) -> impl Iterator<Item = OrderedFrontierEntry<'a>> + 'a {
        self.state.frontier()
    }

    /// Terminal output accumulated by the already expanded identities, in key
    /// order. Weights are exact or `None` as in [`OrderedFrontierEntry::weight`].
    pub fn terminals(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'a IntegralKey, Option<&'a Coefficient>)> + 'a {
        self.state.terminals()
    }

    /// Request guards interned so far, in their deterministic output order.
    pub fn nonzero_conditions(&self) -> &'a [Coefficient] {
        self.state.nonzero_conditions()
    }
}

impl fmt::Debug for OrderedPhaseEvent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderedPhaseEvent")
            .field("boundary", &self.boundary)
            .field("phase", &self.phase)
            .field("descriptor", &self.descriptor)
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}

/// Caller-thread phase inspection. An error stops the request with that error.
pub type OrderedPhaseInspector<'a, E> = dyn FnMut(&OrderedPhaseEvent<'_>) -> Result<(), E> + 'a;
