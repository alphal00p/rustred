//! Focused acceptance tests for compact application-event preparation and commit.

use std::mem::size_of;
use std::sync::Arc;

use crate::generated_affine_residual_group_exact_physical_row::GeneratedAffineResidualGroupExactPhysicalRow;
use crate::generated_affine_residual_group_exact_publication::{
    PreparedPublication, PublicationError, PublicationLeafDisposition, PublicationLimits,
    publication_route_tag_bytes_for_test,
};
use crate::generated_affine_residual_group_exact_session::{
    GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionError,
    GeneratedAffineResidualGroupExactSessionLimits,
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
use crate::{IntegralFamily, ParametricCoefficientContext};

fn ready_for_publication(
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
    session.commit_publication(prepared).unwrap();
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
