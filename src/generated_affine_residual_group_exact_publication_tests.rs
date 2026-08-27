//! Focused acceptance tests for compact application-event preparation and commit.

use std::mem::size_of;
use std::sync::Arc;

use crate::generated_affine_residual_group_exact_physical_row::GeneratedAffineResidualGroupExactPhysicalRow;
use crate::generated_affine_residual_group_exact_publication::{
    PreparedPublication, PublicationError, PublicationLeafDisposition, PublicationLimits,
    publication_route_tag_bytes_for_test,
};
use crate::generated_affine_residual_group_exact_session::{
    CommittedPublicationDomainView, CommittedPublicationEventView, CommittedPublicationLeafView,
    ExceptionalResidualKind, GeneratedAffineResidualGroupExactSession,
    GeneratedAffineResidualGroupExactSessionError, GeneratedAffineResidualGroupExactSessionLimits,
    tests::{
        ExactConditionPlanTestFixture,
        exact_condition_plan_test_fixture_in_sector_with_session_limits,
    },
};
use crate::generated_affine_residual_group_exact_targets::GeneratedAffineResidualGroupExactTargetError;
use crate::generated_affine_residual_group_exact_when_bad_conditions::{
    GeneratedAffineResidualGroupExactConditionPlanCompiler,
    GeneratedAffineResidualGroupExactConditionPlanLimits,
};
use crate::generated_affine_residual_group_exact_when_bad_materialization::{
    GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler,
    GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
};
use crate::generated_affine_residual_group_exact_when_bad_partition::{
    GeneratedAffineResidualGroupExactWhenBadClauseSource,
    GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
    GeneratedAffineResidualGroupExactWhenBadPartitionCompiler,
    GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
};
use crate::native_sparse_scaling::NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA;
use crate::{IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext};

pub(crate) fn ready_for_publication(
    name: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedAffineResidualGroupExactSession,
    Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
) {
    ready_for_publication_with_session_limits(
        name,
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    )
}

fn ready_for_publication_with_session_limits(
    name: &str,
    session_limits: GeneratedAffineResidualGroupExactSessionLimits,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedAffineResidualGroupExactSession,
    Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
) {
    ready_for_publication_in_sector_with_session_limits(name, "011", session_limits)
}

fn ready_for_publication_in_sector_with_session_limits(
    name: &str,
    sector_bits: &str,
    session_limits: GeneratedAffineResidualGroupExactSessionLimits,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedAffineResidualGroupExactSession,
    Arc<GeneratedAffineResidualGroupExactPhysicalRow>,
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
) {
    let ExactConditionPlanTestFixture {
        family,
        context,
        session,
        source,
        ready,
    } = exact_condition_plan_test_fixture_in_sector_with_session_limits(
        name,
        sector_bits,
        false,
        session_limits,
    );
    let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
        &family,
        &context,
        &session,
        ready,
        GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
    )
    .unwrap();
    let materialized = GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
        &family,
        &context,
        &session,
        plan,
        GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
    )
    .unwrap();
    let partitioned = GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
        &family,
        &context,
        &session,
        materialized,
        GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
    )
    .unwrap();
    let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(ready) =
        partitioned
    else {
        panic!("current-lineage fixture unexpectedly became identically bad");
    };
    (family, context, session, source, ready)
}

fn assert_event_bound_domain<'event>(
    event: CommittedPublicationEventView<'event>,
    leaf_ordinal: usize,
    domain: CommittedPublicationDomainView<'event>,
) {
    assert_eq!(domain.event().event_ordinal(), event.event_ordinal());
    assert_eq!(
        domain.target_premises().as_ptr(),
        event.target_premises().as_ptr()
    );
    assert_eq!(
        domain.target_premises().len(),
        event.target_premises().len()
    );
    assert!(std::ptr::eq(
        domain.relative_case(),
        &event.cases_for_test()[leaf_ordinal]
    ));
    assert_eq!(
        domain.predicate_count(),
        domain.relative_case().predicates().len()
    );
    assert_eq!(domain.predicates().len(), domain.predicate_count());
    for (ordinal, raw) in domain.relative_case().predicates().iter().enumerate() {
        let resolved = domain
            .predicate(ordinal)
            .expect("every committed predicate must resolve through its own event");
        assert_eq!(resolved.locus_ordinal(), raw.locus_ordinal());
        assert_eq!(resolved.kind(), raw.kind());
        assert!(std::ptr::eq(
            resolved.polynomial(),
            &event.loci_for_test()[raw.locus_ordinal()]
        ));
    }
    assert!(domain.predicate(domain.predicate_count()).is_none());
}

#[test]
fn larger_recentered_row_uses_the_same_compact_publication_and_exact_census() {
    let (_, _, mut session, source, ready) = ready_for_publication_in_sector_with_session_limits(
        "publication-large-row",
        "111",
        GeneratedAffineResidualGroupExactSessionLimits::default(),
    );
    let source_weak = Arc::downgrade(&source);
    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
    let expected_terms = prepared.ready().terms().len();
    assert!(expected_terms >= 3);
    let expected_publication_bytes = session.publication_retained_bytes_for_test(&prepared);
    let before_native = session.native_sparse_scaling_stats();
    session.commit_publication(prepared).unwrap();
    let after_native = session.native_sparse_scaling_stats();
    assert_eq!(
        after_native.committed_stage_count(),
        before_native.committed_stage_count() + 1,
        "publication must commit exactly the native stage retained by its Ready token"
    );
    assert_eq!(
        after_native.committed_stage_count(),
        session.event_stats().events(),
        "every committed exact-session event must have one validated native stage"
    );
    assert!(!after_native.cumulative_saturated());
    let last_native = after_native.last();
    assert!(last_native.rows() > 0);
    assert!(last_native.physical_columns() > 0);
    assert!(last_native.input_entries() > 0);
    assert_eq!(
        last_native.observed_native_output_entries(),
        last_native
            .native_u_entries()
            .checked_add(last_native.native_l_entries())
            .unwrap()
    );
    assert!(
        last_native.prospective_native_output_entries()
            >= last_native.observed_native_output_entries()
    );
    let first_toml = toml::to_string_pretty(&after_native).unwrap();
    let second_toml = toml::to_string_pretty(&session.native_sparse_scaling_stats()).unwrap();
    assert_eq!(first_toml.as_bytes(), second_toml.as_bytes());
    let document: toml::Value = toml::from_str(&first_toml).unwrap();
    assert_eq!(
        document["schema"].as_str(),
        Some(NATIVE_SYMBOLICA_SPARSE_SCALING_V1_SCHEMA)
    );
    assert_eq!(document["scope"].as_str(), Some(after_native.scope()));
    assert_eq!(
        document["counter_encoding"].as_str(),
        Some(after_native.counter_encoding())
    );
    let returned_trace_entries = last_native.returned_trace_entries().to_string();
    let coefficient_algebra_work = last_native.coefficient_algebra_work().to_string();
    let coefficient_exponent_entry_work = last_native.coefficient_exponent_entry_work().to_string();
    let coefficient_integer_bit_work = last_native.coefficient_integer_bit_work().to_string();
    assert_eq!(
        document["last"]["returned_trace_entries"].as_str(),
        Some(returned_trace_entries.as_str())
    );
    assert_eq!(
        document["last"]["coefficient_algebra_work"].as_str(),
        Some(coefficient_algebra_work.as_str())
    );
    assert_eq!(
        document["last"]["coefficient_exponent_entry_work"].as_str(),
        Some(coefficient_exponent_entry_work.as_str())
    );
    assert_eq!(
        document["last"]["coefficient_integer_bit_work"].as_str(),
        Some(coefficient_integer_bit_work.as_str())
    );
    assert!(document.get("wall_time").is_none());
    assert!(document.get("rss").is_none());
    assert_eq!(
        session.last_publication_term_count_for_test(),
        Some(expected_terms)
    );
    assert_eq!(
        session.event_stats().publication_retained_bytes(),
        expected_publication_bytes
    );
    assert_eq!(
        session.authenticate_event_ledger_census_for_test().unwrap(),
        session.event_stats().ledger_retained_bytes()
    );
    drop(source);
    assert!(
        source_weak.upgrade().is_none(),
        "compact publication must not retain the derivation source"
    );
}

#[test]
fn committed_event_classifies_zero_copy_rule_and_residual_views_exclusively() {
    let (family, context, mut session, source, ready) =
        ready_for_publication("publication-borrowed-routing");
    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
    let expected_stats = prepared.stats();
    let expected_source_ordinal = prepared.ready().source_ordinal();
    let expected_pivot_ordinal = prepared.ready().pivot_ordinal();
    let expected_pivot_term_ordinal = prepared.pivot_term_ordinal();
    let expected_target_locator = *prepared.ready().target_locator();
    let expected_target_premises = prepared.ready().target_premises().as_ptr();
    let expected_target_premise_count = prepared.ready().target_premises().len();
    let expected_database_epoch = session.database_epoch();
    let expected_group_ordinal = session.group_ordinal();
    let (
        expected_ambient_arity,
        expected_free_positions,
        expected_free_position_count,
        expected_compact_matrix,
        expected_compact_matrix_entries,
        expected_target_offset,
        expected_target_offset_entries,
    ) = {
        let geometry = session
            .authenticated_ready_geometry(&family, &context, prepared.ready())
            .unwrap();
        (
            geometry.ambient_arity(),
            geometry.free_positions().as_ptr(),
            geometry.free_positions().len(),
            geometry.compact_affine_matrix().as_ptr(),
            geometry.compact_affine_matrix().len(),
            geometry.target_offset().as_ptr(),
            geometry.target_offset().len(),
        )
    };
    let expected_terms = prepared.ready().terms().as_ptr();
    let expected_loci = prepared.payload().loci().as_ptr();
    let expected_cases = prepared.payload().cases().as_ptr();

    let receipt = session.commit_publication(prepared).unwrap();
    assert_eq!(receipt.source_ordinal(), expected_source_ordinal);
    assert_eq!(receipt.pivot_ordinal(), expected_pivot_ordinal);
    assert_eq!(receipt.stats(), expected_stats);
    assert_eq!(receipt.event().event_ordinal(), receipt.event_ordinal());
    let receipt_event_ordinal = receipt.event_ordinal();
    let ledger_arc_copies = session.event_stats().ledger_arc_copies();

    for ordinal in 0..receipt_event_ordinal {
        assert!(session.committed_publication_event(ordinal).is_none());
    }
    let event = session
        .committed_publication_event(receipt_event_ordinal)
        .expect("the committed receipt must address its compact event");
    assert_eq!(event.event_ordinal(), receipt_event_ordinal);
    assert_eq!(event.source_ordinal(), expected_source_ordinal);
    assert_eq!(event.pivot_ordinal(), expected_pivot_ordinal);
    assert_eq!(event.family_fingerprint(), family.fingerprint_ref());
    assert_eq!(event.context_fingerprint(), context.fingerprint());
    assert_eq!(event.sector().to_bit_string(), "011");
    assert_eq!(event.ordering(), IntegralOrderingPolicy::RustRedUnshiftedV1);
    assert_eq!(event.database_epoch(), expected_database_epoch);
    assert_eq!(event.group_ordinal(), expected_group_ordinal);
    assert_eq!(event.pivot_term_ordinal(), expected_pivot_term_ordinal);
    assert_eq!(event.target_locator(), expected_target_locator);
    assert_eq!(event.target_offset().as_ptr(), expected_target_offset);
    assert_eq!(event.target_offset().len(), expected_target_offset_entries);
    assert_eq!(event.target_premises().as_ptr(), expected_target_premises);
    assert_eq!(event.target_premises().len(), expected_target_premise_count);
    assert_eq!(event.ambient_arity(), expected_ambient_arity);
    assert_eq!(event.free_positions().as_ptr(), expected_free_positions);
    assert_eq!(event.free_positions().len(), expected_free_position_count);
    assert_eq!(
        event.compact_affine_matrix().as_ptr(),
        expected_compact_matrix
    );
    assert_eq!(
        event.compact_affine_matrix().len(),
        expected_compact_matrix_entries
    );
    assert_eq!(event.terms().as_ptr(), expected_terms);
    assert_eq!(event.loci_for_test().as_ptr(), expected_loci);
    assert_eq!(event.cases_for_test().as_ptr(), expected_cases);
    assert_eq!(event.leaf_count(), expected_stats.leaves());
    assert_eq!(event.leaves().len(), expected_stats.leaves());
    assert!(event.leaf(event.leaf_count()).is_none());
    assert_eq!(
        session.committed_publication_events().count(),
        1,
        "the production iterator must omit non-publication ledger events"
    );
    assert_eq!(session.committed_publication_event_handles().count(), 1);
    let retained_from_session = session
        .committed_publication_event_handle(receipt_event_ordinal)
        .expect("a committed publication must support a shallow owning handle");
    assert_eq!(
        retained_from_session.view().terms().as_ptr(),
        expected_terms
    );

    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    let mut classified_leaf_ordinals = Vec::with_capacity(event.leaf_count());
    let mut applicable_leaf_ordinals = Vec::with_capacity(expected_stats.applicable());
    let mut exceptional_leaf_ordinals = Vec::with_capacity(expected_stats.exceptional());
    for leaf in event.leaves() {
        match leaf {
            CommittedPublicationLeafView::Applicable(rule) => {
                applicable += 1;
                classified_leaf_ordinals.push(rule.leaf_ordinal());
                applicable_leaf_ordinals.push(rule.leaf_ordinal());
                assert_eq!(rule.event().event_ordinal(), event.event_ordinal());
                assert_eq!(rule.event().terms().as_ptr(), expected_terms);
                assert_eq!(rule.event().loci_for_test().as_ptr(), expected_loci);
                assert_event_bound_domain(event, rule.leaf_ordinal(), rule.domain());
            }
            CommittedPublicationLeafView::Exceptional(residual) => {
                classified_leaf_ordinals.push(residual.leaf_ordinal());
                exceptional_leaf_ordinals.push(residual.leaf_ordinal());
                assert_eq!(residual.event().event_ordinal(), event.event_ordinal());
                assert_eq!(residual.event().terms().as_ptr(), expected_terms);
                assert_eq!(residual.event().loci_for_test().as_ptr(), expected_loci);
                assert_event_bound_domain(event, residual.leaf_ordinal(), residual.domain());
                match residual.kind() {
                    ExceptionalResidualKind::Domain => exceptional_domain += 1,
                    ExceptionalResidualKind::SectorLeak => exceptional_leak += 1,
                }
            }
        }
    }
    assert_eq!(
        classified_leaf_ordinals,
        (0..event.leaf_count()).collect::<Vec<_>>(),
        "every leaf must appear exactly once in deterministic partition order"
    );
    assert_eq!(applicable, expected_stats.applicable());
    assert_eq!(exceptional_domain, expected_stats.exceptional_domain());
    assert_eq!(exceptional_leak, expected_stats.exceptional_leak());
    assert_eq!(event.applicable_rules().count(), applicable);
    assert_eq!(
        event
            .applicable_rules()
            .map(|rule| rule.leaf_ordinal())
            .collect::<Vec<_>>(),
        applicable_leaf_ordinals
    );
    assert_eq!(
        event.exceptional_residuals().count(),
        exceptional_domain + exceptional_leak
    );
    assert_eq!(
        event
            .exceptional_residuals()
            .map(|residual| residual.leaf_ordinal())
            .collect::<Vec<_>>(),
        exceptional_leaf_ordinals
    );
    assert_eq!(session.event_stats().ledger_arc_copies(), ledger_arc_copies);

    let publication_state_version = session.state_version();
    let transaction = session
        .stage_replayed_row(&family, &context, &source)
        .unwrap();
    let classified = session
        .classify_dependent(transaction)
        .expect("the published source must reduce through its committed pivot");
    session
        .commit_dependent(&family, &context, classified)
        .unwrap();
    assert_eq!(session.state_version(), publication_state_version + 1);
    assert_eq!(
        retained_from_session.view().terms().as_ptr(),
        expected_terms,
        "an owning event handle must remain stable across later session mutation"
    );

    drop(retained_from_session);
    let retained_from_receipt = receipt.into_event_handle();
    drop(session);
    let retained_event = retained_from_receipt.view();
    assert_eq!(retained_event.event_ordinal(), receipt_event_ordinal);
    assert_eq!(
        retained_event.pivot_term_ordinal(),
        expected_pivot_term_ordinal
    );
    assert_eq!(
        retained_event.target_premises().as_ptr(),
        expected_target_premises
    );
    assert_eq!(
        retained_event.target_offset().as_ptr(),
        expected_target_offset
    );
    assert_eq!(
        retained_event.free_positions().as_ptr(),
        expected_free_positions
    );
    assert_eq!(
        retained_event.compact_affine_matrix().as_ptr(),
        expected_compact_matrix
    );
    assert_eq!(retained_event.terms().as_ptr(), expected_terms);
    assert_eq!(retained_event.loci_for_test().as_ptr(), expected_loci);
    assert_eq!(retained_event.cases_for_test().as_ptr(), expected_cases);
}

#[test]
fn prepared_publication_owns_ready_and_one_correct_route_per_leaf() {
    let (_, _, _, _, ready) = ready_for_publication("publication-prepared-natural");
    let ready_retained_bytes = ready.stats().retained_owned_logical_bytes();
    let expected_loci = ready.partition().structural_loci().to_vec();
    let expected_cases: Vec<_> = ready
        .partition()
        .cases()
        .iter()
        .map(|case| (case.id(), case.predicates().to_vec()))
        .collect();
    let expected_routes: Vec<_> = ready
        .partition()
        .classifications()
        .iter()
        .map(|classification| {
            let disposition = match classification.decisive_clause_ordinal() {
                None => PublicationLeafDisposition::Applicable,
                Some(clause) => match ready.clause_provenance()[clause].source() {
                    GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard {
                        ..
                    }
                    | GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity {
                        ..
                    } => PublicationLeafDisposition::ExceptionalDomain,
                    GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary {
                        ..
                    } => PublicationLeafDisposition::ExceptionalLeak,
                },
            };
            (
                classification.case(),
                classification.decisive_clause_ordinal(),
                disposition,
            )
        })
        .collect();

    let mut mapped_recentered_guard = false;
    let mut mapped_denominator_identity = false;
    let mut mapped_retained_boundary = false;
    for provenance in ready.clause_provenance() {
        match provenance.source() {
            GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard { .. } => {
                mapped_recentered_guard = true;
            }
            GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity {
                ..
            } => {
                mapped_denominator_identity = true;
            }
            GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary { .. } => {
                mapped_retained_boundary = true;
            }
        }
    }
    assert!(mapped_recentered_guard);
    assert!(mapped_denominator_identity);
    assert!(mapped_retained_boundary);

    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();

    assert_eq!(prepared.payload().loci(), expected_loci.as_slice());
    assert_eq!(prepared.payload().cases().len(), expected_cases.len());
    for (case, (expected_id, expected_predicates)) in
        prepared.payload().cases().iter().zip(&expected_cases)
    {
        assert_eq!(case.id(), *expected_id);
        assert_eq!(case.predicates(), expected_predicates.as_slice());
    }
    assert_eq!(prepared.leaves().len(), prepared.payload().cases().len());
    assert_eq!(prepared.stats().leaves(), prepared.leaves().len());
    assert!(prepared.stats().applicable() > 0);
    assert!(prepared.stats().exceptional_domain() > 0);
    assert!(prepared.stats().exceptional_leak() > 0);

    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    let mut later_decisive_clause = false;
    for ((ordinal, leaf), (classification_case, decisive_clause, expected_disposition)) in
        prepared.leaves().enumerate().zip(&expected_routes)
    {
        assert_eq!(leaf.ordinal(), ordinal);
        assert_eq!(leaf.case().id(), *classification_case);
        let disposition = leaf.disposition();
        assert_eq!(disposition, *expected_disposition);
        match *decisive_clause {
            None => {
                applicable += 1;
                assert_eq!(disposition, PublicationLeafDisposition::Applicable);
            }
            Some(clause) => {
                later_decisive_clause |= clause > 0;
                match disposition {
                    PublicationLeafDisposition::ExceptionalDomain => {
                        exceptional_domain += 1;
                    }
                    PublicationLeafDisposition::ExceptionalLeak => {
                        exceptional_leak += 1;
                    }
                    PublicationLeafDisposition::Applicable => {
                        panic!("exceptional classification received an applicable route")
                    }
                }
            }
        }
    }

    assert_eq!(applicable, prepared.stats().applicable());
    assert_eq!(exceptional_domain, prepared.stats().exceptional_domain());
    assert_eq!(exceptional_leak, prepared.stats().exceptional_leak());
    assert!(later_decisive_clause);

    let route_tag_bytes = publication_route_tag_bytes_for_test();
    assert_eq!(route_tag_bytes, size_of::<u8>());
    let payload_bytes = prepared.leaves().len() * route_tag_bytes;
    let header_delta = size_of::<PreparedPublication>().saturating_sub(size_of::<
        GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
    >());
    let additional = header_delta + payload_bytes;
    let combined_peak = (ready_retained_bytes + additional)
        .max(ready_retained_bytes + size_of::<Vec<u8>>() + payload_bytes);
    assert_eq!(prepared.stats().additional_retained_bytes(), additional);
    assert_eq!(
        prepared.stats().combined_preparation_peak_bytes(),
        combined_peak
    );
    assert_eq!(
        prepared.stats().applicable() + prepared.stats().exceptional(),
        prepared.stats().leaves()
    );
}

#[test]
fn exact_limits_pass_and_each_one_below_returns_the_original_ready_owner() {
    const NAME: &str = "publication-prepared-limits";
    let (_, _, _, _, baseline_ready) = ready_for_publication(NAME);
    let baseline =
        PreparedPublication::prepare(baseline_ready, PublicationLimits::default()).unwrap();
    let exact = PublicationLimits {
        max_leaves: baseline.stats().leaves(),
        max_additional_retained_bytes: baseline.stats().additional_retained_bytes(),
        max_combined_preparation_peak_bytes: baseline.stats().combined_preparation_peak_bytes(),
    };

    let (_, _, _, _, exact_ready) = ready_for_publication(NAME);
    let exact_prepared = PreparedPublication::prepare(exact_ready, exact).unwrap();
    assert_eq!(exact_prepared.stats(), baseline.stats());

    let one_below = [
        PublicationLimits {
            max_leaves: exact.max_leaves - 1,
            ..exact
        },
        PublicationLimits {
            max_additional_retained_bytes: exact.max_additional_retained_bytes - 1,
            ..exact
        },
        PublicationLimits {
            max_combined_preparation_peak_bytes: exact.max_combined_preparation_peak_bytes - 1,
            ..exact
        },
    ];
    for limits in one_below {
        let (family, context, session, _, ready) = ready_for_publication(NAME);
        let failure = PreparedPublication::prepare(ready, limits).unwrap_err();
        assert!(matches!(
            failure.error(),
            PublicationError::ResourceLimit { .. }
        ));
        let (_, returned_ready) = failure.into_parts();
        returned_ready.replay(&family, &context, &session).unwrap();
        let retry = PreparedPublication::prepare(returned_ready, exact).unwrap();
        assert_eq!(retry.stats(), baseline.stats());
    }
}

#[test]
fn atomic_publication_consumes_one_target_and_moves_compact_routes_once() {
    let (family, context, mut session, source, ready) =
        ready_for_publication("publication-atomic-success");
    let predecessor_version = session.state_version();
    let predecessor_events = session.event_stats().events();
    let predecessor_consumed = session.consumed_target_count();
    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
    let expected_stats = prepared.stats();
    let expected_leaves: Vec<_> = prepared
        .leaves()
        .map(|leaf| (leaf.case().id(), leaf.disposition()))
        .collect();
    let expected_retained = session.publication_retained_bytes_for_test(&prepared);

    let receipt = session.commit_publication(prepared).unwrap();

    assert_eq!(receipt.event_ordinal(), predecessor_events);
    assert_eq!(receipt.source_ordinal(), predecessor_events);
    assert!(receipt.pivot_ordinal() < session.state_version());
    assert_eq!(receipt.stats(), expected_stats);
    assert_eq!(session.state_version(), predecessor_version + 1);
    assert_eq!(session.event_stats().events(), predecessor_events + 1);
    assert_eq!(session.consumed_target_count(), predecessor_consumed + 1);
    assert_eq!(
        session.event_stats().publication_retained_bytes(),
        expected_retained
    );
    assert_eq!(
        receipt.stats().applicable() + receipt.stats().exceptional(),
        receipt.stats().leaves()
    );
    assert_eq!(
        session.authenticate_event_ledger_census_for_test().unwrap(),
        session.event_stats().ledger_retained_bytes()
    );
    let committed_leaves: Vec<_> = session
        .last_publication_payload_for_test()
        .expect("the chronological event must own the compact publication")
        .leaves()
        .map(|leaf| (leaf.case().id(), leaf.disposition()))
        .collect();
    assert_eq!(committed_leaves, expected_leaves);
    assert_eq!(
        session.replay(&family, &context),
        Err(GeneratedAffineResidualGroupExactSessionError::ReplayMismatch)
    );

    let publication_events = session.event_stats().events();
    let publication_consumed = session.consumed_target_count();
    let transaction = session
        .stage_replayed_row(&family, &context, &source)
        .unwrap();
    let classified = session
        .classify_dependent(transaction)
        .expect("the published source must now reduce through its installed pivot");
    session
        .commit_dependent(&family, &context, classified)
        .unwrap();
    assert_eq!(session.event_stats().events(), publication_events + 1);
    assert_eq!(session.consumed_target_count(), publication_consumed);
}

#[test]
fn stale_publication_failure_returns_the_exact_inspectable_owner_without_mutation() {
    let (family, context, mut session, source, ready) =
        ready_for_publication("publication-atomic-stale");
    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
    let expected_stats = prepared.stats();
    session
        .advance_competing_publication_head_for_test(&family, &context, &source, &prepared)
        .unwrap();
    let stale_version = session.state_version();
    let stale_events = session.event_stats();
    let stale_consumed = session.consumed_target_count();

    let failure = session.commit_publication(prepared).unwrap_err();
    assert_eq!(
        failure.error(),
        GeneratedAffineResidualGroupExactSessionError::WrongTargetStateAllocation
    );
    let recovered = failure.into_publication();
    assert_eq!(recovered.stats(), expected_stats);
    assert_eq!(recovered.leaves().len(), expected_stats.leaves());
    assert_eq!(session.state_version(), stale_version);
    assert_eq!(session.event_stats(), stale_events);
    assert_eq!(session.consumed_target_count(), stale_consumed);
}

#[test]
fn publication_commit_exact_event_limits_pass_and_one_below_is_transactional() {
    const NAME: &str = "publication-atomic-limits";
    let (_, _, mut pilot_session, _, pilot_ready) = ready_for_publication(NAME);
    let predecessor_events = pilot_session.event_stats().events();
    let pilot = PreparedPublication::prepare(pilot_ready, PublicationLimits::default()).unwrap();
    let required_event_bytes = pilot_session
        .commit_publication(pilot)
        .unwrap()
        .retained_event_bytes();
    assert!(required_event_bytes > 0);

    let mut exact_limits = GeneratedAffineResidualGroupExactSessionLimits::default();
    exact_limits.events.max_events = predecessor_events + 1;
    exact_limits.events.max_individual_event_retained_bytes = required_event_bytes;
    exact_limits.target_state.max_target_consumptions = 1;
    let (_, _, mut exact_session, _, exact_ready) =
        ready_for_publication_with_session_limits(NAME, exact_limits);
    let exact = PreparedPublication::prepare(exact_ready, PublicationLimits::default()).unwrap();
    exact_session.commit_publication(exact).unwrap();

    let cases = [
        (
            {
                let mut limits = exact_limits;
                limits.events.max_events = predecessor_events;
                limits
            },
            "events",
        ),
        (
            {
                let mut limits = exact_limits;
                limits.events.max_individual_event_retained_bytes = required_event_bytes - 1;
                limits
            },
            "event bytes",
        ),
        (
            {
                let mut limits = exact_limits;
                limits.target_state.max_target_consumptions = 0;
                limits
            },
            "target consumption",
        ),
    ];

    for (limits, kind) in cases {
        let (_, _, mut session, _, ready) = ready_for_publication_with_session_limits(NAME, limits);
        let before_version = session.state_version();
        let before_events = session.event_stats();
        let before_consumed = session.consumed_target_count();
        let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
        let expected_stats = prepared.stats();
        let failure = session.commit_publication(prepared).unwrap_err();
        match kind {
            "events" => assert!(matches!(
                failure.error(),
                GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                    resource: "exact session committed events",
                    requested,
                    limit,
                } if requested == predecessor_events + 1 && limit == predecessor_events
            )),
            "event bytes" => assert!(matches!(
                failure.error(),
                GeneratedAffineResidualGroupExactSessionError::EventResourceLimit {
                    resource: "exact session individual event retained bytes",
                    requested,
                    limit,
                } if requested == required_event_bytes && limit == required_event_bytes - 1
            )),
            "target consumption" => assert!(matches!(
                failure.error(),
                GeneratedAffineResidualGroupExactSessionError::Target(
                    GeneratedAffineResidualGroupExactTargetError::ResourceLimit {
                        resource: "exact target consumptions",
                        requested: 1,
                        limit: 0,
                    }
                )
            )),
            _ => unreachable!(),
        }
        let recovered = failure.into_publication();
        assert_eq!(recovered.stats(), expected_stats);
        assert_eq!(recovered.leaves().len(), expected_stats.leaves());
        assert_eq!(session.state_version(), before_version);
        assert_eq!(session.event_stats(), before_events);
        assert_eq!(session.consumed_target_count(), before_consumed);
    }
}
