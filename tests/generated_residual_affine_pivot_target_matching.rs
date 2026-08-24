//! Concrete sunset validation for the generic pre-`WhenBad` affine target
//! matcher. No recurrence is supplied: every pivot comes from generated
//! IBP/LI rows and exact branch-bound re-elimination.

use std::{mem::size_of, sync::Arc};

use rustred::{
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
    GeneratedResidualAffinePivotTargetMatchingError,
    GeneratedResidualAffinePivotTargetMatchingLimits, GeneratedResidualAffinePivotTargetOutcome,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator,
    ParametricRelationError, SectorMask,
};
use symbolica::domains::integer::Integer;

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

fn inventory(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
) {
    let family = equal_mass_sunset(&format!("affine-pivot-target-sunset-{bits}"));
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
    (family, context, inventory)
}

fn first_generated_matcher(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
) {
    let (family, context, inventory) = inventory(bits);
    let mut ordinals = inventory
        .groups()
        .iter()
        .filter(|group| group.case_ordinals().len() > 1)
        .flat_map(|group| group.case_ordinals().iter().copied())
        .collect::<Vec<_>>();
    for case in inventory.cases() {
        if !ordinals.contains(&case.ordinal()) {
            ordinals.push(case.ordinal());
        }
    }
    for source_case_ordinal in ordinals {
        let Some(matcher) =
            generated_matcher_for_case(&family, &context, &inventory, source_case_ordinal)
        else {
            continue;
        };
        return (family, context, inventory, matcher);
    }
    panic!("sunset {bits} did not produce an eliminated affine branch");
}

fn generated_matcher_for_case(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    source_case_ordinal: usize,
) -> Option<GeneratedResidualAffinePivotTargetMatchingCertificate> {
    let case = &inventory.cases()[source_case_ordinal];
    let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
        family,
        context,
        case.source_cover().clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        case.source_branch().clone(),
        AffineParametricOrderingLimits::default(),
    )
    .unwrap();
    let schedule = Arc::new(
        AffinePreparePointScheduleCertificate::compile_with_authority(
            AffineStartReplayAuthority::ResidualBooleanBranch {
                family,
                context,
                cover: case.source_cover(),
            },
            ordering,
            0,
            AffinePreparePointScheduleLimits::default(),
        )
        .unwrap(),
    );
    let compilation = GeneratedResidualAffineBranchReeliminationCompiler::compile(
        family,
        context,
        schedule,
        case.guard_composition().clone(),
        GeneratedResidualAffineBranchReeliminationLimits::default(),
    )
    .unwrap();
    let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(reelimination) =
        compilation
    else {
        return None;
    };
    if reelimination.pivot_count() == 0 {
        return None;
    }
    Some(
        GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
            family,
            context,
            inventory.clone(),
            source_case_ordinal,
            Arc::new(reelimination),
            GeneratedResidualAffinePivotTargetMatchingLimits::default(),
        )
        .unwrap(),
    )
}

fn generated_pending_matcher() -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
) {
    // The completely generated 001 sunset sector has one authenticated case;
    // its first eliminated branch exercises the full successful recentering
    // path, including translated guards and retained relation manifests.
    let (family, context, inventory) = inventory("001");
    let matcher = generated_matcher_for_case(&family, &context, &inventory, 0)
        .expect("001 sunset case 0 must produce an eliminated affine branch");
    assert!(matcher.stats().pending_when_bad() > 0);
    (family, context, inventory, matcher)
}

fn matcher_resource_limit(
    error: GeneratedResidualAffinePivotTargetMatchingError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | GeneratedResidualAffinePivotTargetMatchingError::Relation(
            ParametricRelationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        other => panic!("expected a typed resource limit, got {other:?}"),
    }
}

#[test]
fn generated_sunset_matcher_replays_complete_group_scan_without_publishing_rules() {
    for bits in ["011", "101"] {
        let (family, context, inventory, matcher) = first_generated_matcher(bits);
        matcher.replay(&family, &context).unwrap();
        assert_eq!(matcher.outcomes().len(), matcher.stats().pivots());
        assert_eq!(matcher.stats().targets_consumed(), 0);
        let group = &inventory.groups()[matcher.source_group_ordinal()];
        let source_case = &inventory.cases()[matcher.source_case_ordinal()];
        // Case ordinals are the inventory's global authenticated priority.
        // Derive the group's full ordered target list by filtering that global
        // sequence, independently of the group's retained case_ordinals Vec.
        let independently_ordered_targets = inventory
            .cases()
            .iter()
            .filter(|case| case.group_ordinal() == matcher.source_group_ordinal())
            .map(|case| case.ordinal())
            .collect::<Vec<_>>();
        assert!(
            independently_ordered_targets.len() > 1,
            "sunset {bits} must exercise a real multi-case affine group"
        );
        assert_eq!(independently_ordered_targets, group.case_ordinals());
        let independently_transformed = |pivot: &rustred::IndexShift| {
            (0..group.ambient_arity())
                .map(|row| {
                    let mut value = source_case.constants()[row].clone();
                    for (free_ordinal, &free_position) in group.free_positions().iter().enumerate()
                    {
                        let coefficient = &group.compact_linear_coefficients()
                            [row * group.free_positions().len() + free_ordinal];
                        value -= coefficient * Integer::from(pivot.values()[free_position]);
                    }
                    value + Integer::from(pivot.values()[row])
                })
                .collect::<Vec<_>>()
        };
        for outcome in matcher.outcomes() {
            match outcome {
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(rejected) => {
                    let expected_transformed = independently_transformed(rejected.pivot());
                    let expected_matches = independently_ordered_targets
                        .iter()
                        .copied()
                        .filter(|&ordinal| {
                            inventory.cases()[ordinal].constants()
                                == expected_transformed.as_slice()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        rejected.transformed_target_constants(),
                        expected_transformed
                    );
                    assert_eq!(
                        rejected.checked_target_case_ordinals(),
                        independently_ordered_targets
                    );
                    assert!(expected_matches.is_empty());
                }
                GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(
                    rejected,
                ) => {
                    let expected_transformed = independently_transformed(rejected.pivot());
                    let expected_matches = independently_ordered_targets
                        .iter()
                        .copied()
                        .filter(|&ordinal| {
                            inventory.cases()[ordinal].constants()
                                == expected_transformed.as_slice()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        rejected.transformed_target_constants(),
                        expected_transformed
                    );
                    assert_eq!(
                        rejected.checked_target_case_ordinals(),
                        independently_ordered_targets
                    );
                    assert_eq!(rejected.matching_target_case_ordinals(), expected_matches);
                    assert!(!rejected.matching_target_case_ordinals().is_empty());
                }
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) => {
                    let expected_transformed = independently_transformed(pending.pivot());
                    let expected_matches = independently_ordered_targets
                        .iter()
                        .copied()
                        .filter(|&ordinal| {
                            inventory.cases()[ordinal].constants()
                                == expected_transformed.as_slice()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(pending.transformed_target_constants(), expected_transformed);
                    assert_eq!(
                        pending.checked_target_case_ordinals(),
                        independently_ordered_targets
                    );
                    assert_eq!(pending.matching_target_case_ordinals(), expected_matches);
                    assert!(!pending.matching_target_case_ordinals().is_empty());
                    assert!(!pending.is_applicable_rule());
                    assert!(pending.target_remains_available_if_when_bad_is_true());
                    for &target in pending.matching_target_case_ordinals() {
                        assert_eq!(
                            pending.transformed_target_constants(),
                            inventory.cases()[target].constants()
                        );
                    }
                    for (position, &component) in pending
                        .coefficient_translation()
                        .values()
                        .iter()
                        .enumerate()
                    {
                        if group.free_positions().contains(&position) {
                            assert_eq!(
                                component,
                                pending.pivot().values()[position].checked_neg().unwrap()
                            );
                        } else {
                            assert_eq!(component, 0);
                        }
                    }
                    assert_eq!(pending.key_center(), pending.pivot());
                }
            }
        }
        assert_eq!(
            matcher.stats().target_checks(),
            matcher.stats().pivots() * independently_ordered_targets.len()
        );
    }
}

#[test]
fn exact_geometry_and_payload_limits_replay_and_one_below_rejects() {
    let (family, context, inventory, matcher) = generated_pending_matcher();
    let stats = matcher.stats();
    let mut exact = matcher.limits();
    exact.max_family_fingerprint_bytes = stats.family_fingerprint_bytes();
    exact.max_context_fingerprint_bytes = stats.context_fingerprint_bytes();
    exact.max_scope_fingerprint_comparison_bytes = stats.scope_fingerprint_comparison_bytes();
    exact.max_pivots = stats.pivots();
    exact.max_ambient_arity = stats.ambient_arity();
    exact.max_free_positions = stats.free_positions();
    exact.max_group_cases = stats.group_cases();
    exact.max_geometry_comparison_entries = stats.geometry_comparison_entries();
    exact.max_geometry_comparison_integer_bit_work = stats.geometry_comparison_integer_bit_work();
    exact.max_target_checks = stats.target_checks();
    exact.max_checked_target_ordinals = stats.checked_target_ordinals();
    exact.max_matching_target_ordinals = stats.matching_target_ordinals();
    exact.max_matching_flag_bytes = stats.maximum_matching_flag_bytes();
    exact.max_affine_operations = stats.affine_operations();
    exact.max_affine_integer_bit_work = stats.affine_integer_bit_work();
    exact.max_affine_integer_bits = stats.maximum_affine_integer_bits();
    exact.max_target_comparison_entries = stats.target_comparison_entries();
    exact.max_target_comparison_integer_bit_work = stats.target_comparison_integer_bit_work();
    exact.max_transformed_constant_entries = stats.transformed_constant_entries();
    exact.max_retained_integer_bits = stats.retained_integer_bits();
    exact.max_retained_shift_components = stats.retained_shift_components();
    exact.max_row_label_bytes = stats.row_label_bytes();
    exact.max_recenter_attempts = stats.recenter_attempts();
    exact.max_recenter_terms = stats.recenter_terms();
    exact.max_recenter_guards = stats.recenter_guards();
    exact.max_recenter_translation_components = stats.recenter_translation_components();
    exact.max_recenter_key_subtraction_boundary_checks =
        stats.recenter_key_subtraction_boundary_checks();
    exact.max_recenter_source_terms = stats.recenter_source_terms();
    exact.max_recenter_source_exponent_entries = stats.recenter_source_exponent_entries();
    exact.max_recenter_output_terms = stats.recenter_output_terms();
    exact.max_recenter_output_exponent_entries = stats.recenter_output_exponent_entries();
    exact.max_recenter_power_operations = stats.recenter_power_operations();
    exact.max_recenter_integer_bit_work = stats.recenter_integer_bit_work();
    exact.max_recenter_normalized_coefficient_terms = stats.recenter_normalized_coefficient_terms();
    exact.max_recenter_retained_bytes = stats.recenter_retained_bytes();
    exact.max_retained_payload_bytes = stats.retained_payload_bytes();
    exact.max_payload_comparison_units = stats.payload_comparison_units();
    exact.max_payload_comparison_bytes = stats.payload_comparison_bytes();
    exact.max_payload_comparison_integer_bits = stats.payload_comparison_integer_bits();
    exact.max_payload_comparison_relation_manifest_bytes =
        stats.payload_comparison_relation_manifest_bytes();
    let exactly_bounded = GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
        &family,
        &context,
        inventory,
        matcher.source_case_ordinal(),
        matcher.reelimination().clone(),
        exact,
    )
    .unwrap();
    exactly_bounded.replay(&family, &context).unwrap();
    assert_eq!(exactly_bounded.stats(), stats);

    struct OneBelowCase {
        name: &'static str,
        observed: usize,
        set: fn(&mut GeneratedResidualAffinePivotTargetMatchingLimits, usize),
        accepted_resources: &'static [&'static str],
    }
    let one_below_cases = [
        OneBelowCase {
            name: "family fingerprint bytes",
            observed: stats.family_fingerprint_bytes(),
            set: |limits, value| limits.max_family_fingerprint_bytes = value,
            accepted_resources: &["affine target family fingerprint bytes"],
        },
        OneBelowCase {
            name: "context fingerprint bytes",
            observed: stats.context_fingerprint_bytes(),
            set: |limits, value| limits.max_context_fingerprint_bytes = value,
            accepted_resources: &["affine target context fingerprint bytes"],
        },
        OneBelowCase {
            name: "scope fingerprint comparison bytes",
            observed: stats.scope_fingerprint_comparison_bytes(),
            set: |limits, value| limits.max_scope_fingerprint_comparison_bytes = value,
            accepted_resources: &["affine target scope fingerprint comparison bytes"],
        },
        OneBelowCase {
            name: "pivots",
            observed: stats.pivots(),
            set: |limits, value| limits.max_pivots = value,
            accepted_resources: &["affine target pivots"],
        },
        OneBelowCase {
            name: "ambient arity",
            observed: stats.ambient_arity(),
            set: |limits, value| limits.max_ambient_arity = value,
            accepted_resources: &["affine target ambient arity"],
        },
        OneBelowCase {
            name: "free positions",
            observed: stats.free_positions(),
            set: |limits, value| limits.max_free_positions = value,
            accepted_resources: &["affine target free positions"],
        },
        OneBelowCase {
            name: "group cases",
            observed: stats.group_cases(),
            set: |limits, value| limits.max_group_cases = value,
            accepted_resources: &["affine target group cases"],
        },
        OneBelowCase {
            name: "geometry comparison entries",
            observed: stats.geometry_comparison_entries(),
            set: |limits, value| limits.max_geometry_comparison_entries = value,
            accepted_resources: &["affine target geometry comparison entries"],
        },
        OneBelowCase {
            name: "geometry comparison integer-bit work",
            observed: stats.geometry_comparison_integer_bit_work(),
            set: |limits, value| limits.max_geometry_comparison_integer_bit_work = value,
            accepted_resources: &["affine target geometry comparison integer-bit work"],
        },
        OneBelowCase {
            name: "target checks",
            observed: stats.target_checks(),
            set: |limits, value| limits.max_target_checks = value,
            accepted_resources: &["affine target checks"],
        },
        OneBelowCase {
            name: "checked target ordinals",
            observed: stats.checked_target_ordinals(),
            set: |limits, value| limits.max_checked_target_ordinals = value,
            accepted_resources: &["affine target checked target ordinals"],
        },
        OneBelowCase {
            name: "matching target ordinals",
            observed: stats.matching_target_ordinals(),
            set: |limits, value| limits.max_matching_target_ordinals = value,
            accepted_resources: &["affine target matching target ordinals"],
        },
        OneBelowCase {
            name: "matching flag bytes",
            observed: stats.maximum_matching_flag_bytes(),
            set: |limits, value| limits.max_matching_flag_bytes = value,
            accepted_resources: &["affine target matching flags"],
        },
        OneBelowCase {
            name: "affine operations",
            observed: stats.affine_operations(),
            set: |limits, value| limits.max_affine_operations = value,
            accepted_resources: &["affine target affine operations"],
        },
        OneBelowCase {
            name: "affine integer-bit work",
            observed: stats.affine_integer_bit_work(),
            set: |limits, value| limits.max_affine_integer_bit_work = value,
            accepted_resources: &["affine target affine integer-bit work"],
        },
        OneBelowCase {
            name: "maximum affine integer bits",
            observed: stats.maximum_affine_integer_bits(),
            set: |limits, value| limits.max_affine_integer_bits = value,
            accepted_resources: &["affine target transformed integer bits"],
        },
        OneBelowCase {
            name: "target comparison entries",
            observed: stats.target_comparison_entries(),
            set: |limits, value| limits.max_target_comparison_entries = value,
            accepted_resources: &["affine target comparison entries"],
        },
        OneBelowCase {
            name: "target comparison integer-bit work",
            observed: stats.target_comparison_integer_bit_work(),
            set: |limits, value| limits.max_target_comparison_integer_bit_work = value,
            accepted_resources: &["affine target comparison integer-bit work"],
        },
        OneBelowCase {
            name: "transformed constant entries",
            observed: stats.transformed_constant_entries(),
            set: |limits, value| limits.max_transformed_constant_entries = value,
            accepted_resources: &["affine target transformed constant entries"],
        },
        OneBelowCase {
            name: "retained integer bits",
            observed: stats.retained_integer_bits(),
            set: |limits, value| limits.max_retained_integer_bits = value,
            accepted_resources: &["affine target retained integer bits"],
        },
        OneBelowCase {
            name: "retained shift components",
            observed: stats.retained_shift_components(),
            set: |limits, value| limits.max_retained_shift_components = value,
            accepted_resources: &["affine target retained shift components"],
        },
        OneBelowCase {
            name: "row label bytes",
            observed: stats.row_label_bytes(),
            set: |limits, value| limits.max_row_label_bytes = value,
            accepted_resources: &["affine target row label bytes"],
        },
        OneBelowCase {
            name: "recenter attempts",
            observed: stats.recenter_attempts(),
            set: |limits, value| limits.max_recenter_attempts = value,
            accepted_resources: &["affine target recenter attempts"],
        },
        OneBelowCase {
            name: "recenter terms",
            observed: stats.recenter_terms(),
            set: |limits, value| limits.max_recenter_terms = value,
            accepted_resources: &["affine free recentering terms"],
        },
        OneBelowCase {
            name: "recenter guards",
            observed: stats.recenter_guards(),
            set: |limits, value| limits.max_recenter_guards = value,
            accepted_resources: &["affine free recentering guards"],
        },
        OneBelowCase {
            name: "recenter translation components",
            observed: stats.recenter_translation_components(),
            set: |limits, value| limits.max_recenter_translation_components = value,
            accepted_resources: &["affine free recentering translation components"],
        },
        OneBelowCase {
            name: "recenter key-subtraction boundary checks",
            observed: stats.recenter_key_subtraction_boundary_checks(),
            set: |limits, value| limits.max_recenter_key_subtraction_boundary_checks = value,
            accepted_resources: &[
                "affine target recentered key-subtraction boundary checks",
                "affine free recentering key-subtraction boundary checks",
            ],
        },
        OneBelowCase {
            name: "recenter source terms",
            observed: stats.recenter_source_terms(),
            set: |limits, value| limits.max_recenter_source_terms = value,
            accepted_resources: &["affine free recentering source terms"],
        },
        OneBelowCase {
            name: "recenter source exponent entries",
            observed: stats.recenter_source_exponent_entries(),
            set: |limits, value| limits.max_recenter_source_exponent_entries = value,
            accepted_resources: &["affine free recentering source exponent entries"],
        },
        OneBelowCase {
            name: "recenter output terms",
            observed: stats.recenter_output_terms(),
            set: |limits, value| limits.max_recenter_output_terms = value,
            accepted_resources: &["affine free recentering output terms"],
        },
        OneBelowCase {
            name: "recenter output exponent entries",
            observed: stats.recenter_output_exponent_entries(),
            set: |limits, value| limits.max_recenter_output_exponent_entries = value,
            accepted_resources: &["affine free recentering output exponent entries"],
        },
        OneBelowCase {
            name: "recenter power operations",
            observed: stats.recenter_power_operations(),
            set: |limits, value| limits.max_recenter_power_operations = value,
            accepted_resources: &["affine free recentering power operations"],
        },
        OneBelowCase {
            name: "recenter integer-bit work",
            observed: stats.recenter_integer_bit_work(),
            set: |limits, value| limits.max_recenter_integer_bit_work = value,
            accepted_resources: &["affine free recentering integer-bit work"],
        },
        OneBelowCase {
            name: "recenter normalized coefficient terms",
            observed: stats.recenter_normalized_coefficient_terms(),
            set: |limits, value| limits.max_recenter_normalized_coefficient_terms = value,
            accepted_resources: &["affine free recentering normalized coefficient terms"],
        },
        OneBelowCase {
            name: "recenter retained bytes",
            observed: stats.recenter_retained_bytes(),
            set: |limits, value| limits.max_recenter_retained_bytes = value,
            accepted_resources: &["affine free recentering retained bytes"],
        },
        OneBelowCase {
            name: "retained payload bytes",
            observed: stats.retained_payload_bytes(),
            set: |limits, value| limits.max_retained_payload_bytes = value,
            accepted_resources: &[
                "affine target retained payload bytes",
                "affine free recentering retained bytes",
            ],
        },
        OneBelowCase {
            name: "payload comparison units",
            observed: stats.payload_comparison_units(),
            set: |limits, value| limits.max_payload_comparison_units = value,
            accepted_resources: &["affine target payload comparison units"],
        },
        OneBelowCase {
            name: "payload comparison bytes",
            observed: stats.payload_comparison_bytes(),
            set: |limits, value| limits.max_payload_comparison_bytes = value,
            accepted_resources: &["affine target payload comparison bytes"],
        },
        OneBelowCase {
            name: "payload comparison integer bits",
            observed: stats.payload_comparison_integer_bits(),
            set: |limits, value| limits.max_payload_comparison_integer_bits = value,
            accepted_resources: &["affine target payload comparison integer bits"],
        },
        OneBelowCase {
            name: "payload comparison relation manifest bytes",
            observed: stats.payload_comparison_relation_manifest_bytes(),
            set: |limits, value| limits.max_payload_comparison_relation_manifest_bytes = value,
            accepted_resources: &[
                "affine target payload comparison relation manifest bytes",
                "parametric relation manifest bytes",
            ],
        },
    ];
    for case in one_below_cases {
        assert!(case.observed > 0, "{} fixture must be nonzero", case.name);
        let mut limited = exact;
        (case.set)(&mut limited, case.observed - 1);
        let error = GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
            &family,
            &context,
            exactly_bounded.inventory().clone(),
            exactly_bounded.source_case_ordinal(),
            exactly_bounded.reelimination().clone(),
            limited,
        )
        .unwrap_err();
        let (resource, requested, limit) = matcher_resource_limit(error);
        assert!(
            case.accepted_resources.contains(&resource),
            "{} returned unexpected resource {resource:?}",
            case.name
        );
        assert!(
            requested > limit,
            "{} did not exceed its admitted limit: requested={requested}, limit={limit}",
            case.name
        );
    }

    let certificate_debug = format!("{exactly_bounded:?}");
    for forbidden in [
        "ParametricRelation",
        "ParametricCoefficient",
        "ParametricPolynomial",
        "generated-residual-affine-pending-when-bad:",
        "family_fingerprint: \"",
        "context_fingerprint: \"",
        "row_id: ",
        "raw:",
    ] {
        assert!(
            !certificate_debug.contains(forbidden),
            "certificate Debug leaked private marker {forbidden:?}: {certificate_debug}"
        );
    }
    let mut pending_outcomes = 0usize;
    for outcome in exactly_bounded.outcomes() {
        let outcome_debug = format!("{outcome:?}");
        for forbidden in [
            "ParametricRelation",
            "ParametricCoefficient",
            "ParametricPolynomial",
            "generated-residual-affine-pending-when-bad:",
            "family_fingerprint: \"",
            "context_fingerprint: \"",
            "row_id: ",
            "raw:",
        ] {
            assert!(
                !outcome_debug.contains(forbidden),
                "outcome Debug leaked private marker {forbidden:?}: {outcome_debug}"
            );
        }
        if let GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) = outcome {
            pending_outcomes += 1;
            assert!(
                pending.recentered_guard_count() > 0,
                "the 001 fixture must exercise translated guards in every private pending row"
            );
            assert!(
                pending.recentered_owned_retained_byte_bound().unwrap()
                    <= pending.recentered_retained_byte_envelope()
            );
        }
    }
    assert_eq!(pending_outcomes, stats.pending_when_bad());

    let fixed_outcome_buffer_lower_bound =
        size_of::<GeneratedResidualAffinePivotTargetMatchingCertificate>()
            + stats.pivots() * size_of::<GeneratedResidualAffinePivotTargetOutcome>();
    assert!(fixed_outcome_buffer_lower_bound <= stats.retained_payload_bytes());
    let mut buffer_one_below = exact;
    buffer_one_below.max_retained_payload_bytes = fixed_outcome_buffer_lower_bound - 1;
    assert!(matches!(
        GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
            &family,
            &context,
            exactly_bounded.inventory().clone(),
            exactly_bounded.source_case_ordinal(),
            exactly_bounded.reelimination().clone(),
            buffer_one_below,
        ),
        Err(rustred::GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit {
            resource: "affine target retained payload bytes",
            requested,
            limit,
        }) if requested == fixed_outcome_buffer_lower_bound && limit + 1 == requested
    ));

    // Operand byte limits are authenticated before equality, including when
    // the mismatching operand itself is oversized.
    let wrong_family = equal_mass_sunset(&"oversized-family-".repeat(128));
    assert!(wrong_family.fingerprint_ref().len() > stats.family_fingerprint_bytes());
    let family_error = GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
        &wrong_family,
        &context,
        exactly_bounded.inventory().clone(),
        exactly_bounded.source_case_ordinal(),
        exactly_bounded.reelimination().clone(),
        exact,
    )
    .unwrap_err();
    assert_eq!(
        matcher_resource_limit(family_error).0,
        "affine target family fingerprint bytes"
    );
    assert_eq!(
        matcher_resource_limit(exactly_bounded.replay(&wrong_family, &context).unwrap_err()).0,
        "affine target family fingerprint bytes"
    );

    let wrong_context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        &"oversized-context-".repeat(128),
        context.index_count(),
    )
    .unwrap();
    assert!(wrong_context.fingerprint().len() > stats.context_fingerprint_bytes());
    let context_error = GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
        &family,
        &wrong_context,
        exactly_bounded.inventory().clone(),
        exactly_bounded.source_case_ordinal(),
        exactly_bounded.reelimination().clone(),
        exact,
    )
    .unwrap_err();
    assert_eq!(
        matcher_resource_limit(context_error).0,
        "affine target context fingerprint bytes"
    );
    assert_eq!(
        matcher_resource_limit(exactly_bounded.replay(&family, &wrong_context).unwrap_err()).0,
        "affine target context fingerprint bytes"
    );
}

#[test]
fn transformed_formula_distinguishes_same_adjacent_and_absent_targets() {
    // This direct arithmetic oracle is independent of generated elimination;
    // it validates the exact b' formula for the canonical dependent start
    // F(t)=(3-t,t) and does not encode any recurrence coefficient.
    let b = [Integer::from(3), Integer::from(0)];
    let a = [Integer::from(-1), Integer::from(1)];
    let transform = |pivot: [i64; 2]| {
        vec![
            b[0].clone() - &a[0] * Integer::from(pivot[1]) + Integer::from(pivot[0]),
            b[1].clone() - &a[1] * Integer::from(pivot[1]) + Integer::from(pivot[1]),
        ]
    };
    assert_eq!(transform([1, -1]), vec![Integer::from(3), Integer::from(0)]);
    assert_eq!(transform([1, 0]), vec![Integer::from(4), Integer::from(0)]);
    assert_eq!(transform([7, 0]), vec![Integer::from(10), Integer::from(0)]);
}
