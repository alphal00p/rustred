//! Atomic import of one immutable exact Janet division epoch.
//!
//! Every exact row and every epoch-local binding is preflighted before the
//! lazy session is mutated. All lazy divisors then enter one transaction, so
//! a failure at any ordinal rolls the complete attempted prefix back.

use std::collections::HashSet;

use crate::algebra::IndexedCoefficientContext;

use super::super::super::janet::JanetDivisionEpoch;
use super::super::super::{EpochId, OreOrderingAdapter};
use super::error::{check_limit, try_vec};
use super::import::{
    ExactConsequenceImportPlan, try_build_planned_exact_consequence,
    try_plan_exact_consequence_import,
};
use super::{
    ExactLazyConsequence, ExactLazyError, ExactLazyLimits, ExactLazyOwner, ExactLazySession,
};

const FROZEN_DIVISORS: &str = "exact-lazy frozen Janet divisors";

/// One exact Janet division epoch and its atomically imported lazy divisors.
#[derive(Debug)]
pub(super) struct ExactLazyFrozenJanetEpoch<'epoch> {
    division: &'epoch JanetDivisionEpoch,
    epoch: EpochId,
    owner: ExactLazyOwner,
    divisors: Vec<ExactLazyConsequence>,
}

impl<'epoch> ExactLazyFrozenJanetEpoch<'epoch> {
    pub(super) fn try_import(
        session: &mut ExactLazySession<'_>,
        division: &'epoch JanetDivisionEpoch,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        session.require_binding(ordering, context, limits)?;
        division.require_ordering(ordering)?;
        if division.arity() != ordering.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "frozen Janet division epoch",
                expected: ordering.arity(),
                actual: division.arity(),
            });
        }
        check_limit(
            FROZEN_DIVISORS,
            division.elements().len(),
            limits.max_frozen_epoch_divisors,
        )?;

        // Allocate every fallible retained container and authenticate every
        // exact row before charging or opening the sole arena transaction.
        let mut plans = try_vec("frozen Janet import plans", division.elements().len())?;
        let mut censuses = try_vec(
            "frozen Janet import payload censuses",
            division.elements().len(),
        )?;
        let mut leaders = HashSet::new();
        leaders
            .try_reserve(division.elements().len())
            .map_err(|_| ExactLazyError::AllocationFailure {
                resource: "frozen Janet unique leading shifts",
                requested: division.elements().len(),
            })?;
        let exact_one = context.one();
        for (position, element) in division.elements().iter().enumerate() {
            if element.ordinal() != position {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "frozen Janet ordinal does not equal its canonical position",
                });
            }
            let consequence = element.consequence();
            let plan =
                try_plan_exact_consequence_import(session, consequence, ordering, context, limits)?;
            let Some((leader, _)) = consequence.row().try_leading_term(ordering)? else {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "frozen Janet divisor is the zero row",
                });
            };
            if leader.shift() != element.leading_shift() {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "frozen Janet element and exact row disagree on their leader",
                });
            }
            if leader.coefficient() != &exact_one {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "frozen Janet divisor is not exactly monic",
                });
            }
            if !leaders.insert(element.leading_shift().clone()) {
                return Err(ExactLazyError::InvalidSupport {
                    detail: "frozen Janet epoch has duplicate leading shifts",
                });
            }
            censuses.push(plan.census());
            plans.push(plan);
        }

        let mut divisors = try_vec("frozen Janet lazy divisors", plans.len())?;
        let epoch = division.epoch().clone();
        let owner = session.owner().clone();
        let mut transaction = session.try_begin_import_batch_transaction(&censuses)?;
        let built = try_build_all_divisors(
            &mut transaction,
            &plans,
            division,
            ordering,
            context,
            limits,
            &mut divisors,
        );
        match built {
            Ok(()) => {
                transaction.try_commit()?;
                Ok(Self {
                    division,
                    epoch,
                    owner,
                    divisors,
                })
            }
            Err(error) => {
                transaction.try_abort()?;
                Err(error)
            }
        }
    }

    pub(super) fn division(&self) -> &'epoch JanetDivisionEpoch {
        self.division
    }

    pub(super) fn epoch(&self) -> &EpochId {
        &self.epoch
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) const fn len(&self) -> usize {
        self.divisors.len()
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.divisors.is_empty()
    }

    pub(super) fn divisor(&self, ordinal: usize) -> Result<&ExactLazyConsequence, ExactLazyError> {
        self.divisors
            .get(ordinal)
            .ok_or(ExactLazyError::FrozenDivisorOutOfRange {
                ordinal,
                divisor_count: self.divisors.len(),
            })
    }

    pub(super) fn require_owner(&self, owner: &ExactLazyOwner) -> Result<(), ExactLazyError> {
        if self.owner.belongs_to(owner) {
            Ok(())
        } else {
            Err(ExactLazyError::WrongSessionOwner)
        }
    }

    /// Consume the exact-ingress authority and release its already committed
    /// lazy rows to the separately typed initial persistent-epoch path.
    pub(super) fn into_committed_divisors(self) -> Vec<ExactLazyConsequence> {
        self.divisors
    }
}

#[allow(clippy::too_many_arguments)]
fn try_build_all_divisors(
    transaction: &mut super::ExactLazyTransaction<'_, '_>,
    plans: &[ExactConsequenceImportPlan<'_>],
    division: &JanetDivisionEpoch,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
    divisors: &mut Vec<ExactLazyConsequence>,
) -> Result<(), ExactLazyError> {
    for (ordinal, plan) in plans.iter().enumerate() {
        let imported =
            try_build_planned_exact_consequence(transaction, plan, ordering, context, limits)?;
        let Some(lazy_leader) = imported
            .row()
            .try_leading_term_in_transaction(transaction, ordering)?
        else {
            return Err(ExactLazyError::InvalidSupport {
                detail: "imported frozen Janet divisor lost its support",
            });
        };
        let element = &division.elements()[ordinal];
        if lazy_leader.shift() != element.leading_shift() {
            return Err(ExactLazyError::InvalidSupport {
                detail: "imported frozen Janet divisor changed its leading shift",
            });
        }
        if lazy_leader.coefficient() != &transaction.one() {
            return Err(ExactLazyError::InvalidSupport {
                detail: "imported frozen Janet leader is not the session structural one",
            });
        }
        divisors.push(imported);
    }
    Ok(())
}
