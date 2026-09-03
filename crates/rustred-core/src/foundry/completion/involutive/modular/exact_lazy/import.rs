use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;

use super::super::super::{OreConsequence, OreOrderingAdapter};
use super::error::try_vec;
use super::{
    ClassifiedLazyOreRow, ExactIngressNonzero, ExactLazyConsequence, ExactLazyError,
    ExactLazyLimits, ExactLazyPayloadCensus, ExactLazySession, ExactLazyTransaction,
    ExactNonzeroProof, ImportedGuardLineage, ImportedSourceDerivation, ImportedSourceTerm,
    LazyOreTerm,
};

/// Unforgeable authority that the complete exact consequence, rather than a
/// caller-selected term subset, crossed the ingress loop in this module.
pub(super) struct ExactIngressRowSeal {
    _private: (),
}

/// Fully validated, session-bound plan for one exact consequence import.
///
/// Frozen epochs preflight every plan before opening their single mutation
/// transaction. The opaque owner binding prevents replay in another lazy
/// generation.
pub(super) struct ExactConsequenceImportPlan<'consequence> {
    owner: super::ExactLazyOwner,
    consequence: &'consequence OreConsequence,
    census: ExactLazyPayloadCensus,
}

impl ExactConsequenceImportPlan<'_> {
    pub(super) const fn census(&self) -> ExactLazyPayloadCensus {
        self.census
    }
}

pub(super) fn try_plan_exact_consequence_import<'consequence>(
    session: &ExactLazySession<'_>,
    consequence: &'consequence OreConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> Result<ExactConsequenceImportPlan<'consequence>, ExactLazyError> {
    session.require_binding(ordering, context, limits)?;
    consequence.try_validate(ordering, context, limits.exact)?;
    for source in consequence.provenance().terms() {
        ordering.require_source_ordinal(source.source_ordinal())?;
        session.source_relation(source.source_ordinal())?;
    }
    Ok(ExactConsequenceImportPlan {
        owner: session.owner().clone(),
        consequence,
        census: ExactLazyPayloadCensus::new(
            consequence.row().terms().len(),
            consequence.provenance().terms().len(),
            consequence.required_nonzero_guards().len(),
        ),
    })
}

pub(super) fn try_build_planned_exact_consequence(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    plan: &ExactConsequenceImportPlan<'_>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> Result<ExactLazyConsequence, ExactLazyError> {
    if !plan.owner.belongs_to(transaction.owner()) {
        return Err(ExactLazyError::WrongSessionOwner);
    }
    transaction.owner().require_ordering(ordering)?;
    if transaction.owner().limits() != limits {
        return Err(ExactLazyError::WrongLimitsContract);
    }
    let census = plan.census;
    let (row, derivation, guards) = try_build_imported_parts(
        transaction,
        plan.consequence,
        ordering,
        context,
        census.physical_terms(),
        census.provenance_terms(),
        census.guard_descriptors(),
    )?;
    ExactLazyConsequence::try_new(transaction, row, derivation, guards, census)
}

/// Import one completely authenticated exact consequence into an ELC1
/// coefficient generation.
///
/// Physical coefficients, every source-module coefficient, and every exact
/// localization guard enter the same coefficient transaction. Source
/// ordinals are rechecked explicitly because they are meaningful only under
/// the ordering's sealed completed-source chronology.
pub(super) fn try_import_exact_consequence(
    session: &mut ExactLazySession<'_>,
    consequence: &OreConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: ExactLazyLimits,
) -> Result<ExactLazyConsequence, ExactLazyError> {
    let plan = try_plan_exact_consequence_import(session, consequence, ordering, context, limits)?;
    let census = plan.census();
    let mut transaction = session.try_begin_import_batch_transaction(&[census])?;
    let built =
        try_build_planned_exact_consequence(&mut transaction, &plan, ordering, context, limits);
    match built {
        Ok(imported) => {
            transaction.try_commit()?;
            Ok(imported)
        }
        Err(error) => {
            transaction.try_abort()?;
            Err(error)
        }
    }
}

fn try_build_imported_parts(
    transaction: &mut ExactLazyTransaction<'_, '_>,
    consequence: &OreConsequence,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    physical_count: usize,
    provenance_count: usize,
    guard_count: usize,
) -> Result<
    (
        ClassifiedLazyOreRow,
        ImportedSourceDerivation,
        ImportedGuardLineage,
    ),
    ExactLazyError,
> {
    let mut physical = try_vec("imported exact-lazy physical terms", physical_count)?;
    for term in consequence.row().terms() {
        let coefficient = Arc::new(term.coefficient().clone());
        let (root, proof) = ExactIngressNonzero::try_ingress(transaction, context, coefficient)?;
        physical.push(LazyOreTerm::try_new(
            transaction,
            term.shift().clone(),
            root,
            ExactNonzeroProof::ExactIngress(proof),
        )?);
    }
    let physical = ClassifiedLazyOreRow::try_from_exact_ingress(
        transaction,
        physical,
        ExactIngressRowSeal { _private: () },
    )?;

    let mut provenance = try_vec("imported exact-lazy source-module terms", provenance_count)?;
    for term in consequence.provenance().terms() {
        // Retain exact chronology even when the imported consequence came from
        // an internal trusted path whose ordinary validation did not need to
        // rescan its source-owner range.
        ordering.require_source_ordinal(term.source_ordinal())?;
        if term.left_shift().arity() != transaction.owner().arity() {
            return Err(ExactLazyError::WrongArity {
                object: "imported exact-lazy provenance shift",
                expected: transaction.owner().arity(),
                actual: term.left_shift().arity(),
            });
        }
        let coefficient = Arc::new(term.left_coefficient().clone());
        let (root, proof) = ExactIngressNonzero::try_ingress(transaction, context, coefficient)?;
        provenance.push(ImportedSourceTerm::try_new(
            transaction,
            term.source_ordinal(),
            term.left_shift().clone(),
            root,
            ExactNonzeroProof::ExactIngress(proof),
        )?);
    }

    let mut guards = try_vec("imported exact-lazy guard descriptors", guard_count)?;
    for guard in consequence.required_nonzero_guards() {
        guards.push(transaction.try_polynomial_guard(context, guard)?);
    }

    let provenance = ImportedSourceDerivation::try_new(transaction, provenance)?;
    let guards = ImportedGuardLineage::try_new(transaction, guards)?;
    Ok((physical, provenance, guards))
}
