//! Focused acceptance tests for move-bound compact publication preparation.

use std::mem::size_of;

use crate::generated_affine_residual_group_exact_publication::{
    PreparedPublication, PublicationError, PublicationLeafDisposition, PublicationLimits,
    publication_clause_source_is_domain_for_test, publication_route_word_bytes_for_test,
};
use crate::generated_affine_residual_group_exact_session::{
    GeneratedAffineResidualGroupExactSession,
    tests::{ExactConditionPlanTestFixture, exact_condition_plan_test_fixture_in_sector},
};
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
    GeneratedAffineResidualGroupExactWhenBadReadyForPublication,
) {
    let ExactConditionPlanTestFixture {
        family,
        context,
        session,
        ready,
    } = exact_condition_plan_test_fixture_in_sector(name, "011", false);
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
    (family, context, session, ready)
}

#[test]
fn prepared_publication_owns_ready_and_one_correct_route_per_leaf() {
    let (family, context, session, ready) = ready_for_publication("publication-prepared-natural");
    let ready_retained_bytes = ready.stats().retained_owned_logical_bytes();

    let mut mapped_recentered_guard = false;
    let mut mapped_denominator_identity = false;
    let mut mapped_retained_boundary = false;
    for provenance in ready.clause_provenance() {
        match provenance.source() {
            source @ GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard {
                ..
            } => {
                mapped_recentered_guard = true;
                assert!(publication_clause_source_is_domain_for_test(source));
            }
            source
            @ GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity {
                ..
            } => {
                mapped_denominator_identity = true;
                assert!(publication_clause_source_is_domain_for_test(source));
            }
            source @ GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary {
                ..
            } => {
                mapped_retained_boundary = true;
                assert!(!publication_clause_source_is_domain_for_test(source));
            }
        }
    }
    assert!(mapped_recentered_guard);
    assert!(mapped_denominator_identity);
    assert!(mapped_retained_boundary);

    let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();

    prepared
        .ready()
        .replay(&family, &context, &session)
        .unwrap();
    assert_eq!(
        prepared.leaves().len(),
        prepared.ready().partition().cases().len()
    );
    assert_eq!(prepared.stats().leaves(), prepared.leaves().len());
    assert!(prepared.stats().applicable() > 0);
    assert!(prepared.stats().exceptional_domain() > 0);
    assert!(prepared.stats().exceptional_leak() > 0);

    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    let mut later_decisive_clause = false;
    for ((ordinal, leaf), classification) in prepared
        .leaves()
        .enumerate()
        .zip(prepared.ready().partition().classifications())
    {
        assert_eq!(leaf.ordinal(), ordinal);
        assert_eq!(leaf.case().id(), classification.case());
        let disposition = leaf.disposition();
        assert_eq!(
            leaf.provenance().map(|provenance| provenance.ordinal()),
            classification.decisive_clause_ordinal()
        );
        match classification.decisive_clause_ordinal() {
            None => {
                applicable += 1;
                assert_eq!(disposition, PublicationLeafDisposition::Applicable);
                assert!(leaf.provenance().is_none());
            }
            Some(clause) => {
                later_decisive_clause |= clause > 0;
                let provenance = leaf.provenance().expect("exceptional leaf has provenance");
                assert_eq!(provenance.ordinal(), clause);
                match disposition {
                    PublicationLeafDisposition::ExceptionalDomain {
                        provenance: route_provenance,
                    } => {
                        assert!(std::ptr::eq(route_provenance, provenance));
                        assert!(matches!(
                            provenance.source(),
                            GeneratedAffineResidualGroupExactWhenBadClauseSource::RecenteredRowGuard { .. }
                                | GeneratedAffineResidualGroupExactWhenBadClauseSource::DenominatorIdentity { .. }
                        ));
                        exceptional_domain += 1;
                    }
                    PublicationLeafDisposition::ExceptionalLeak {
                        provenance: route_provenance,
                    } => {
                        assert!(std::ptr::eq(route_provenance, provenance));
                        assert!(matches!(
                            provenance.source(),
                            GeneratedAffineResidualGroupExactWhenBadClauseSource::RetainedBoundary { .. }
                        ));
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

    let route_word_bytes = publication_route_word_bytes_for_test();
    assert_eq!(route_word_bytes, size_of::<usize>());
    let payload_bytes = prepared.leaves().len() * route_word_bytes;
    let header_delta = size_of::<PreparedPublication>()
        - size_of::<GeneratedAffineResidualGroupExactWhenBadReadyForPublication>();
    let additional = header_delta + payload_bytes;
    let combined_peak = (ready_retained_bytes + additional)
        .max(ready_retained_bytes + size_of::<Vec<usize>>() + payload_bytes);
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
    let (_, _, _, baseline_ready) = ready_for_publication(NAME);
    let baseline =
        PreparedPublication::prepare(baseline_ready, PublicationLimits::default()).unwrap();
    let exact = PublicationLimits {
        max_leaves: baseline.stats().leaves(),
        max_additional_retained_bytes: baseline.stats().additional_retained_bytes(),
        max_combined_preparation_peak_bytes: baseline.stats().combined_preparation_peak_bytes(),
    };

    let (_, _, _, exact_ready) = ready_for_publication(NAME);
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
        let (family, context, session, ready) = ready_for_publication(NAME);
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
