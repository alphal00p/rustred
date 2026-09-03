use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;

use super::super::super::{OreConsequence, OreOrderingAdapter};
use super::error::try_vec;
use super::{
    ClassifiedLazyOreRow, ExactGuardDescriptor, ExactIngressNonzero, ExactLazyConsequence,
    ExactLazyError, ExactLazyLimits, ExactLazyPayloadCensus, ExactLazySession,
    ExactLazyTransaction, ExactNonzeroProof, ImportedGuardLineage, ImportedSourceDerivation,
    ImportedSourceTerm, LazyOreTerm,
};

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
    session.require_binding(ordering, context, limits)?;
    consequence.try_validate(ordering, context, limits.exact)?;
    for source in consequence.provenance().terms() {
        ordering.require_source_ordinal(source.source_ordinal())?;
        session.source_relation(source.source_ordinal())?;
    }

    let physical_terms = consequence.row().terms().len();
    let provenance_terms = consequence.provenance().terms().len();
    let guard_descriptors = consequence.required_nonzero_guards().len();
    session.try_charge_import_attempt(physical_terms, provenance_terms, guard_descriptors)?;

    let mut transaction = session.try_begin_transaction()?;
    let built = try_build_imported_parts(
        &mut transaction,
        consequence,
        ordering,
        context,
        physical_terms,
        provenance_terms,
        guard_descriptors,
    );
    match built {
        Ok((row, derivation, guards)) => {
            let imported = ExactLazyConsequence::try_new(
                &transaction,
                row,
                derivation,
                guards,
                ExactLazyPayloadCensus::new(physical_terms, provenance_terms, guard_descriptors),
            )?;
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
    let physical = ClassifiedLazyOreRow::try_from_exact_ingress(transaction, physical)?;

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
        // Use the indexed context's existing sealed polynomial-to-rational
        // conversion. ELC1 must not implement another CAS representation.
        let coefficient = context.coefficient_from_polynomial_sealed(guard)?;
        let (root, proof) =
            ExactIngressNonzero::try_ingress(transaction, context, Arc::new(coefficient))?;
        guards.push(ExactGuardDescriptor::try_polynomial(
            transaction,
            root,
            ExactNonzeroProof::ExactIngress(proof),
        )?);
    }

    Ok((
        physical,
        ImportedSourceDerivation::try_new(transaction, provenance)?,
        ImportedGuardLineage::try_new(transaction, guards)?,
    ))
}
