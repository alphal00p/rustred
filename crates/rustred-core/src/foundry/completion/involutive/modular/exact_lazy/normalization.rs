//! Guarded monic normalization for exact-support lazy consequences.
//!
//! Only the actual greatest live coefficient may be inverted. The authority
//! to replace its product with structural one is opaque and row-bound; every
//! tail coefficient and the complete source derivation are multiplied by the
//! same inverse, while a typed numerator guard records the newly excluded
//! exceptional fibre.

use crate::algebra::IndexedCoefficientContext;

use super::super::super::OreOrderingAdapter;
use super::error::{check_limit, checked_add, try_vec};
use super::{
    ExactLazyCompletionLedger, ExactLazyConsequence, ExactLazyError, ExactLazyFrozenJanetEpoch,
    ExactLazyFullJanetNormalForm, ExactLazyLimits, ExactLazyPayloadCensus, ExactLazyProbeSchedule,
    ExactLazySelfExcludedJanetNormalForm, ExactLazySession, ExactLazySupportBudget,
    ExactLazyTransaction, ImportedGuardLineage, ImportedSourceDerivation, PendingLazyOreTerm,
    UnclassifiedLazyOreRow, try_classify_support,
};

const NORMALIZED_PHYSICAL_TERMS: &str = "exact-lazy monic normalization physical terms";
const NORMALIZED_PROVENANCE_TERMS: &str = "exact-lazy monic normalization provenance terms";
const NORMALIZED_GUARDS: &str = "exact-lazy monic normalization guard descriptors";

/// Opaque output of the sole proof-bound guarded-normalization constructor.
/// Its field is private to this module, so another exact-lazy component cannot
/// replace a finalized normal-form remainder with an unchecked consequence.
#[derive(Debug)]
pub(super) struct AuthenticatedMonicReplacement(ExactLazyConsequence);

impl AuthenticatedMonicReplacement {
    pub(super) fn into_consequence(self) -> ExactLazyConsequence {
        self.0
    }
}

/// Guard-normalize a full normal form while preserving its full-NF authority
/// and exact epoch/ledger binding.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_normalize_full_normal_form_monic(
    session: &mut ExactLazySession<'_>,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    normal_form: &mut ExactLazyFullJanetNormalForm,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    ledger: &mut ExactLazyCompletionLedger,
    limits: ExactLazyLimits,
) -> Result<bool, ExactLazyError> {
    normal_form.require_binding(session, frozen.epoch(), ordering, context, ledger, limits)?;
    let ledger_id = normal_form.ledger_id().clone();
    let support_budget = ledger.try_support_budget(&ledger_id)?;
    let normalized = try_normalize_consequence_monic(
        session,
        normal_form.remainder(),
        ordering,
        context,
        schedule,
        support_budget,
        limits,
    )?;
    if let Some(remainder) = normalized {
        normal_form.replace_with_normalized_remainder(remainder, ledger);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Guard-normalize a self-excluding normal form without erasing its excluded
/// divisor or converting it into unrestricted normal-form authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_normalize_self_excluded_normal_form_monic(
    session: &mut ExactLazySession<'_>,
    frozen: &ExactLazyFrozenJanetEpoch<'_>,
    normal_form: &mut ExactLazySelfExcludedJanetNormalForm,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    ledger: &mut ExactLazyCompletionLedger,
    limits: ExactLazyLimits,
) -> Result<bool, ExactLazyError> {
    normal_form.require_binding(session, frozen.epoch(), ordering, context, ledger, limits)?;
    let ledger_id = normal_form.ledger_id().clone();
    let support_budget = ledger.try_support_budget(&ledger_id)?;
    let normalized = try_normalize_consequence_monic(
        session,
        normal_form.remainder(),
        ordering,
        context,
        schedule,
        support_budget,
        limits,
    )?;
    if let Some(remainder) = normalized {
        normal_form.replace_with_normalized_remainder(remainder, ledger);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Test-only raw normalization seam. Production callers must retain and
/// consume a typed full or self-excluding normal-form authority instead.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn try_normalize_monic_test_local(
    session: &mut ExactLazySession<'_>,
    consequence: &ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    ledger: &mut ExactLazyCompletionLedger,
    limits: ExactLazyLimits,
) -> Result<Option<ExactLazyConsequence>, ExactLazyError> {
    let ledger_id = ledger.id().clone();
    ledger.require_binding(&ledger_id, session, ordering, context, limits)?;
    let support_budget = ledger.try_support_budget(&ledger_id)?;
    try_normalize_consequence_monic(
        session,
        consequence,
        ordering,
        context,
        schedule,
        support_budget,
        limits,
    )
    .map(|replacement| replacement.map(AuthenticatedMonicReplacement::into_consequence))
}

/// Return an exactly supported monic copy, or `None` on the immutable
/// structural-one fast path. A failed classification remains charged even
/// though all coefficient/provenance/guard arena mutations roll back.
#[allow(clippy::too_many_arguments)]
fn try_normalize_consequence_monic(
    session: &mut ExactLazySession<'_>,
    consequence: &ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    support_budget: &mut ExactLazySupportBudget,
    limits: ExactLazyLimits,
) -> Result<Option<AuthenticatedMonicReplacement>, ExactLazyError> {
    session.require_binding(ordering, context, limits)?;
    schedule.require_owner(session.owner())?;
    support_budget.require_owner(session.owner())?;
    consequence.try_validate_live(session)?;
    let leader = consequence
        .row()
        .try_leading_term(session, ordering)?
        .ok_or(ExactLazyError::InvalidSupport {
            detail: "cannot normalize an empty exact-lazy consequence",
        })?;
    if leader.coefficient() == &session.one() {
        return Ok(None);
    }

    let expected_physical = consequence.row().physical_term_count();
    check_limit(
        NORMALIZED_PHYSICAL_TERMS,
        expected_physical,
        limits.exact.max_row_terms,
    )?;
    let expected_provenance = consequence.derivation().source_term_count();
    check_limit(
        NORMALIZED_PROVENANCE_TERMS,
        expected_provenance,
        limits.exact.max_provenance_terms,
    )?;
    let expected_guards = checked_add(
        NORMALIZED_GUARDS,
        consequence.guards().descriptor_count(),
        1,
    )?;
    check_limit(
        NORMALIZED_GUARDS,
        expected_guards,
        limits.exact.max_localization_guards,
    )?;

    let mut transaction = session.try_begin_transaction()?;
    let built = try_build_normalized(
        &mut transaction,
        consequence,
        ordering,
        context,
        schedule,
        support_budget,
        expected_physical,
        expected_provenance,
        expected_guards,
    );
    match built {
        Ok(normalized) => {
            transaction.try_commit()?;
            Ok(Some(AuthenticatedMonicReplacement(normalized)))
        }
        Err(error) => {
            transaction.try_abort()?;
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn try_build_normalized(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    consequence: &ExactLazyConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    schedule: &ExactLazyProbeSchedule,
    support_budget: &mut ExactLazySupportBudget,
    expected_physical: usize,
    expected_provenance: usize,
    expected_guards: usize,
) -> Result<ExactLazyConsequence, ExactLazyError> {
    let inverse = transaction.try_actual_leader_inverse(consequence.row(), ordering)?;
    let source_terms = consequence.row().try_terms_in_transaction(transaction)?;
    let mut pending = try_vec(NORMALIZED_PHYSICAL_TERMS, source_terms.len())?;
    let mut leader_occurrences = 0usize;
    for term in source_terms {
        if term.shift() == inverse.leader_shift() {
            if term.coefficient() != inverse.leader() {
                return Err(ExactLazyError::InvalidProof {
                    detail: "normalization seal and actual leader root disagree",
                });
            }
            leader_occurrences = checked_add(
                "exact-lazy monic normalization leader occurrences",
                leader_occurrences,
                1,
            )?;
            let (one, proof) =
                transaction.try_guarded_structural_one(&inverse, consequence.row(), ordering)?;
            pending.push(PendingLazyOreTerm::from_unchanged(
                term.shift().clone(),
                one,
                proof,
            ));
        } else {
            let coefficient = transaction.try_mul(inverse.inverse(), term.coefficient())?;
            pending.push(PendingLazyOreTerm::from_changed(
                term.shift().clone(),
                coefficient,
            ));
        }
    }
    if leader_occurrences != 1 {
        return Err(ExactLazyError::InvalidSupport {
            detail: "monic normalization did not find one unique sealed leader",
        });
    }
    let unclassified = UnclassifiedLazyOreRow::try_new(transaction, pending, [])?;

    let zero = transaction.zero_derivation();
    let derivation_root = transaction.try_axpy_derivation(
        &zero,
        inverse.inverse(),
        consequence.derivation().root(),
    )?;
    let derivation = ImportedSourceDerivation::try_from_lineage(transaction, derivation_root)?;
    if derivation.source_term_count() != expected_provenance {
        return Err(ExactLazyError::InvalidSupport {
            detail: "monic normalization changed the logical source provenance census",
        });
    }

    let numerator_guard =
        transaction.try_leader_numerator_guard(&inverse, consequence.row(), ordering)?;
    let guard_root = transaction.try_union_guards(consequence.guards().root(), &numerator_guard)?;
    let guards = ImportedGuardLineage::try_from_lineage(transaction, guard_root)?;
    if guards.descriptor_count() != expected_guards {
        return Err(ExactLazyError::InvalidSupport {
            detail: "monic normalization lost a localization-guard occurrence",
        });
    }
    let guard_requirements =
        transaction.try_collect_guard_probe_requirements(guards.root(), ordering)?;
    let row = try_classify_support(
        transaction,
        context,
        &guard_requirements,
        unclassified,
        schedule,
        support_budget,
    )?;
    if row.physical_term_count() != expected_physical {
        return Err(ExactLazyError::InvalidSupport {
            detail: "monic normalization changed exact physical support",
        });
    }
    let normalized_leader = row
        .try_leading_term_in_transaction(transaction, ordering)?
        .ok_or(ExactLazyError::InvalidSupport {
            detail: "monic normalization produced an empty row",
        })?;
    if normalized_leader.shift() != inverse.leader_shift()
        || normalized_leader.coefficient() != &transaction.one()
        || !matches!(
            normalized_leader.nonzero_proof(),
            super::ExactNonzeroProof::GuardedStructuralOne(_)
        )
    {
        return Err(ExactLazyError::InvalidProof {
            detail: "monic normalization did not retain its sealed structural-one leader",
        });
    }

    let census =
        ExactLazyPayloadCensus::new(expected_physical, expected_provenance, expected_guards);
    ExactLazyConsequence::try_new(transaction, row, derivation, guards, census)
}
