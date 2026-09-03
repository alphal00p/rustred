//! Campaign-owned cumulative accounting for exact-lazy completion.
//!
//! A ledger has one opaque identity and one immutable owner/action/limits
//! binding.  Cursors and every authority derived from them retain that
//! identity, so a caller cannot replace either cumulative budget between
//! subjects, retries, normalization, or future epoch transitions.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;

use super::super::super::limits::{InvolutiveWorkBudget, InvolutiveWorkCensus};
use super::super::super::{OreActionIdentity, OreOrderingAdapter};
use super::{
    ExactLazyError, ExactLazyLimits, ExactLazyOwner, ExactLazySession, ExactLazySupportBudget,
    ExactLazySupportCensus,
};

#[derive(Debug)]
struct CompletionLedgerBinding {
    owner: ExactLazyOwner,
    action: OreActionIdentity,
    limits: ExactLazyLimits,
}

/// Opaque identity of one cumulative exact-lazy completion ledger.
///
/// Cloning this value shares authority; constructing an equal-looking ledger
/// never does.  There is deliberately no scalar, ordinal, or public
/// constructor from which callers could forge the binding.
#[derive(Clone, Debug)]
pub(super) struct ExactLazyCompletionLedgerId(Arc<CompletionLedgerBinding>);

impl ExactLazyCompletionLedgerId {
    pub(super) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn require_environment(
        &self,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        if !self.0.owner.belongs_to(session.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !self.0.action.belongs_to(ordering.identity()) {
            return Err(ExactLazyError::WrongOreAction);
        }
        if self.0.limits != limits {
            return Err(ExactLazyError::WrongLimitsContract);
        }
        Ok(())
    }
}

impl PartialEq for ExactLazyCompletionLedgerId {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for ExactLazyCompletionLedgerId {}

/// The only mutable support and involutive-work ledgers in one completion
/// campaign.
///
/// This object is intentionally non-cloneable.  The opaque ID retained by
/// cursors and normal-form seals names this exact object, while all mutable
/// accounting remains here and survives failed transactions and retries.
#[derive(Debug)]
pub(super) struct ExactLazyCompletionLedger {
    id: ExactLazyCompletionLedgerId,
    support: ExactLazySupportBudget,
    work: InvolutiveWorkBudget,
}

impl ExactLazyCompletionLedger {
    pub(super) fn try_new(
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        let id = ExactLazyCompletionLedgerId(Arc::new(CompletionLedgerBinding {
            owner: session.owner().clone(),
            action: ordering.identity().clone(),
            limits,
        }));
        Ok(Self {
            support: ExactLazySupportBudget::new(session.owner()),
            work: InvolutiveWorkBudget::default(),
            id,
        })
    }

    pub(super) fn id(&self) -> &ExactLazyCompletionLedgerId {
        &self.id
    }

    pub(super) const fn support_census(&self) -> ExactLazySupportCensus {
        self.support.census()
    }

    pub(super) fn work_census(&self) -> InvolutiveWorkCensus {
        self.work.census()
    }

    /// Recheck both opaque identity and immutable environment before an
    /// authority-bearing operation is allowed to observe either budget.
    pub(super) fn require_binding(
        &self,
        expected: &ExactLazyCompletionLedgerId,
        session: &ExactLazySession<'_>,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.require_identity(expected)?;
        self.id
            .require_environment(session, ordering, context, limits)
    }

    pub(super) fn require_identity(
        &self,
        expected: &ExactLazyCompletionLedgerId,
    ) -> Result<(), ExactLazyError> {
        if self.id.belongs_to(expected) {
            Ok(())
        } else {
            Err(ExactLazyError::WrongCompletionLedger)
        }
    }

    /// Narrow internal access after the enclosing operation has performed a
    /// complete environment check with [`Self::require_binding`].
    pub(super) fn try_support_budget(
        &mut self,
        expected: &ExactLazyCompletionLedgerId,
    ) -> Result<&mut ExactLazySupportBudget, ExactLazyError> {
        if self.id.belongs_to(expected) {
            Ok(&mut self.support)
        } else {
            Err(ExactLazyError::WrongCompletionLedger)
        }
    }

    /// Narrow internal access after the enclosing operation has performed a
    /// complete environment check with [`Self::require_binding`].
    pub(super) fn try_work_budget(
        &mut self,
        expected: &ExactLazyCompletionLedgerId,
    ) -> Result<&mut InvolutiveWorkBudget, ExactLazyError> {
        if self.id.belongs_to(expected) {
            Ok(&mut self.work)
        } else {
            Err(ExactLazyError::WrongCompletionLedger)
        }
    }
}
