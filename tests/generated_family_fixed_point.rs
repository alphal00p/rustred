//! Black-box regressions for the generic residual-anchor fixed point.
//!
//! The tadpole and sunset are adversarial fixtures only.  The production
//! compiler receives an `IntegralFamily`, its generated coefficient context,
//! a generic base-family certificate, and resource policies.  No topology
//! name, loop-count dispatch, recurrence, master list, or expected rule is an
//! input to rule discovery.

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, ConcreteIntegralKey, CoordinateEqualityLeafStatus,
    GeneratedAnchorWitnessSearchExhaustionReason, GeneratedFamilyFixedPointAttemptOutcome,
    GeneratedFamilyFixedPointBasePreparationOutcome, GeneratedFamilyFixedPointCompiler,
    GeneratedFamilyFixedPointConfig, GeneratedFamilyFixedPointFinalStatus,
    GeneratedFamilyFixedPointLimits, GeneratedFamilyFixedPointSelectionPolicy,
    GeneratedFamilyRuleSystemCompiler, GeneratedFamilyRuleSystemConfig,
    GeneratedFamilyRuleSystemLimits, GeneratedFixedPointMaterialLocator,
    GeneratedResidualAnchorOrigin, GeneratedResidualCandidateOutcome,
    GeneratedSectorLiveLeafOutcome, GeneratedSectorQueuedSourceDisposition, IntegralFamily,
    IntegralOrderingPolicy, ParametricIbpGenerator, ParametricRuleApplication,
    ParametricRuleInapplicability, ParametricSectorLeafDisposition, PowerShiftPolicy, SectorMask,
    SectorRestrictions,
};

fn mask(bits: &str) -> SectorMask {
    SectorMask::try_from_bit_string(bits).unwrap()
}

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

fn massive_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

/// Connected equal-mass sunset with `D3 = (k0+k1)^2-m2`.
fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let mass = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(mass.clone(), vec![one.clone(), zero.clone(), zero.clone()]),
            AffineDenominator::new(mass.clone(), vec![zero.clone(), zero.clone(), one.clone()]),
            AffineDenominator::new(mass, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn compile_base(
    family: &IntegralFamily,
) -> (
    rustred::ParametricCoefficientContext,
    rustred::GeneratedFamilyRuleSystemCertificate,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.discovery.adaptive.max_search_depth = 0;
    limits.live_leaf_queue.translation_radius = 0;
    limits.live_leaf_queue.max_translation_points = 1;
    let base = GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        limits,
    )
    .unwrap();
    (context, base)
}

fn fixed_point_config(
    residual_frontier_depth: usize,
    residual_anchor_local_depth: usize,
    maximum_local_depth: usize,
) -> GeneratedFamilyFixedPointConfig {
    GeneratedFamilyFixedPointConfig {
        base_search_depth: 0,
        maximum_rounds: 1,
        residual_frontier_depth,
        residual_anchor_local_depth,
        maximum_local_depth,
        selection: GeneratedFamilyFixedPointSelectionPolicy::ResidualSubsectorFirstPrefix {
            max_selected_sectors: 1,
        },
        stop_on_no_strict_improvement: false,
    }
}

#[test]
fn tadpole_anchor_usefulness_does_not_erase_the_symbolic_bad_locus() {
    let family = massive_tadpole("opaque-fixed-point-adversary");
    let (context, base) = compile_base(&family);
    let certificate = GeneratedFamilyFixedPointCompiler::compile(
        &family,
        &context,
        base,
        // Search n=1 at depth zero with no deeper local retry. Exhaustion of
        // that configured witness search must remain distinct from a master.
        fixed_point_config(1, 0, 0),
        GeneratedFamilyFixedPointLimits::default(),
    )
    .unwrap();

    let preparation = certificate
        .base_preparations()
        .first()
        .expect("the selected active sector must have one phase-zero preparation");
    assert_eq!(preparation.ordinal(), 0);
    assert_eq!(preparation.sector(), &mask("1"));
    assert_eq!(
        preparation.input_material(),
        GeneratedFixedPointMaterialLocator::BaseRuleSystem { solve_ordinal: 0 }
    );
    let GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
        search_discovery,
        after,
        discovery,
        live_leaf_queue,
        accepted_candidates,
    } = preparation.outcome()
    else {
        panic!("phase-zero tadpole preparation did not complete")
    };
    assert_eq!(search_discovery.candidate_counts_by_layer(), [1]);
    assert_eq!(accepted_candidates.len(), 1);
    assert!(!after.is_empty());
    assert!(matches!(
        search_discovery
            .coverage()
            .classification_for_indices(&context, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));
    assert!(matches!(
        search_discovery
            .coverage()
            .classification_for_indices(&context, &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule {
            candidate_ordinal: 0
        }
    ));

    let shared = certificate.base().row_span_arc().unwrap();
    assert!(Arc::ptr_eq(search_discovery.row_span_arc(), shared));
    assert!(Arc::ptr_eq(discovery.row_span_arc(), shared));
    assert_eq!(live_leaf_queue.work_items().len(), 1);
    let residual = &live_leaf_queue.work_items()[0];
    assert_eq!(residual.ordinal(), 0);
    assert_eq!(
        residual.source_disposition(),
        &GeneratedSectorQueuedSourceDisposition::Uncovered
    );
    assert_eq!(residual.extraction().assignment().entries(), [(0, 1)]);
    assert_eq!(
        residual.extraction().status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    assert!(residual.extraction().unresolved_predicates().is_empty());
    assert!(matches!(
        residual.outcome(),
        GeneratedSectorLiveLeafOutcome::PartialReelimination { .. }
    ));

    let round = certificate
        .rounds()
        .first()
        .expect("one round was requested");
    assert_eq!(round.ordinal(), 0);
    assert_eq!(round.attempts().len(), 1);
    let attempt = &round.attempts()[0];
    assert_eq!(
        attempt.input_material(),
        GeneratedFixedPointMaterialLocator::BasePreparation {
            preparation_ordinal: 0
        }
    );
    assert!(attempt.newly_accepted_candidates().is_empty());
    assert!(matches!(
        attempt.outcome(),
        GeneratedFamilyFixedPointAttemptOutcome::NoCandidateCoveredRequestAnchors { after }
            if after == attempt.before()
    ));
    assert_eq!(attempt.anchor_searches().len(), 1);
    let search = &attempt.anchor_searches()[0];
    assert_eq!(search.request_anchor(), &key([1]));
    assert_eq!(search.requested_local_depth(), 0);
    assert_eq!(search.origins().len(), 1);
    assert!(matches!(
        search.origins()[0],
        GeneratedResidualAnchorOrigin::CoordinateAssignment {
            material: GeneratedFixedPointMaterialLocator::BasePreparation {
                preparation_ordinal: 0
            },
            work_item_ordinal: 0,
        }
    ));
    assert_eq!(search.selected_visit_ordinal(), None);
    assert_eq!(search.visited().len(), 1);
    let visit = &search.visited()[0];
    assert_eq!(visit.locator().local_depth(), 0);
    assert_eq!(visit.locator().within_layer_ordinal(), 0);
    assert!(matches!(
        visit.outcome(),
        GeneratedResidualCandidateOutcome::CertifiedNotCoveringRequestAnchor { .. }
    ));
    let candidate = visit.outcome().compilation().candidate();
    assert_eq!(candidate.discovery_anchor(), [2]);
    assert!(matches!(
        candidate.apply(&context, &[1]).unwrap(),
        ParametricRuleApplication::Inapplicable(
            ParametricRuleInapplicability::NonzeroGuardVanished
        )
    ));
    assert!(Arc::ptr_eq(
        visit
            .outcome()
            .compilation()
            .source_authentication()
            .row_span_arc(),
        shared
    ));

    let status = certificate.final_status(&mask("1")).unwrap();
    assert_eq!(
        status.latest_material(),
        GeneratedFixedPointMaterialLocator::BasePreparation {
            preparation_ordinal: 0
        }
    );
    assert_eq!(
        status.cumulative_accepted_candidates(),
        accepted_candidates.as_ref()
    );
    assert!(matches!(
        status.status(),
        GeneratedFamilyFixedPointFinalStatus::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
            residual,
            reason: GeneratedAnchorWitnessSearchExhaustionReason::MaximumLocalSearchDepthExhausted,
        } if !residual.is_empty()
    ));
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn phase_zero_charges_only_novel_candidates_after_payload_deduplication() {
    let family = massive_tadpole("phase-zero-exact-candidate-budget");
    let (context, base) = compile_base(&family);
    let base_attempts = match base.status(&mask("1")).unwrap() {
        rustred::GeneratedFamilySectorStatus::Unresolved { discovery, .. } => {
            discovery.coverage().candidate_attempts().len()
        }
        other => panic!("expected an unresolved tadpole sector, got {other:?}"),
    };
    assert_eq!(base_attempts, 1);

    let mut limits = GeneratedFamilyFixedPointLimits::default();
    // The aggregate transcript owns the phase-zero reference and repeats it
    // in the final status, so two retained references are the exact budget
    // for one composed payload.
    limits.max_accepted_candidate_references = 2 * base_attempts;
    let mut config = fixed_point_config(1, 0, 0);
    // Isolate phase zero: a later residual round is allowed to discover a
    // genuinely new candidate and must be charged separately.
    config.maximum_rounds = 0;
    let certificate =
        GeneratedFamilyFixedPointCompiler::compile(&family, &context, base, config, limits)
            .unwrap();
    let GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
        accepted_candidates,
        ..
    } = certificate.base_preparations()[0].outcome()
    else {
        panic!("phase-zero tadpole preparation did not complete")
    };
    assert_eq!(accepted_candidates.len(), base_attempts);
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn sunset_011_reanchors_on_the_first_authenticated_numerator_frontier() {
    let family = equal_mass_sunset("opaque-family-name-with-no-loop-dispatch");
    let (context, base) = compile_base(&family);
    let certificate = GeneratedFamilyFixedPointCompiler::compile(
        &family,
        &context,
        base,
        fixed_point_config(1, 1, 1),
        GeneratedFamilyFixedPointLimits::default(),
    )
    .unwrap();

    let preparation = certificate
        .base_preparations()
        .first()
        .expect("subsector-first prefix must select 011");
    assert_eq!(preparation.sector(), &mask("011"));
    let base_material = certificate
        .material(GeneratedFixedPointMaterialLocator::BaseRuleSystem { solve_ordinal: 0 })
        .expect("the original 011 base material must remain addressable");
    assert_eq!(base_material.live_leaf_queue().work_items().len(), 1);
    let original_residual = &base_material.live_leaf_queue().work_items()[0];
    assert_eq!(original_residual.source_case().value(), 3);
    assert_eq!(
        original_residual.source_disposition(),
        &GeneratedSectorQueuedSourceDisposition::Unsupported {
            candidate_ordinals: vec![0, 2].into_boxed_slice(),
        }
    );
    let GeneratedFamilyFixedPointBasePreparationOutcome::Prepared {
        live_leaf_queue,
        accepted_candidates,
        ..
    } = preparation.outcome()
    else {
        panic!("phase-zero 011 preparation did not complete")
    };

    // This is the crucial adversary: the residual leaf contains no equality
    // assignment.  J(-1,1,1) therefore has to come from the generic persisted
    // frontier scheduler, not from invented coordinate-extraction provenance.
    assert_eq!(live_leaf_queue.work_items().len(), 1);
    let residual = &live_leaf_queue.work_items()[0];
    assert_eq!(residual.ordinal(), 0);
    assert_eq!(residual.source_case().value(), 3);
    // Phase zero is monotone in its input material: the original base
    // attempts remain installed even when the shallower fresh search finds
    // no additional descending candidate.  The residual therefore retains
    // the authenticated unsupported-candidate provenance instead of being
    // weakened to an uncovered leaf.
    assert_eq!(
        residual.source_disposition(),
        &GeneratedSectorQueuedSourceDisposition::Unsupported {
            candidate_ordinals: vec![0, 2].into_boxed_slice(),
        }
    );
    assert!(residual.extraction().assignment().is_empty());
    assert_eq!(
        residual.extraction().status(),
        &CoordinateEqualityLeafStatus::NotProvedEmpty
    );
    assert_eq!(residual.extraction().unresolved_predicates().len(), 2);
    assert!(matches!(
        residual.outcome(),
        GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment
    ));

    let round = certificate
        .rounds()
        .first()
        .expect("one round was requested");
    assert_eq!(round.attempts().len(), 1);
    let attempt = &round.attempts()[0];
    assert_eq!(attempt.sector(), &mask("011"));
    assert_eq!(
        attempt.input_material(),
        GeneratedFixedPointMaterialLocator::BasePreparation {
            preparation_ordinal: 0
        }
    );
    let request = key([-1, 1, 1]);
    let (search_ordinal, search) = attempt
        .anchor_searches()
        .iter()
        .enumerate()
        .find(|(_, search)| search.request_anchor() == &request)
        .expect("the first numerator frontier must schedule J(-1,1,1)");
    assert_eq!(search.requested_local_depth(), 1);
    assert_eq!(search.origins().len(), 1);
    assert!(matches!(
        search.origins()[0],
        GeneratedResidualAnchorOrigin::ResidualFrontier {
            material: GeneratedFixedPointMaterialLocator::BasePreparation {
                preparation_ordinal: 0
            },
            work_item_ordinal: 0,
            frontier_depth: 1,
            within_frontier_ordinal: 0,
        }
    ));

    // The re-anchored adaptive search has layers [4, 19].  It visits all four
    // depth-zero candidates and the first eleven depth-one candidates before
    // selecting locator (1,10), i.e. global visit ordinal 14.
    assert_eq!(search.selected_visit_ordinal(), Some(14));
    assert_eq!(search.visited().len(), 15);
    for (within, visit) in search.visited()[..4].iter().enumerate() {
        assert_eq!(visit.locator().local_depth(), 0);
        assert_eq!(visit.locator().within_layer_ordinal(), within);
        assert!(!visit.outcome().covers_request_anchor());
    }
    for (within, visit) in search.visited()[4..].iter().enumerate() {
        assert_eq!(visit.locator().local_depth(), 1);
        assert_eq!(visit.locator().within_layer_ordinal(), within);
    }
    let selected = &search.visited()[14];
    assert_eq!(selected.locator().local_depth(), 1);
    assert_eq!(selected.locator().within_layer_ordinal(), 10);
    assert!(matches!(
        selected.outcome(),
        GeneratedResidualCandidateOutcome::CertifiedCoveredRequestAnchor { .. }
    ));
    let compilation = selected.outcome().compilation();
    assert_eq!(compilation.candidate().discovery_anchor(), [-2, 1, 1]);
    let shared = certificate.base().row_span_arc().unwrap();
    assert!(Arc::ptr_eq(
        compilation.source_authentication().row_span_arc(),
        shared
    ));

    let ParametricRuleApplication::Applicable(application) = compilation
        .candidate()
        .apply(&context, request.powers())
        .unwrap()
    else {
        panic!("selected generated candidate is not applicable at J(-1,1,1)")
    };
    assert_eq!(application.source(), &request);
    let coefficients = family.coefficient_context();
    let expected_rhs = BTreeMap::from([
        (key([0, 0, 1]), coefficients.parse("1/(d-1)").unwrap()),
        (key([0, 0, 2]), coefficients.parse("2*m2/(d-1)").unwrap()),
        (key([0, 1, 0]), coefficients.parse("-1/(d-1)").unwrap()),
        (key([0, 1, 1]), coefficients.parse("m2").unwrap()),
    ]);
    assert_eq!(application.rhs(), &expected_rhs);
    assert!(application.verify_descent(IntegralOrderingPolicy::RustRedUnshiftedV1));

    assert_eq!(attempt.newly_accepted_candidates().len(), 1);
    assert!(matches!(
        attempt.newly_accepted_candidates()[0].origin(),
        rustred::GeneratedAcceptedCandidateOrigin::ResidualSelection {
            round_ordinal: 0,
            sector_attempt_ordinal: 0,
            anchor_search_ordinal,
            visit_ordinal: 14,
        } if *anchor_search_ordinal == search_ordinal
    ));
    let GeneratedFamilyFixedPointAttemptOutcome::Completed {
        strict_improvement,
        after,
        discovery: recomposed_discovery,
        ..
    } = attempt.outcome()
    else {
        panic!("the selected generated candidate was not globally recomposed")
    };
    assert!(!after.is_empty());
    assert!(
        *strict_improvement,
        "a newly accepted candidate must count as progress: residual measure changed from ({}, {}) to ({}, {})",
        attempt.before().root_leaves(),
        attempt.before().predicate_instances(),
        after.root_leaves(),
        after.predicate_instances(),
    );
    assert!(Arc::ptr_eq(recomposed_discovery.row_span_arc(), shared));
    assert!(matches!(
        recomposed_discovery
            .coverage()
            .classification_for_indices(&context, request.powers())
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule { .. }
    ));

    let status = certificate.final_status(&mask("011")).unwrap();
    assert_eq!(
        status.latest_material(),
        GeneratedFixedPointMaterialLocator::ResidualRound {
            round_ordinal: 0,
            sector_attempt_ordinal: 0,
        }
    );
    assert_eq!(
        status.cumulative_accepted_candidates().len(),
        accepted_candidates.len() + 1
    );
    assert!(matches!(
        status.status(),
        GeneratedFamilyFixedPointFinalStatus::ExhaustedAtMaximumRounds { residual }
            if !residual.is_empty()
    ));

    // Rebuild every source row, anchor, candidate locator, guard partition,
    // queue leaf and material locator from the topology-independent inputs.
    certificate.replay(&family, &context).unwrap();
}
