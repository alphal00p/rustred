use std::sync::Arc;

use crate::generated_residual_affine_group_effective_coverage::{
    GeneratedResidualAffineGroupEffectiveCoverageCompiler,
    GeneratedResidualAffineGroupEffectiveCoverageError,
    GeneratedResidualAffineGroupEffectiveCoverageLimits,
    GeneratedResidualAffineGroupTargetDisposition, GeneratedResidualAffineLocalTransitionKind,
    GeneratedResidualAffineResidualWorkKind, GeneratedResidualAffineSequentialTargetState,
    GeneratedResidualAffineTargetAttemptOutcome, distribute_unconsumed_rejected_attempts,
};
use crate::{
    AffineDenominator, AffineParametricOrderingLimits, AffinePreparePointScheduleCertificate,
    AffinePreparePointScheduleLimits, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, CoefficientContext,
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationCompiler,
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryCompiler,
    GeneratedResidualAffineCaseInventoryLimits,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingCompiler,
    GeneratedResidualAffinePivotTargetMatchingLimits, GeneratedResidualAffinePivotTargetOutcome,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator, SectorMask,
};

#[test]
fn pure_sequential_state_consumes_only_certified_targets() {
    let mut state = GeneratedResidualAffineSequentialTargetState::try_with_group_size(2).unwrap();
    assert!(
        !state
            .commit_selected(
                17,
                0,
                GeneratedResidualAffineLocalTransitionKind::Unsupported
            )
            .unwrap()
    );
    assert!(!state.is_consumed_position(0));
    assert!(
        !state
            .commit_selected(
                17,
                0,
                GeneratedResidualAffineLocalTransitionKind::IdenticallyBad,
            )
            .unwrap()
    );
    assert!(!state.is_consumed_position(0));
    assert!(
        state
            .commit_selected(17, 0, GeneratedResidualAffineLocalTransitionKind::Certified)
            .unwrap()
    );
    assert!(state.is_consumed_position(0));
    assert!(
        state
            .commit_selected(23, 1, GeneratedResidualAffineLocalTransitionKind::Certified)
            .unwrap()
    );
    assert!(state.is_consumed_position(1));
    assert_eq!(state.consumed_count(), 2);
}

#[test]
fn pure_sequential_state_rejects_double_acceptance() {
    let mut state = GeneratedResidualAffineSequentialTargetState::try_with_group_size(1).unwrap();
    state
        .commit_selected(17, 0, GeneratedResidualAffineLocalTransitionKind::Certified)
        .unwrap();
    assert!(matches!(
        state.commit_selected(17, 0, GeneratedResidualAffineLocalTransitionKind::Certified),
        Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::TargetAcceptedTwice {
                target_case_ordinal: 17
            }
        )
    ));
}

#[test]
fn pure_reject_then_accept_finishes_with_a_consumed_target() {
    let mut state = GeneratedResidualAffineSequentialTargetState::try_with_group_size(1).unwrap();
    assert!(
        !state
            .commit_selected(
                17,
                0,
                GeneratedResidualAffineLocalTransitionKind::IdenticallyBad,
            )
            .unwrap()
    );
    assert!(!state.is_consumed_position(0));
    assert!(
        state
            .commit_selected(17, 0, GeneratedResidualAffineLocalTransitionKind::Certified)
            .unwrap()
    );
    assert!(state.is_consumed_position(0));
    assert_eq!(state.consumed_count(), 1);
}

#[test]
fn pure_rejected_reference_distribution_is_linear_ordered_and_omits_consumed_targets() {
    let mut retained_bytes = 0usize;
    let grouped = distribute_unconsumed_rejected_attempts(
        &[true, false, false],
        &[2, 1, 1],
        &[(0, 0), (1, 1), (0, 2), (2, 3)],
        &mut retained_bytes,
        usize::MAX,
    )
    .unwrap();
    assert!(grouped[0].is_empty());
    assert_eq!(grouped[1], [1]);
    assert_eq!(grouped[2], [3]);
    assert!(retained_bytes >= 2 * std::mem::size_of::<usize>());
}

#[test]
fn pure_two_target_fixture_falls_through_only_after_acceptance() {
    let candidates = [17usize, 23usize];
    let first_available = |state: &GeneratedResidualAffineSequentialTargetState| {
        candidates
            .iter()
            .copied()
            .enumerate()
            .find_map(|(position, target)| {
                (!state.is_consumed_position(position)).then_some(target)
            })
    };
    let mut state = GeneratedResidualAffineSequentialTargetState::try_with_group_size(2).unwrap();
    assert_eq!(first_available(&state), Some(17));
    state
        .commit_selected(
            17,
            0,
            GeneratedResidualAffineLocalTransitionKind::Unsupported,
        )
        .unwrap();
    assert_eq!(first_available(&state), Some(17));
    state
        .commit_selected(
            17,
            0,
            GeneratedResidualAffineLocalTransitionKind::IdenticallyBad,
        )
        .unwrap();
    assert_eq!(first_available(&state), Some(17));
    state
        .commit_selected(17, 0, GeneratedResidualAffineLocalTransitionKind::Certified)
        .unwrap();
    assert_eq!(first_available(&state), Some(23));
    state
        .commit_selected(23, 1, GeneratedResidualAffineLocalTransitionKind::Certified)
        .unwrap();
    assert_eq!(first_available(&state), None);
}

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn generated_matcher(
    bits: &str,
    source_case_ordinal: usize,
    name: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
) {
    let family = equal_mass_sunset(name);
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string(bits).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let inventory = Arc::new(
        GeneratedResidualAffineCaseInventoryCompiler::compile(
            &family,
            &context,
            queue,
            GeneratedResidualAffineCaseInventoryLimits::default(),
        )
        .unwrap(),
    );
    let case = &inventory.cases()[source_case_ordinal];
    let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
        &family,
        &context,
        case.source_cover().clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        case.source_branch().clone(),
        AffineParametricOrderingLimits::default(),
    )
    .unwrap();
    let schedule = Arc::new(
        AffinePreparePointScheduleCertificate::compile_with_authority(
            AffineStartReplayAuthority::ResidualBooleanBranch {
                family: &family,
                context: &context,
                cover: case.source_cover(),
            },
            ordering,
            0,
            AffinePreparePointScheduleLimits::default(),
        )
        .unwrap(),
    );
    let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(reelimination) =
        GeneratedResidualAffineBranchReeliminationCompiler::compile(
            &family,
            &context,
            schedule,
            case.guard_composition().clone(),
            GeneratedResidualAffineBranchReeliminationLimits::default(),
        )
        .unwrap()
    else {
        panic!("generated 001 fixture must eliminate");
    };
    let matcher = Arc::new(
        GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            source_case_ordinal,
            Arc::new(reelimination),
            GeneratedResidualAffinePivotTargetMatchingLimits::default(),
        )
        .unwrap(),
    );
    (family, context, inventory, matcher)
}

fn generated_001_matcher() -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
) {
    let fixture = generated_matcher("001", 0, "group-effective-coverage-sunset-001");
    assert_eq!(fixture.2.cases().len(), 1);
    fixture
}

#[test]
fn generated_001_consumes_once_then_exhausts_and_replays() {
    let (family, context, inventory, matcher) = generated_001_matcher();
    assert_eq!(
        inventory.groups()[matcher.source_group_ordinal()].case_ordinals(),
        [0]
    );
    assert_eq!(matcher.outcomes().len(), 4);
    let empty_consumed = std::collections::BTreeSet::<usize>::new();
    for outcome in matcher.outcomes() {
        let GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) = outcome
        else {
            panic!("generated 001 fixture changed outcome: {outcome:?}");
        };
        assert_eq!(pending.matching_target_case_ordinals(), [0]);
        assert!(matches!(
            pending
                .first_available_target_for_effective_coverage(
                    |case_ordinal| empty_consumed.contains(&case_ordinal),
                    1,
                )
                .unwrap(),
            crate::generated_residual_affine_pivot_target_matching::GeneratedResidualAffineEffectiveTargetSelection::Selected {
                case_ordinal: 0,
                position: 0,
                references_inspected: 1,
            }
        ));
        assert!(matches!(
            pending.first_available_target_for_effective_coverage(
                |case_ordinal| empty_consumed.contains(&case_ordinal),
                0,
            ),
            Err(crate::generated_residual_affine_pivot_target_matching::GeneratedResidualAffineEffectiveTargetSelectionError::ResourceLimit {
                requested: 1,
                limit: 0,
            })
        ));
    }

    let certificate = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
        &family,
        &context,
        matcher,
        GeneratedResidualAffineGroupEffectiveCoverageLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();
    assert_eq!(certificate.attempts().len(), 4);
    assert_eq!(
        certificate.attempts()[0].selected_target_case_ordinal(),
        Some(0)
    );
    assert_eq!(
        certificate.attempts()[0].selected_target_position(),
        Some(0)
    );
    assert!(matches!(
        certificate.attempts()[0].outcome(),
        GeneratedResidualAffineTargetAttemptOutcome::Accepted(_)
    ));
    for attempt in &certificate.attempts()[1..] {
        assert_eq!(attempt.selected_target_case_ordinal(), None);
        assert!(matches!(
            attempt.outcome(),
            GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase
        ));
    }
    assert_eq!(certificate.target_dispositions().len(), 1);
    assert!(matches!(
        certificate.target_dispositions()[0].disposition(),
        GeneratedResidualAffineGroupTargetDisposition::Consumed {
            accepted_attempt_ordinal: 0,
            ..
        }
    ));
    assert!(!certificate.sealed_rules().is_empty());
    assert!(
        certificate
            .sealed_rules()
            .iter()
            .all(|rule| rule.target_case_ordinal() == 0)
    );
    assert!(certificate.residual_work().iter().all(|leaf| {
        leaf.target_case_ordinal() == 0
            && !matches!(
                leaf.kind(),
                GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
            )
    }));
    let stats = certificate.stats();
    assert_eq!(stats.matcher_outcomes_inspected(), 4);
    assert_eq!(stats.pending_target_selections(), 4);
    assert_eq!(stats.local_when_bad_compilations(), 1);
    assert_eq!(stats.accepted_attempts(), 1);
    assert_eq!(stats.rejected_attempts(), 3);
    assert_eq!(stats.consumed_targets(), 1);
    assert_eq!(stats.group_target_dispositions(), 1);
    assert_eq!(
        stats.sealed_conditional_rule_handles(),
        certificate.sealed_rules().len()
    );
    assert_eq!(
        stats.child_applicable_leaves(),
        certificate.sealed_rules().len()
    );
    assert_eq!(
        stats.exceptional_residual_leaves(),
        certificate.residual_work().len()
    );
    assert_eq!(
        stats.child_exceptional_leaves(),
        stats.exceptional_residual_leaves()
    );
    let formatted = format!("{certificate:?}");
    assert!(formatted.contains("<redacted>"));
    assert!(!formatted.contains("ParametricRelation"));
    assert!(!formatted.contains("coefficient_translation"));
}

#[test]
fn generated_011_and_101_retain_both_unconsumed_roots() {
    for bits in ["011", "101"] {
        let (family, context, inventory, matcher) =
            generated_matcher(bits, 1, &format!("group-effective-coverage-sunset-{bits}"));
        let group = &inventory.groups()[matcher.source_group_ordinal()];
        assert_eq!(matcher.source_group_ordinal(), 1);
        assert_eq!(group.case_ordinals(), [1, 3]);
        assert_eq!(matcher.outcomes().len(), 4);
        assert!(matcher.outcomes().iter().all(|outcome| matches!(
            outcome,
            GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(_)
        )));

        let mut certificate = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
            &family,
            &context,
            matcher,
            GeneratedResidualAffineGroupEffectiveCoverageLimits::default(),
        )
        .unwrap();
        certificate.replay(&family, &context).unwrap();
        assert_eq!(certificate.target_dispositions().len(), 2);
        assert!(
            certificate
                .target_dispositions()
                .iter()
                .all(|record| matches!(
                    record.disposition(),
                    GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                        rejected_attempt_ordinals,
                    } if rejected_attempt_ordinals.is_empty()
                ))
        );
        assert_eq!(certificate.residual_work().len(), 2);
        assert!(certificate.residual_work().iter().all(|leaf| matches!(
            leaf.kind(),
            GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
        )));
        let stats = certificate.stats();
        assert_eq!(stats.accepted_attempts(), 0);
        assert_eq!(stats.rejected_attempts(), 4);
        assert_eq!(stats.consumed_targets(), 0);
        assert_eq!(stats.unconsumed_residual_roots(), 2);
        assert_eq!(stats.exceptional_residual_leaves(), 0);
        if bits == "011" {
            assert!(certificate.test_only_corrupt_first_residual_authority_shape());
            assert!(matches!(
                certificate.test_only_validate_private_authorities(),
                Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)
            ));
        }
    }
}

fn exact_group_limits(
    certificate: &crate::generated_residual_affine_group_effective_coverage::GeneratedResidualAffineGroupEffectiveCoverageCertificate,
) -> GeneratedResidualAffineGroupEffectiveCoverageLimits {
    let stats = certificate.stats();
    let mut exact = certificate.limits();
    exact.max_matcher_outcomes_inspected = stats.matcher_outcomes_inspected();
    exact.max_pending_target_selections = stats.pending_target_selections();
    exact.max_checked_target_references = stats.checked_target_references();
    exact.max_matching_target_references = stats.matching_target_references();
    exact.max_selection_target_references_inspected = stats.selection_target_references_inspected();
    exact.max_group_cases_inspected = stats.group_cases_inspected();
    exact.max_local_when_bad_compilations = stats.local_when_bad_compilations();
    exact.max_accepted_attempts = stats.accepted_attempts();
    exact.max_rejected_attempts = stats.rejected_attempts();
    exact.max_consumed_targets = stats.consumed_targets();
    exact.max_rejected_attempt_references = stats.rejected_attempt_references();
    exact.max_rejected_attempt_references_per_target =
        stats.maximum_rejected_attempt_references_per_target();
    exact.max_child_source_terms = stats.child_source_terms();
    exact.max_child_source_exponent_entries = stats.child_source_exponent_entries();
    exact.max_child_source_integer_bits = stats.child_source_integer_bits();
    exact.max_child_output_terms = stats.child_output_terms();
    exact.max_child_output_exponent_entries = stats.child_output_exponent_entries();
    exact.max_child_native_integer_bit_work = stats.child_native_integer_bit_work();
    exact.max_child_total_integer_bit_work = stats.child_total_integer_bit_work();
    exact.max_child_payload_comparison_units = stats.child_payload_comparison_units();
    exact.max_child_payload_comparison_bytes = stats.child_payload_comparison_bytes();
    exact.max_child_payload_comparison_integer_bits = stats.child_payload_comparison_integer_bits();
    exact.max_child_payload_comparison_private_manifest_bytes =
        stats.child_payload_comparison_private_manifest_bytes();
    exact.max_child_structural_loci = stats.child_structural_loci();
    exact.max_child_bad_clauses = stats.child_bad_clauses();
    exact.max_child_relative_leaves = stats
        .child_applicable_leaves()
        .checked_add(stats.child_exceptional_leaves())
        .unwrap();
    exact.max_child_retained_bytes = stats.child_retained_bytes();
    exact.max_group_target_dispositions = stats.group_target_dispositions();
    exact.max_sealed_conditional_rule_handles = stats.sealed_conditional_rule_handles();
    exact.max_residual_work_leaves = stats.residual_work_leaves();
    exact.max_outer_retained_bytes = stats.outer_retained_bytes();
    exact.max_outer_payload_comparison_units = stats.outer_payload_comparison_units();
    exact.max_outer_payload_comparison_bytes = stats.outer_payload_comparison_bytes();
    exact.max_outer_payload_comparison_integer_bits = stats.outer_payload_comparison_integer_bits();
    exact
}

#[test]
fn generated_001_exact_group_limits_and_selection_one_below() {
    let (family, context, _, matcher) = generated_001_matcher();
    let baseline = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
        &family,
        &context,
        matcher.clone(),
        GeneratedResidualAffineGroupEffectiveCoverageLimits::default(),
    )
    .unwrap();
    let exact = exact_group_limits(&baseline);
    let rebuilt = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
        &family,
        &context,
        matcher.clone(),
        exact,
    )
    .unwrap();
    rebuilt.replay(&family, &context).unwrap();

    let mut one_below = exact;
    one_below.max_selection_target_references_inspected -= 1;
    assert!(matches!(
        GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
            &family,
            &context,
            matcher.clone(),
            one_below,
        ),
        Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
                resource: "group effective selection target references",
                ..
            }
        )
    ));

    assert!(exact.max_outer_retained_bytes > 0);
    let mut one_below = exact;
    one_below.max_outer_retained_bytes -= 1;
    assert!(matches!(
        GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
            &family,
            &context,
            matcher.clone(),
            one_below,
        ),
        Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
                resource: "group effective outer retained bytes",
                ..
            }
        )
    ));

    assert!(exact.max_outer_payload_comparison_units > 0);
    let mut one_below = exact;
    one_below.max_outer_payload_comparison_units -= 1;
    assert!(matches!(
        GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
            &family,
            &context,
            matcher.clone(),
            one_below,
        ),
        Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
                resource: "group effective outer payload comparison units",
                ..
            }
        )
    ));

    assert!(exact.max_child_source_terms > 0);
    let mut one_below = exact;
    one_below.max_child_source_terms -= 1;
    let error = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
        &family, &context, matcher, one_below,
    )
    .expect_err("one-below child source-term capacity must fail");
    match error {
        GeneratedResidualAffineGroupEffectiveCoverageError::Local(error) => assert!(
            error.to_string().contains("source terms"),
            "unexpected child source-term error: {error}"
        ),
        GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit { resource, .. } => {
            assert_eq!(resource, "group effective child source terms");
        }
        other => panic!("unexpected child source-term error: {other}"),
    }
}
