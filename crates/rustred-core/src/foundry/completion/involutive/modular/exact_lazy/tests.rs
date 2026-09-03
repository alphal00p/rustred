use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::{
    ForwardShift, InvolutiveLimits, OreConsequence, OreOrderingAdapter, OreRow,
};
use super::*;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn completed_ordering(
    completed: &CompletedIbpSourceRows,
    exact: InvolutiveLimits,
) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true]).unwrap(),
        completed,
        exact,
    )
    .unwrap()
}

fn consequence(
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    coefficients: impl IntoIterator<Item = (u64, crate::algebra::IndexedCoefficient)>,
    exact: InvolutiveLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        coefficients.into_iter().map(|(shift, coefficient)| {
            (ForwardShift::try_new([shift], exact).unwrap(), coefficient)
        }),
        context,
        exact,
    )
    .unwrap();
    OreConsequence::try_from_source(0, row, ordering, context, exact).unwrap()
}

#[test]
fn exact_import_retains_physical_support_source_chronology_and_guards() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let n = context.index(0).unwrap();
    let mut exact = consequence(
        &ordering,
        context,
        [(0, n.clone()), (2, context.integer(3))],
        limits.exact,
    );
    let guard_coefficient = context.add(&n, &context.one()).unwrap();
    let guard = context
        .numerator_condition_with_limits(
            &guard_coefficient,
            limits.exact.indexed_algebra.exact_algebra,
        )
        .unwrap();
    exact = exact
        .try_require_nonzero_guard(guard, context, limits.exact)
        .unwrap()
        .0;

    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let imported =
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap();

    assert!(imported.owner().belongs_to(session.owner()));
    assert_eq!(imported.row().terms().len(), 2);
    for term in imported.row().terms() {
        assert!(
            term.nonzero_proof()
                .owns(imported.owner(), term.coefficient())
        );
        assert!(!session.try_is_structural_zero(term.coefficient()).unwrap());
    }
    assert_eq!(
        imported
            .row()
            .try_leading_term(&ordering, imported.owner())
            .unwrap()
            .unwrap()
            .shift()
            .values(),
        &[2]
    );
    assert_eq!(imported.derivation().terms().len(), 1);
    let source = &imported.derivation().terms()[0];
    assert_eq!(source.source_ordinal(), 0);
    assert_eq!(source.left_shift().values(), &[0]);
    assert!(
        source
            .nonzero_proof()
            .owns(imported.owner(), source.left_coefficient())
    );
    assert_eq!(
        session.source_relation(0).unwrap().row_id(),
        completed.source_relation(0).unwrap().row_id()
    );
    assert_eq!(imported.guards().descriptors().len(), 1);
    let guard = &imported.guards().descriptors()[0];
    assert!(
        guard
            .nonzero_proof()
            .owns(imported.owner(), guard.coefficient())
    );
    assert_eq!(imported.census().physical_terms(), 2);
    assert_eq!(imported.census().provenance_terms(), 1);
    assert_eq!(imported.census().guard_descriptors(), 1);
    assert_eq!(session.census().transaction_attempts(), 1);
    assert_eq!(session.census().committed_transactions(), 1);
    assert_eq!(session.census().imported_physical_terms(), 2);
    assert_eq!(session.census().imported_provenance_terms(), 1);
    assert_eq!(session.census().imported_guard_descriptors(), 1);
    assert_eq!(session.committed_floor().2, 1);
}

#[test]
fn session_borrows_the_exact_completed_source_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let foreign_completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let foreign_ordering = completed_ordering(&foreign_completed, limits.exact);
    let context = generator.context();

    assert_eq!(
        ExactLazySession::try_new(&ordering, context, &foreign_completed, limits).unwrap_err(),
        ExactLazyError::WrongSourceModule
    );
    let session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    assert!(std::ptr::eq(session.completed_sources(), &completed));
    assert_eq!(
        session
            .owner()
            .require_completed_source_module(&foreign_ordering, &foreign_completed),
        Err(ExactLazyError::WrongOreAction)
    );
}

#[test]
fn owner_rejects_foreign_action_context_and_limit_contract() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let independently_constructed_ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    assert_eq!(
        session.require_binding(&independently_constructed_ordering, context, limits),
        Err(ExactLazyError::WrongOreAction)
    );

    let base = CoefficientContext::new(["d"]);
    let foreign_context =
        IndexedCoefficientContext::try_new(&base, "elc1-foreign-context", 1).unwrap();
    assert_eq!(
        ExactLazySession::try_new(&ordering, &foreign_context, &completed, limits).unwrap_err(),
        ExactLazyError::WrongIndexedContext
    );
    assert_eq!(
        session.require_binding(&ordering, &foreign_context, limits),
        Err(ExactLazyError::WrongIndexedContext)
    );
    let widened = ExactLazyLimits {
        max_imported_physical_terms: limits.max_imported_physical_terms + 1,
        ..limits
    };
    assert_eq!(
        session.require_binding(&ordering, context, widened),
        Err(ExactLazyError::WrongLimitsContract)
    );
}

#[test]
fn exact_ingress_proof_cannot_be_paired_with_an_unrelated_root() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = session.try_begin_transaction().unwrap();
    let (first_root, _) = ExactIngressNonzero::try_ingress(
        &mut transaction,
        context,
        Arc::new(context.index(0).unwrap()),
    )
    .unwrap();
    let (_, second_proof) = ExactIngressNonzero::try_ingress(
        &mut transaction,
        context,
        Arc::new(
            context
                .add(&context.index(0).unwrap(), &context.one())
                .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        LazyOreTerm::try_new(
            &transaction,
            ForwardShift::try_zero(1, limits.exact).unwrap(),
            first_root,
            ExactNonzeroProof::ExactIngress(second_proof),
        )
        .unwrap_err(),
        ExactLazyError::InvalidProof {
            detail: "an Ore term proof does not authenticate its coefficient root"
        }
    );
}

#[test]
fn restricted_arena_rejects_foreign_roots_contexts_and_actions() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let foreign_ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut first = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut second = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let mut first_transaction = first.try_begin_transaction().unwrap();
    let (first_root, _) = ExactIngressNonzero::try_ingress(
        &mut first_transaction,
        context,
        Arc::new(context.index(0).unwrap()),
    )
    .unwrap();
    first_transaction.try_commit().unwrap();

    let base = CoefficientContext::new(["d"]);
    let foreign_context =
        IndexedCoefficientContext::try_new(&base, "elc1-foreign-leaf", 1).unwrap();
    let mut second_transaction = second.try_begin_transaction().unwrap();
    assert_eq!(
        second_transaction.try_neg(&first_root),
        Err(ExactLazyError::WrongSessionOwner)
    );
    assert_eq!(
        second_transaction.try_exact_leaf(
            &foreign_context,
            Arc::new(foreign_context.index(0).unwrap()),
        ),
        Err(ExactLazyError::WrongIndexedContext)
    );
    let own = second_transaction.one();
    assert_eq!(
        second_transaction.try_translate_by_operator(
            &own,
            &ForwardShift::try_zero(1, limits.exact).unwrap(),
            &foreign_ordering,
        ),
        Err(ExactLazyError::WrongOreAction)
    );
}

#[test]
fn restricted_consequence_constructor_rejoins_owner_and_exact_census() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut first = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut second = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let first_transaction = first.try_begin_transaction().unwrap();
    let row = ClassifiedLazyOreRow::try_from_exact_ingress(&first_transaction, Vec::new()).unwrap();
    let provenance = ImportedSourceDerivation::try_new(&first_transaction, Vec::new()).unwrap();
    let guards = ImportedGuardLineage::try_new(&first_transaction, Vec::new()).unwrap();
    let second_transaction = second.try_begin_transaction().unwrap();
    assert_eq!(
        ExactLazyConsequence::try_new(
            &second_transaction,
            row,
            provenance,
            guards,
            ExactLazyPayloadCensus::default(),
        )
        .unwrap_err(),
        ExactLazyError::WrongSessionOwner
    );

    let row =
        ClassifiedLazyOreRow::try_from_exact_ingress(&second_transaction, Vec::new()).unwrap();
    let provenance = ImportedSourceDerivation::try_new(&second_transaction, Vec::new()).unwrap();
    let guards = ImportedGuardLineage::try_new(&second_transaction, Vec::new()).unwrap();
    assert_eq!(
        ExactLazyConsequence::try_new(
            &second_transaction,
            row,
            provenance,
            guards,
            ExactLazyPayloadCensus::new(1, 0, 0),
        )
        .unwrap_err(),
        ExactLazyError::InvalidSupport {
            detail: "exact-lazy payload census disagrees with imported payload"
        }
    );
}

#[test]
fn classified_support_cannot_be_queried_through_a_foreign_session_owner() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut first = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let second = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let mut transaction = first.try_begin_transaction().unwrap();
    let (coefficient, proof) = ExactIngressNonzero::try_ingress(
        &mut transaction,
        context,
        Arc::new(context.index(0).unwrap()),
    )
    .unwrap();
    let term = LazyOreTerm::try_new(
        &transaction,
        ForwardShift::try_zero(1, limits.exact).unwrap(),
        coefficient,
        ExactNonzeroProof::ExactIngress(proof),
    )
    .unwrap();
    let row = ClassifiedLazyOreRow::try_from_exact_ingress(&transaction, vec![term]).unwrap();
    assert!(matches!(
        row.try_leading_term(&ordering, second.owner()),
        Err(ExactLazyError::WrongSessionOwner)
    ));
}

#[test]
fn rollback_keeps_committed_roots_live_and_escaped_attempt_roots_stale() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();

    let mut first = session.try_begin_transaction().unwrap();
    let (committed, _) =
        ExactIngressNonzero::try_ingress(&mut first, context, Arc::new(context.index(0).unwrap()))
            .unwrap();
    first.try_commit().unwrap();
    let committed_floor = session.committed_floor();
    assert!(!session.try_is_structural_zero(&committed).unwrap());

    let escaped = {
        let mut failed = session.try_begin_transaction().unwrap();
        let (root, _) = ExactIngressNonzero::try_ingress(
            &mut failed,
            context,
            Arc::new(
                context
                    .add(&context.index(0).unwrap(), &context.one())
                    .unwrap(),
            ),
        )
        .unwrap();
        root
    };
    assert_eq!(session.committed_floor(), committed_floor);
    assert!(!session.try_is_structural_zero(&committed).unwrap());
    assert!(matches!(
        session.try_is_structural_zero(&escaped),
        Err(ExactLazyError::Modular(
            super::super::ModularGuideError::StaleDagReference { .. }
        ))
    ));

    // Reuse the same ordinal under a fresh incarnation. The escaped root must
    // stay stale, while the replacement may commit above the old floor.
    let mut replacement = session.try_begin_transaction().unwrap();
    let (replacement_root, _) = ExactIngressNonzero::try_ingress(
        &mut replacement,
        context,
        Arc::new(
            context
                .add(&context.index(0).unwrap(), &context.integer(2))
                .unwrap(),
        ),
    )
    .unwrap();
    replacement.try_commit().unwrap();
    assert!(!session.try_is_structural_zero(&replacement_root).unwrap());
    assert!(session.try_is_structural_zero(&escaped).is_err());
    assert_eq!(session.committed_floor().2, 2);
}

#[test]
fn structural_zero_proofs_are_owned_and_unclassified_rows_hide_support() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let transaction = session.try_begin_transaction().unwrap();
    let zero = transaction.zero();
    let proof = StructuralZeroProof::try_new(&transaction, &zero).unwrap();
    assert!(proof.owns(transaction.owner(), &zero));
    let pending = PendingLazyOreTerm::from_changed(
        ForwardShift::try_zero(1, limits.exact).unwrap(),
        transaction.one(),
    );
    let row = UnclassifiedLazyOreRow::try_new(&transaction, [pending], [proof]).unwrap();
    assert_eq!(row.pending_term_count(), 1);
    assert_eq!(row.structural_zero_elision_count(), 1);
}

#[test]
fn failed_import_rolls_back_live_storage_but_keeps_attempted_churn_charged() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.coefficient.max_exact_leaves = 0;
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let exact = consequence(
        &ordering,
        context,
        [(0, context.index(0).unwrap())],
        limits.exact,
    );
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    let before_live = session.coefficient_live_census();
    let before_cumulative = session.coefficient_cumulative_census();
    assert!(matches!(
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits),
        Err(ExactLazyError::Modular(
            super::super::ModularGuideError::ResourceLimit {
                resource: "modular coefficient exact leaves",
                requested: 1,
                limit: 0,
            }
        ))
    ));
    assert_eq!(session.coefficient_live_census(), before_live);
    assert!(session.coefficient_cumulative_census().3 > before_cumulative.3);
    assert_eq!(session.census().transaction_attempts(), 1);
    assert_eq!(session.census().committed_transactions(), 0);
    assert_eq!(session.census().imported_physical_terms(), 1);
}

#[test]
fn import_and_transaction_limits_fail_at_the_exact_one_below_boundaries() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ExactLazyLimits::default();
    limits.max_imported_physical_terms = 0;
    let ordering = completed_ordering(&completed, limits.exact);
    let context = generator.context();
    let exact = consequence(
        &ordering,
        context,
        [(0, context.index(0).unwrap())],
        limits.exact,
    );
    let mut session = ExactLazySession::try_new(&ordering, context, &completed, limits).unwrap();
    assert_eq!(
        try_import_exact_consequence(&mut session, &exact, &ordering, context, limits).unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy imported physical terms",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.census(), ExactLazyCensus::default());

    let mut commit_limited = ExactLazyLimits::default();
    commit_limited.max_committed_transactions = 0;
    let ordering = completed_ordering(&completed, commit_limited.exact);
    let mut session =
        ExactLazySession::try_new(&ordering, context, &completed, commit_limited).unwrap();
    let floor = session.committed_floor();
    let transaction = session.try_begin_transaction().unwrap();
    assert_eq!(
        transaction.try_commit(),
        Err(ExactLazyError::ResourceLimit {
            resource: "exact-lazy committed transactions",
            requested: 1,
            limit: 0,
        })
    );
    assert_eq!(session.committed_floor(), floor);
    assert_eq!(session.census().transaction_attempts(), 1);
    assert_eq!(session.census().committed_transactions(), 0);

    let transaction_limited = ExactLazyLimits {
        max_transaction_attempts: 0,
        ..ExactLazyLimits::default()
    };
    let ordering = completed_ordering(&completed, transaction_limited.exact);
    let mut session =
        ExactLazySession::try_new(&ordering, context, &completed, transaction_limited).unwrap();
    assert_eq!(
        session.try_begin_transaction().unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy transaction attempts",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(session.census().transaction_attempts(), 0);
}

#[test]
fn provenance_guard_and_cumulative_import_caps_are_independent() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let base_limits = ExactLazyLimits::default();
    let ordering = completed_ordering(&completed, base_limits.exact);
    let context = generator.context();
    let n = context.index(0).unwrap();
    let exact = consequence(&ordering, context, [(0, n.clone())], base_limits.exact);
    let guard_coefficient = context.add(&n, &context.one()).unwrap();
    let guard = context
        .numerator_condition_with_limits(
            &guard_coefficient,
            base_limits.exact.indexed_algebra.exact_algebra,
        )
        .unwrap();
    let exact = exact
        .try_require_nonzero_guard(guard, context, base_limits.exact)
        .unwrap()
        .0;

    let provenance_limited = ExactLazyLimits {
        max_imported_provenance_terms: 0,
        ..base_limits
    };
    let mut session =
        ExactLazySession::try_new(&ordering, context, &completed, provenance_limited).unwrap();
    assert_eq!(
        try_import_exact_consequence(&mut session, &exact, &ordering, context, provenance_limited,)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy imported provenance terms",
            requested: 1,
            limit: 0,
        }
    );

    let guard_limited = ExactLazyLimits {
        max_imported_guard_descriptors: 0,
        ..base_limits
    };
    let mut session =
        ExactLazySession::try_new(&ordering, context, &completed, guard_limited).unwrap();
    assert_eq!(
        try_import_exact_consequence(&mut session, &exact, &ordering, context, guard_limited,)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy imported guard descriptors",
            requested: 1,
            limit: 0,
        }
    );

    let cumulative_limited = ExactLazyLimits {
        max_total_imported_physical_terms: 1,
        max_total_imported_provenance_terms: 1,
        max_total_imported_guard_descriptors: 1,
        ..base_limits
    };
    let mut session =
        ExactLazySession::try_new(&ordering, context, &completed, cumulative_limited).unwrap();
    try_import_exact_consequence(&mut session, &exact, &ordering, context, cumulative_limited)
        .unwrap();
    assert_eq!(
        try_import_exact_consequence(&mut session, &exact, &ordering, context, cumulative_limited,)
            .unwrap_err(),
        ExactLazyError::ResourceLimit {
            resource: "exact-lazy imported physical terms",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(session.census().committed_transactions(), 1);
    assert_eq!(session.census().imported_physical_terms(), 1);
}
