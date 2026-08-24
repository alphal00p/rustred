use std::sync::Arc;

use crate::generated_residual_affine_group_effective_coverage::{
    GeneratedResidualAffineGroupTargetDisposition, GeneratedResidualAffineResidualWorkKind,
    GeneratedResidualAffineTargetAttemptOutcome,
};
use crate::generated_residual_affine_when_bad_compilation::{
    GeneratedResidualAffineSealedApplicationLimits, GeneratedResidualAffineSealedApplicationStats,
    GeneratedResidualAffineWhenBadApplicationError, GeneratedResidualAffineWhenBadCompilation,
    GeneratedResidualAffineWhenBadPointError, GeneratedResidualAffineWhenBadPointLimits,
    GeneratedResidualAffineWhenBadPointStats,
};
use crate::generated_sector_affine_effective_coverage::{
    GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA,
    GeneratedSectorAffineConcretePointOutcome, GeneratedSectorAffineEffectiveCoverageCertificate,
    GeneratedSectorAffineEffectiveCoverageCompiler, GeneratedSectorAffineEffectiveCoverageConfig,
    GeneratedSectorAffineEffectiveCoverageError, GeneratedSectorAffineEffectiveCoverageLimits,
    GeneratedSectorAffineExceptionalChildLocator, GeneratedSectorAffineGroupPassOutcome,
    GeneratedSectorAffineOrderedChildOutput, GeneratedSectorAffinePointDisposition,
    GeneratedSectorAffinePointError, GeneratedSectorAffinePointLimits,
    GeneratedSectorAffinePointSpecializationLimits, GeneratedSectorAffinePointSpecializationStats,
    GeneratedSectorAffinePointStats, GeneratedSectorAffineResidualRootLocator,
    GeneratedSectorAffineRuleApplicationError, GeneratedSectorAffineRuleApplicationLimits,
    GeneratedSectorAffineRuleApplicationStats, GeneratedSectorAffineRuleLocator,
    GeneratedSectorAffineTerminalDisposition,
};
use crate::generated_sector_affine_provider::GeneratedSectorAffineConditionalRuleProviderError;
use crate::parametric_relation::ParametricConcreteSpecializationLimits;
use crate::residual_affine_integer_system::{
    ResidualAffineIntegerMapPointError, ResidualAffineIntegerMapPointLimits,
    ResidualAffineIntegerMapPointStats,
};
use crate::{
    AffineDenominator, AffineParametricOrderingError, AffinePreparePointScheduleError,
    AffineWhenBadRelativeLeafDisposition, CoefficientContext, ConcreteIntegralKey,
    ConditionalConcreteReduction, ConditionalParametricRuleError,
    GeneratedResidualAffineBranchReeliminationError,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryCompiler,
    GeneratedResidualAffineCaseInventoryError, GeneratedResidualAffineCaseInventoryLimits,
    GeneratedResidualAffineInventoryTerminalOutcome,
    GeneratedResidualAffinePivotTargetMatchingError, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, GeneratedSectorQueuedSourceDisposition, GuardOrigin,
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator,
    ParametricSectorLeafDisposition, SectorMask,
};

#[test]
fn sector_affine_effective_coverage_schema_is_stable() {
    assert_eq!(
        GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA,
        "rustred-generated-sector-affine-effective-coverage-v1"
    );
}

#[test]
fn sector_affine_owner_errors_are_redacted_and_preserve_typed_sources() {
    const PRIVATE_SENTINEL: &str =
        "ParametricRelation polynomial pullback private_predicate split_recentered_relation";
    const FORBIDDEN: [&str; 5] = [
        "ParametricRelation",
        "polynomial",
        "pullback",
        "private_predicate",
        "split_recentered_relation",
    ];

    let direct = [
        GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
            detail: PRIVATE_SENTINEL,
        },
        GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource: PRIVATE_SENTINEL,
            requested: 2,
            limit: 1,
        },
    ];
    for error in direct {
        let rendered = format!("{error} {error:?}");
        for forbidden in FORBIDDEN {
            assert!(
                !rendered.contains(forbidden),
                "leaked {forbidden}: {rendered}"
            );
        }
    }

    let owner_error = GeneratedSectorAffineEffectiveCoverageError::Inventory(
        GeneratedResidualAffineCaseInventoryError::MalformedGrouping {
            detail: PRIVATE_SENTINEL,
        },
    );
    let rendered = format!("{owner_error} {owner_error:?}");
    for forbidden in FORBIDDEN {
        assert!(
            !rendered.contains(forbidden),
            "leaked {forbidden}: {rendered}"
        );
    }
    assert!(
        std::error::Error::source(&owner_error)
            .and_then(|source| {
                source.downcast_ref::<GeneratedResidualAffineCaseInventoryError>()
            })
            .is_some()
    );

    let provider_error =
        GeneratedSectorAffineConditionalRuleProviderError::<std::io::Error>::OwnerReplay {
            sector: SectorMask::try_from_bit_string("001").unwrap(),
            error: owner_error,
        };
    let rendered = format!("{provider_error} {provider_error:?}");
    for forbidden in FORBIDDEN {
        assert!(
            !rendered.contains(forbidden),
            "leaked {forbidden}: {rendered}"
        );
    }
    assert!(
        std::error::Error::source(&provider_error)
            .and_then(|source| {
                source.downcast_ref::<GeneratedSectorAffineEffectiveCoverageError>()
            })
            .is_some()
    );
}

fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
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

fn inventory_fixture(
    bits: &str,
    name: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedResidualAffineCaseInventoryCertificate>,
) {
    let family = equal_mass_two_loop_family(name);
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

fn owner_fixture(
    bits: &str,
    name: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    GeneratedSectorAffineEffectiveCoverageCertificate,
) {
    let (family, context, inventory) = inventory_fixture(bits, name);
    let owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory,
        GeneratedSectorAffineEffectiveCoverageConfig::new(0),
        GeneratedSectorAffineEffectiveCoverageLimits::default(),
    )
    .unwrap();
    (family, context, owner)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndependentOwnerPoint {
    disposition: GeneratedSectorAffinePointDisposition,
    inventory_terminal_ordinal: Option<usize>,
    child_output_ordinal: Option<usize>,
}

fn queued_source_matches_independent_global(
    queued: &GeneratedSectorQueuedSourceDisposition,
    global: &ParametricSectorLeafDisposition,
) -> bool {
    match (queued, global) {
        (
            GeneratedSectorQueuedSourceDisposition::Uncovered,
            ParametricSectorLeafDisposition::Uncovered,
        ) => true,
        (
            GeneratedSectorQueuedSourceDisposition::Unsupported {
                candidate_ordinals: queued,
            },
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals: global,
            },
        ) => queued.as_ref() == global.as_ref(),
        _ => false,
    }
}

/// Independent composition of the already-authenticated lower proof seams.
///
/// This intentionally does not call the owner point API.  It starts at the V1
/// global partition, resolves the exact source Boolean terminal, checks
/// `F(n) == n` through the retained integer-affine map, enters the sealed
/// target-relative classifier, and finally authenticates the flattened child
/// locator and its exact `Arc` provenance.
fn independent_owner_point(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
    indices: &[i64],
) -> IndependentOwnerPoint {
    let queue = owner.source_queue();
    let inventory = owner.inventory();
    let global = queue.discovery().coverage();
    let Some(global_classification) = global.classification_for_indices(context, indices).unwrap()
    else {
        return IndependentOwnerPoint {
            disposition: GeneratedSectorAffinePointDisposition::OutsideSector,
            inventory_terminal_ordinal: None,
            child_output_ordinal: None,
        };
    };

    match global_classification.disposition() {
        ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal } => {
            return IndependentOwnerPoint {
                disposition: GeneratedSectorAffinePointDisposition::CoveredByGlobal {
                    candidate_ordinal: *candidate_ordinal,
                },
                inventory_terminal_ordinal: None,
                child_output_ordinal: None,
            };
        }
        ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => {
            panic!("a concrete in-sector fixture point matched a V1 empty locus")
        }
        ParametricSectorLeafDisposition::Uncovered
        | ParametricSectorLeafDisposition::Unsupported { .. } => {}
    }

    let matching_items = queue
        .work_items()
        .iter()
        .filter(|item| item.source_case() == global_classification.case())
        .collect::<Vec<_>>();
    assert_eq!(matching_items.len(), 1, "the source work item is unique");
    let item = matching_items[0];
    assert!(queued_source_matches_independent_global(
        item.source_disposition(),
        global_classification.disposition(),
    ));

    let source_terminals = inventory
        .terminals()
        .iter()
        .filter(|terminal| terminal.locator().work_item_ordinal() == item.ordinal())
        .collect::<Vec<_>>();
    assert!(
        !source_terminals.is_empty(),
        "an integer source point must retain a Boolean cover"
    );
    let source_cover = source_terminals[0].source_cover();
    assert!(source_terminals.iter().all(|terminal| {
        terminal.locator().source_case() == global_classification.case()
            && Arc::ptr_eq(terminal.source_cover(), source_cover)
    }));
    let ready = source_cover
        .ready_terminal_for_indices(context, indices)
        .unwrap()
        .expect("an in-source integer point must enter one ready Boolean terminal");
    let matching_terminals = inventory
        .terminals()
        .iter()
        .enumerate()
        .filter(|(_, terminal)| {
            terminal.locator().work_item_ordinal() == item.ordinal()
                && terminal.locator().source_case() == global_classification.case()
                && terminal.locator().terminal_ordinal() == ready.ordinal()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_terminals.len(),
        1,
        "the ready Boolean terminal resolves uniquely"
    );
    let (inventory_terminal_ordinal, terminal) = matching_terminals[0];
    assert!(Arc::ptr_eq(terminal.source_cover(), source_cover));
    let record = owner
        .terminal_records()
        .get(inventory_terminal_ordinal)
        .expect("terminal records are inventory ordered");
    assert_eq!(
        record.inventory_terminal_ordinal(),
        inventory_terminal_ordinal
    );
    assert_eq!(record.source_locator(), terminal.locator());
    assert_eq!(record.source_outcome(), terminal.outcome());

    let residual = |locator| IndependentOwnerPoint {
        disposition: GeneratedSectorAffinePointDisposition::ResidualRoot(locator),
        inventory_terminal_ordinal: Some(inventory_terminal_ordinal),
        child_output_ordinal: None,
    };
    let case_ordinal = match terminal.outcome() {
        GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported => {
            let locator = GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                terminal_ordinal: inventory_terminal_ordinal,
            };
            assert_eq!(
                record.disposition(),
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
            );
            return residual(locator);
        }
        GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal } => {
            case_ordinal
        }
        GeneratedResidualAffineInventoryTerminalOutcome::SourceCoordinateLeafProvedEmpty
        | GeneratedResidualAffineInventoryTerminalOutcome::BooleanProvedEmpty
        | GeneratedResidualAffineInventoryTerminalOutcome::AffineProvedEmpty
        | GeneratedResidualAffineInventoryTerminalOutcome::GuardContradiction { .. } => {
            panic!("a concrete point resolved to a proved-empty inventory terminal")
        }
    };
    let case = &inventory.cases()[case_ordinal];
    assert_eq!(case.ordinal(), case_ordinal);
    assert_eq!(case.locator(), terminal.locator());
    let (fixed, _) = case
        .affine_map()
        .fixes_i64_point_with_limits(indices, ResidualAffineIntegerMapPointLimits::default())
        .unwrap();
    assert!(
        fixed,
        "the exact source affine map must fix its integer point"
    );

    let passes = owner
        .group_passes()
        .iter()
        .filter(|pass| pass.group_ordinal() == case.group_ordinal())
        .collect::<Vec<_>>();
    assert_eq!(passes.len(), 1, "the case geometry selects one group pass");
    let pass = passes[0];
    assert_eq!(pass.pass_ordinal(), case.group_ordinal());
    assert_eq!(pass.group_ordinal(), case.group_ordinal());
    let effective = match pass.outcome() {
        GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(no_rows) => {
            let group = &inventory.groups()[case.group_ordinal()];
            let anchor = &inventory.cases()[group.anchor_case_ordinal()];
            assert!(Arc::ptr_eq(no_rows.branch(), anchor.source_branch()));
            assert!(Arc::ptr_eq(
                no_rows.branch_guards(),
                anchor.guard_composition()
            ));
            let locator = GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase {
                case_ordinal,
            };
            assert_eq!(
                record.disposition(),
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
            );
            return residual(locator);
        }
        GeneratedSectorAffineGroupPassOutcome::Effective(effective) => effective,
    };
    assert!(Arc::ptr_eq(effective.matcher().inventory(), inventory));
    let targets = effective
        .target_dispositions()
        .iter()
        .filter(|target| target.target_case_ordinal() == case_ordinal)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 1, "the group target resolves uniquely");
    assert_eq!(targets[0].target_locator(), case.locator());
    let (accepted_attempt_ordinal, when_bad) = match targets[0].disposition() {
        GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. } => {
            let locator = GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                group_pass_ordinal: pass.pass_ordinal(),
                target_case_ordinal: case_ordinal,
            };
            assert_eq!(
                record.disposition(),
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator)
            );
            return residual(locator);
        }
        GeneratedResidualAffineGroupTargetDisposition::Consumed {
            accepted_attempt_ordinal,
            when_bad,
        } => (*accepted_attempt_ordinal, when_bad),
    };
    let (first_child_output_ordinal, child_output_count) = match record.disposition() {
        GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
            group_pass_ordinal,
            target_case_ordinal,
            first_child_output_ordinal,
            child_output_count,
        } => {
            assert_eq!(group_pass_ordinal, pass.pass_ordinal());
            assert_eq!(target_case_ordinal, case_ordinal);
            (first_child_output_ordinal, child_output_count)
        }
        GeneratedSectorAffineTerminalDisposition::ProvedEmpty
        | GeneratedSectorAffineTerminalDisposition::ResidualRoot(_) => {
            panic!("an independently consumed target must own a child partition")
        }
    };
    let attempts = effective
        .attempts()
        .iter()
        .filter(|attempt| attempt.attempt_ordinal() == accepted_attempt_ordinal)
        .collect::<Vec<_>>();
    assert_eq!(attempts.len(), 1, "the accepted attempt resolves uniquely");
    assert_eq!(
        attempts[0].selected_target_case_ordinal(),
        Some(case_ordinal)
    );
    let GeneratedResidualAffineTargetAttemptOutcome::Accepted(attempt_when_bad) =
        attempts[0].outcome()
    else {
        panic!("the consumed target must point at an accepted attempt");
    };
    assert!(Arc::ptr_eq(attempt_when_bad, when_bad));
    let GeneratedResidualAffineWhenBadCompilation::Certified(certified) = when_bad.as_ref() else {
        panic!("a consumed target must retain a certified private partition");
    };
    let relative = certified
        .classify_relative_point(
            context,
            indices,
            GeneratedResidualAffineWhenBadPointLimits::default(),
        )
        .unwrap();
    assert!(relative.leaf_ordinal() < child_output_count);
    let child_output_ordinal = first_child_output_ordinal
        .checked_add(relative.leaf_ordinal())
        .expect("small fixture child offset fits usize");
    let child = &owner.ordered_child_outputs()[child_output_ordinal];

    match relative.disposition() {
        AffineWhenBadRelativeLeafDisposition::Applicable => {
            let handles = effective
                .sealed_rules()
                .iter()
                .filter(|handle| {
                    handle.target_case_ordinal() == case_ordinal
                        && handle.target_locator() == case.locator()
                        && handle.accepted_attempt_ordinal() == accepted_attempt_ordinal
                        && handle.leaf_ordinal() == relative.leaf_ordinal()
                        && handle.relative_case() == relative.case()
                })
                .collect::<Vec<_>>();
            assert_eq!(handles.len(), 1, "the sealed rule handle resolves uniquely");
            assert!(Arc::ptr_eq(handles[0].when_bad(), when_bad));
            let expected_locator = GeneratedSectorAffineRuleLocator {
                group_pass_ordinal: pass.pass_ordinal(),
                accepted_attempt_ordinal,
                leaf_ordinal: relative.leaf_ordinal(),
            };
            assert_eq!(
                *child,
                GeneratedSectorAffineOrderedChildOutput::Rule(expected_locator)
            );
            IndependentOwnerPoint {
                disposition: GeneratedSectorAffinePointDisposition::Rule(expected_locator),
                inventory_terminal_ordinal: Some(inventory_terminal_ordinal),
                child_output_ordinal: Some(child_output_ordinal),
            }
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal } => {
            independent_exceptional_owner_point(
                effective,
                when_bad,
                case_ordinal,
                case.locator(),
                pass.pass_ordinal(),
                accepted_attempt_ordinal,
                relative.leaf_ordinal(),
                relative.case(),
                GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { condition_ordinal },
                child,
                inventory_terminal_ordinal,
                child_output_ordinal,
            )
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
            independent_exceptional_owner_point(
                effective,
                when_bad,
                case_ordinal,
                case.locator(),
                pass.pass_ordinal(),
                accepted_attempt_ordinal,
                relative.leaf_ordinal(),
                relative.case(),
                GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { pullback_ordinal },
                child,
                inventory_terminal_ordinal,
                child_output_ordinal,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn independent_exceptional_owner_point(
    effective: &crate::generated_residual_affine_group_effective_coverage::GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    when_bad: &Arc<GeneratedResidualAffineWhenBadCompilation>,
    case_ordinal: usize,
    target_locator: crate::GeneratedResidualAffineCaseLocator,
    group_pass_ordinal: usize,
    accepted_attempt_ordinal: usize,
    leaf_ordinal: usize,
    relative_case: crate::AffineWhenBadRelativeCaseId,
    expected_kind: GeneratedResidualAffineResidualWorkKind,
    child: &GeneratedSectorAffineOrderedChildOutput,
    inventory_terminal_ordinal: usize,
    child_output_ordinal: usize,
) -> IndependentOwnerPoint {
    let leaves = effective
        .residual_work()
        .iter()
        .filter(|leaf| {
            leaf.target_case_ordinal() == case_ordinal
                && leaf.target_locator() == target_locator
                && leaf.accepted_attempt_ordinal() == Some(accepted_attempt_ordinal)
                && leaf.leaf_ordinal() == Some(leaf_ordinal)
                && leaf.relative_case() == Some(relative_case)
                && leaf.kind() == expected_kind
        })
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), 1, "the exceptional child resolves uniquely");
    assert!(
        leaves[0]
            .when_bad()
            .is_some_and(|retained| Arc::ptr_eq(retained, when_bad))
    );
    let expected_locator = GeneratedSectorAffineExceptionalChildLocator {
        group_pass_ordinal,
        accepted_attempt_ordinal,
        leaf_ordinal,
    };
    assert_eq!(
        *child,
        GeneratedSectorAffineOrderedChildOutput::Exceptional(expected_locator)
    );
    IndependentOwnerPoint {
        disposition: GeneratedSectorAffinePointDisposition::Exceptional(expected_locator),
        inventory_terminal_ordinal: Some(inventory_terminal_ordinal),
        child_output_ordinal: Some(child_output_ordinal),
    }
}

fn exact_point_specialization_limits(
    stats: GeneratedSectorAffinePointSpecializationStats,
) -> GeneratedSectorAffinePointSpecializationLimits {
    GeneratedSectorAffinePointSpecializationLimits {
        max_source_terms: stats.source_terms(),
        max_source_exponent_entries: stats.source_exponent_entries(),
        max_preflight_validation_source_term_scan_bound: stats
            .preflight_validation_source_term_scan_bound(),
        max_preflight_validation_source_exponent_entry_scan_bound: stats
            .preflight_validation_source_exponent_entry_scan_bound(),
        max_output_term_bound: stats.output_term_bound(),
        max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
        max_power_operation_bound: stats.power_operation_bound(),
        max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
        max_integer_bit_work_bound: stats.integer_bit_work_bound(),
        max_retained_output_term_bound: stats.retained_output_term_bound(),
        max_retained_output_byte_bound: stats.retained_output_byte_bound(),
    }
}

fn exact_map_point_limits(
    stats: ResidualAffineIntegerMapPointStats,
) -> ResidualAffineIntegerMapPointLimits {
    ResidualAffineIntegerMapPointLimits {
        max_ambient_arity: stats.ambient_arity(),
        max_matrix_entries_inspected: stats.matrix_entries_inspected(),
        max_nonzero_multiplications: stats.nonzero_multiplications(),
        max_additions: stats.additions(),
        max_fixed_point_comparisons: stats.fixed_point_comparisons(),
        max_peak_temporary_bytes: stats.peak_temporary_bytes(),
        max_integer_bits: stats.largest_integer_bits(),
        max_integer_bit_work: stats.integer_bit_work(),
    }
}

fn exact_relative_point_limits(
    stats: GeneratedResidualAffineWhenBadPointStats,
) -> GeneratedResidualAffineWhenBadPointLimits {
    GeneratedResidualAffineWhenBadPointLimits {
        max_context_fingerprint_comparison_bytes: stats.context_fingerprint_comparison_bytes(),
        max_index_entries: stats.index_entries(),
        max_cases: stats.cases(),
        max_classifications: stats.classifications(),
        max_predicates: stats.predicates(),
        max_source_terms: stats.source_terms(),
        max_source_exponent_entries: stats.source_exponent_entries(),
        max_preflight_validation_source_term_scan_bound: stats
            .preflight_validation_source_term_scan_bound(),
        max_preflight_validation_source_exponent_entry_scan_bound: stats
            .preflight_validation_source_exponent_entry_scan_bound(),
        max_output_term_bound: stats.output_term_bound(),
        max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
        max_power_operation_bound: stats.power_operation_bound(),
        max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
        max_integer_bit_work_bound: stats.integer_bit_work_bound(),
        max_retained_output_term_bound: stats.retained_output_term_bound(),
        max_retained_output_byte_bound: stats.retained_output_byte_bound(),
    }
}

fn exact_owner_point_limits(
    stats: GeneratedSectorAffinePointStats,
) -> GeneratedSectorAffinePointLimits {
    GeneratedSectorAffinePointLimits {
        map: exact_map_point_limits(stats.map().unwrap_or_default()),
        relative: exact_relative_point_limits(stats.relative().unwrap_or_default()),
        global_specialization: exact_point_specialization_limits(stats.global_specialization()),
        boolean_specialization: exact_point_specialization_limits(stats.boolean_specialization()),
        max_family_fingerprint_comparison_bytes: stats.family_fingerprint_comparison_bytes(),
        max_context_fingerprint_comparison_bytes: stats.context_fingerprint_comparison_bytes(),
        max_index_entries: stats.index_entries(),
        max_global_cases: stats.global_cases(),
        max_global_classifications: stats.global_classifications(),
        max_global_predicates: stats.global_predicates(),
        max_work_items_scanned: stats.work_items_scanned(),
        max_inventory_terminal_scans: stats.inventory_terminal_scans(),
        max_boolean_nodes_scanned: stats.boolean_nodes_scanned(),
        max_boolean_ready_terminals: stats.boolean_ready_terminals(),
        max_boolean_predicates: stats.boolean_predicates(),
        max_owner_terminal_record_scans: stats.owner_terminal_record_scans(),
        max_inventory_case_lookups: stats.inventory_case_lookups(),
        max_group_pass_scans: stats.group_pass_scans(),
        max_group_case_references_scanned: stats.group_case_references_scanned(),
        max_target_disposition_scans: stats.target_disposition_scans(),
        max_attempt_scans: stats.attempt_scans(),
        max_child_output_lookups: stats.child_output_lookups(),
        max_sealed_rule_scans: stats.sealed_rule_scans(),
        max_residual_work_scans: stats.residual_work_scans(),
        max_child_offset_arithmetic: stats.child_offset_arithmetic(),
        max_child_offset_comparisons: stats.child_offset_comparisons(),
        max_child_authority_comparisons: stats.child_authority_comparisons(),
    }
}

fn exact_owner_rule_application_limits(
    stats: GeneratedSectorAffineRuleApplicationStats,
) -> GeneratedSectorAffineRuleApplicationLimits {
    GeneratedSectorAffineRuleApplicationLimits {
        point: exact_owner_point_limits(stats.point()),
        sealed: exact_sealed_application_limits(stats.sealed()),
        max_owner_replays: stats.owner_replays(),
        max_group_pass_lookups: stats.group_pass_lookups(),
        max_sealed_rule_scans: stats.sealed_rule_scans(),
        max_symbolic_rhs_terms: stats.symbolic_rhs_terms(),
        max_specialized_rhs_terms: stats.specialized_rhs_terms(),
        max_required_nonzero_conditions: stats.required_nonzero_conditions(),
        max_required_nonzero_origins: stats.required_nonzero_origins(),
        max_retained_authority_references: stats.retained_authority_references(),
        max_concrete_reduction_retained_byte_bound: stats.concrete_reduction_retained_byte_bound(),
        max_peak_visible_application_byte_bound: stats.peak_visible_application_byte_bound(),
    }
}

fn exact_sealed_application_limits(
    stats: GeneratedResidualAffineSealedApplicationStats,
) -> GeneratedResidualAffineSealedApplicationLimits {
    let relation_stats = stats.relation();
    let mut relation = ParametricConcreteSpecializationLimits::default();
    relation.max_source_terms = relation_stats.source_terms();
    relation.max_source_exponent_entries = relation_stats.source_exponent_entries();
    relation.max_output_term_bound = relation_stats.output_term_bound();
    relation.max_output_exponent_entry_bound = relation_stats.output_exponent_entry_bound();
    relation.max_power_operation_bound = relation_stats.power_operation_bound();
    relation.max_integer_bit_work_bound = relation_stats.integer_bit_work_bound();
    relation.max_normalization_input_term_pair_bound =
        relation_stats.normalization_input_term_pair_bound();
    relation.max_key_component_bound = relation_stats.key_component_bound();
    relation.max_guard_occurrence_bound = relation_stats.guard_occurrence_bound();
    relation.max_guard_polynomial_retained_byte_bound =
        relation_stats.guard_polynomial_retained_byte_bound();
    relation.max_guard_origin_occurrence_bound = relation_stats.guard_origin_occurrence_bound();
    relation.max_guard_origin_retained_byte_bound =
        relation_stats.guard_origin_retained_byte_bound();
    relation.max_normalized_coefficient_term_bound =
        relation_stats.normalized_coefficient_term_bound();
    relation.max_normalized_coefficient_retained_byte_bound =
        relation_stats.normalized_coefficient_retained_byte_bound();
    relation.max_concrete_relation_retained_byte_bound =
        relation_stats.concrete_relation_retained_byte_bound();
    relation.max_peak_execution_retained_byte_bound =
        relation_stats.peak_execution_retained_byte_bound();

    GeneratedResidualAffineSealedApplicationLimits {
        max_condition_rows: stats.condition_rows(),
        max_condition_source_lookups: stats.condition_source_lookups(),
        max_condition_copy_terms: stats.condition_copy_terms(),
        max_condition_copy_exponent_entries: stats.condition_copy_exponent_entries(),
        max_condition_copy_integer_bits: stats.condition_copy_integer_bits(),
        max_condition_origin_inputs: stats.condition_origin_inputs(),
        max_condition_origin_retained_bytes: stats.condition_origin_retained_bytes(),
        max_temporary_condition_retained_byte_bound: stats
            .temporary_condition_retained_byte_bound(),
        max_temporary_plus_relation_peak_byte_bound: stats
            .temporary_plus_relation_peak_byte_bound(),
        relation,
    }
}

fn owner_rule_application_resource_limit(
    error: GeneratedSectorAffineRuleApplicationError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedSectorAffineRuleApplicationError::ResourceLimit {
            resource,
            requested,
            limit,
        } => (resource, requested, limit),
        GeneratedSectorAffineRuleApplicationError::Concrete(
            ConditionalParametricRuleError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        GeneratedSectorAffineRuleApplicationError::Sealed(
            GeneratedResidualAffineWhenBadApplicationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        other => panic!("expected a rule-application resource limit, got {other:?}"),
    }
}

fn assert_concrete_reduction_coefficient(
    reduction: &ConditionalConcreteReduction,
    family: &IntegralFamily,
    powers: [i64; 3],
    expected: &str,
) {
    let key = ConcreteIntegralKey::try_new(powers).unwrap();
    let actual = reduction
        .rhs()
        .get(&key)
        .unwrap_or_else(|| panic!("missing expected RHS target {key:?}"));
    let expected = family.coefficient_context().parse(expected).unwrap();
    assert!(
        family
            .coefficient_context()
            .try_sub(actual, &expected, Default::default())
            .unwrap()
            .is_zero(),
        "unexpected coefficient for RHS target {key:?}: {actual:?}",
    );
}

fn owner_point_resource_limit(
    error: GeneratedSectorAffinePointError,
) -> (&'static str, usize, usize) {
    match error {
        GeneratedSectorAffinePointError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | GeneratedSectorAffinePointError::AffineMap(
            ResidualAffineIntegerMapPointError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
        | GeneratedSectorAffinePointError::RelativePoint(
            GeneratedResidualAffineWhenBadPointError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => (resource, requested, limit),
        other => panic!("expected a point-query resource limit, got {other:?}"),
    }
}

fn assert_exact_and_one_below_owner_point_limits(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    point: &[i64],
) {
    let baseline = owner
        .classification_for_indices(
            family,
            context,
            point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    let stats = baseline.stats();
    let exact = exact_owner_point_limits(stats);
    assert_eq!(
        owner
            .classification_for_indices(family, context, point, exact)
            .unwrap(),
        baseline,
        "the exact point-query census must be accepted"
    );

    let mut tested_positive_limits = 0usize;
    macro_rules! one_below_outer {
        ($limit_field:ident, $observed:expr) => {{
            let observed = $observed;
            if observed > 0 {
                tested_positive_limits += 1;
                let mut one_below = exact;
                one_below.$limit_field = observed - 1;
                let (_, requested, limit) = owner_point_resource_limit(
                    owner
                        .classification_for_indices(family, context, point, one_below)
                        .unwrap_err(),
                );
                assert_eq!((requested, limit), (observed, observed - 1));
            }
        }};
    }
    one_below_outer!(
        max_family_fingerprint_comparison_bytes,
        stats.family_fingerprint_comparison_bytes()
    );
    one_below_outer!(
        max_context_fingerprint_comparison_bytes,
        stats.context_fingerprint_comparison_bytes()
    );
    one_below_outer!(max_index_entries, stats.index_entries());
    one_below_outer!(max_global_cases, stats.global_cases());
    one_below_outer!(max_global_classifications, stats.global_classifications());
    one_below_outer!(max_global_predicates, stats.global_predicates());
    one_below_outer!(max_work_items_scanned, stats.work_items_scanned());
    one_below_outer!(
        max_inventory_terminal_scans,
        stats.inventory_terminal_scans()
    );
    one_below_outer!(max_boolean_nodes_scanned, stats.boolean_nodes_scanned());
    one_below_outer!(max_boolean_ready_terminals, stats.boolean_ready_terminals());
    one_below_outer!(max_boolean_predicates, stats.boolean_predicates());
    one_below_outer!(
        max_owner_terminal_record_scans,
        stats.owner_terminal_record_scans()
    );
    one_below_outer!(max_inventory_case_lookups, stats.inventory_case_lookups());
    one_below_outer!(max_group_pass_scans, stats.group_pass_scans());
    one_below_outer!(
        max_group_case_references_scanned,
        stats.group_case_references_scanned()
    );
    one_below_outer!(
        max_target_disposition_scans,
        stats.target_disposition_scans()
    );
    one_below_outer!(max_attempt_scans, stats.attempt_scans());
    one_below_outer!(max_child_output_lookups, stats.child_output_lookups());
    one_below_outer!(max_sealed_rule_scans, stats.sealed_rule_scans());
    one_below_outer!(max_residual_work_scans, stats.residual_work_scans());
    one_below_outer!(max_child_offset_arithmetic, stats.child_offset_arithmetic());
    one_below_outer!(
        max_child_offset_comparisons,
        stats.child_offset_comparisons()
    );
    one_below_outer!(
        max_child_authority_comparisons,
        stats.child_authority_comparisons()
    );

    macro_rules! one_below_specialization {
        ($stage:ident, $stats:expr, $limit_field:ident, $getter:ident) => {{
            let observed = $stats.$getter();
            if observed > 0 {
                tested_positive_limits += 1;
                let mut one_below = exact;
                one_below.$stage.$limit_field = observed - 1;
                let (_, requested, limit) = owner_point_resource_limit(
                    owner
                        .classification_for_indices(family, context, point, one_below)
                        .unwrap_err(),
                );
                assert_eq!((requested, limit), (observed, observed - 1));
            }
        }};
    }
    macro_rules! all_specialization_limits {
        ($stage:ident, $stats:expr) => {
            one_below_specialization!($stage, $stats, max_source_terms, source_terms);
            one_below_specialization!(
                $stage,
                $stats,
                max_source_exponent_entries,
                source_exponent_entries
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_preflight_validation_source_term_scan_bound,
                preflight_validation_source_term_scan_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_preflight_validation_source_exponent_entry_scan_bound,
                preflight_validation_source_exponent_entry_scan_bound
            );
            one_below_specialization!($stage, $stats, max_output_term_bound, output_term_bound);
            one_below_specialization!(
                $stage,
                $stats,
                max_output_exponent_entry_bound,
                output_exponent_entry_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_power_operation_bound,
                power_operation_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_largest_output_integer_bit_bound,
                largest_output_integer_bit_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_integer_bit_work_bound,
                integer_bit_work_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_retained_output_term_bound,
                retained_output_term_bound
            );
            one_below_specialization!(
                $stage,
                $stats,
                max_retained_output_byte_bound,
                retained_output_byte_bound
            );
        };
    }
    all_specialization_limits!(global_specialization, stats.global_specialization());
    all_specialization_limits!(boolean_specialization, stats.boolean_specialization());

    if let Some(map_stats) = stats.map() {
        macro_rules! one_below_map {
            ($limit_field:ident, $getter:ident) => {{
                let observed = map_stats.$getter();
                if observed > 0 {
                    tested_positive_limits += 1;
                    let mut one_below = exact;
                    one_below.map.$limit_field = observed - 1;
                    let (_, requested, limit) = owner_point_resource_limit(
                        owner
                            .classification_for_indices(family, context, point, one_below)
                            .unwrap_err(),
                    );
                    assert_eq!((requested, limit), (observed, observed - 1));
                }
            }};
        }
        one_below_map!(max_ambient_arity, ambient_arity);
        one_below_map!(max_matrix_entries_inspected, matrix_entries_inspected);
        one_below_map!(max_nonzero_multiplications, nonzero_multiplications);
        one_below_map!(max_additions, additions);
        one_below_map!(max_fixed_point_comparisons, fixed_point_comparisons);
        one_below_map!(max_peak_temporary_bytes, peak_temporary_bytes);
        one_below_map!(max_integer_bits, largest_integer_bits);
        one_below_map!(max_integer_bit_work, integer_bit_work);
    }

    if let Some(relative_stats) = stats.relative() {
        macro_rules! one_below_relative {
            ($limit_field:ident, $getter:ident) => {{
                let observed = relative_stats.$getter();
                if observed > 0 {
                    tested_positive_limits += 1;
                    let mut one_below = exact;
                    one_below.relative.$limit_field = observed - 1;
                    let (_, requested, limit) = owner_point_resource_limit(
                        owner
                            .classification_for_indices(family, context, point, one_below)
                            .unwrap_err(),
                    );
                    assert_eq!((requested, limit), (observed, observed - 1));
                }
            }};
        }
        one_below_relative!(
            max_context_fingerprint_comparison_bytes,
            context_fingerprint_comparison_bytes
        );
        one_below_relative!(max_index_entries, index_entries);
        one_below_relative!(max_cases, cases);
        one_below_relative!(max_classifications, classifications);
        one_below_relative!(max_predicates, predicates);
        one_below_relative!(max_source_terms, source_terms);
        one_below_relative!(max_source_exponent_entries, source_exponent_entries);
        one_below_relative!(
            max_preflight_validation_source_term_scan_bound,
            preflight_validation_source_term_scan_bound
        );
        one_below_relative!(
            max_preflight_validation_source_exponent_entry_scan_bound,
            preflight_validation_source_exponent_entry_scan_bound
        );
        one_below_relative!(max_output_term_bound, output_term_bound);
        one_below_relative!(max_output_exponent_entry_bound, output_exponent_entry_bound);
        one_below_relative!(max_power_operation_bound, power_operation_bound);
        one_below_relative!(
            max_largest_output_integer_bit_bound,
            largest_output_integer_bit_bound
        );
        one_below_relative!(max_integer_bit_work_bound, integer_bit_work_bound);
        one_below_relative!(max_retained_output_term_bound, retained_output_term_bound);
        one_below_relative!(max_retained_output_byte_bound, retained_output_byte_bound);
    }
    assert!(tested_positive_limits > 0);
}

fn bounded_fixture_points(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
) -> Vec<[i64; 3]> {
    assert_eq!(
        owner.source_queue().sector().arity(),
        3,
        "this concrete validation fixture has three denominators"
    );
    let mut points = Vec::new();
    for first in -4i64..=6 {
        for second in -4i64..=6 {
            for third in -4i64..=6 {
                let point = [first, second, third];
                if owner
                    .source_queue()
                    .sector()
                    .contains_indices(&point)
                    .unwrap()
                {
                    points.push(point);
                }
            }
        }
    }
    points
}

fn independent_initial_context_comparison_bytes(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
) -> usize {
    let queue = owner.source_queue();
    let global = queue.discovery().coverage();
    owner.inventory().context_fingerprint().len()
        + context.fingerprint().len()
        + queue.context_fingerprint().len()
        + context.fingerprint().len()
        + global.context_fingerprint().len()
        + context.fingerprint().len()
}

fn independent_through_global_context_comparison_bytes(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
) -> usize {
    let global = owner.source_queue().discovery().coverage();
    independent_initial_context_comparison_bytes(owner, context)
        + global.context_fingerprint().len()
        + context.fingerprint().len()
}

fn independent_through_boolean_context_comparison_bytes(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
    inventory_terminal_ordinal: usize,
) -> usize {
    let source_cover = owner.inventory().terminals()[inventory_terminal_ordinal].source_cover();
    independent_through_global_context_comparison_bytes(owner, context)
        + source_cover.context_fingerprint().len()
        + context.fingerprint().len()
        + source_cover.context_fingerprint().len()
        + context.fingerprint().len()
}

#[test]
fn generated_001_sealed_owner_application_produces_exact_concrete_reduction() {
    let (family, context, owner) =
        owner_fixture("001", "sector-affine-sealed-application-generated-001");
    let owner = Arc::new(owner);
    let source = [-4, -4, 2];
    let expected_locator = GeneratedSectorAffineRuleLocator {
        group_pass_ordinal: 0,
        accepted_attempt_ordinal: 0,
        leaf_ordinal: 4,
    };
    let classification = owner
        .classification_for_indices(
            &family,
            &context,
            &source,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        classification.disposition(),
        GeneratedSectorAffinePointDisposition::Rule(expected_locator)
    );

    let application = owner
        .concrete_application_for_indices(
            &family,
            &context,
            &source,
            GeneratedSectorAffineRuleApplicationLimits::default(),
        )
        .unwrap();
    let stats = application.stats();
    assert_eq!(stats.point(), classification.stats());
    assert_eq!(
        (
            stats.owner_replays(),
            stats.group_pass_lookups(),
            stats.sealed_rule_scans(),
            stats.symbolic_rhs_terms(),
            stats.specialized_rhs_terms(),
            stats.required_nonzero_conditions(),
            stats.required_nonzero_origins(),
            stats.retained_authority_references(),
        ),
        (1, 1, 2, 4, 4, 0, 0, 1),
    );
    let GeneratedSectorAffineConcretePointOutcome::Reduction(reduction) = application.outcome()
    else {
        panic!("the authenticated applicable leaf must produce a reduction")
    };
    assert!(reduction.coordinate_rule().is_none());
    assert_eq!(
        reduction.sector(),
        &SectorMask::try_from_bit_string("001").unwrap()
    );
    assert_eq!(
        reduction.ordering_policy(),
        IntegralOrderingPolicy::RustRedUnshiftedV1
    );
    assert_eq!(reduction.pivot_ordinal(), 0);
    assert_eq!(reduction.source().powers(), &source);
    assert_eq!(reduction.rhs().len(), 4);
    assert_concrete_reduction_coefficient(reduction, &family, [-3, -5, 2], "1");
    assert_concrete_reduction_coefficient(reduction, &family, [-3, -4, 1], "d+5");
    assert_concrete_reduction_coefficient(reduction, &family, [-3, -4, 2], "-m2");
    assert_concrete_reduction_coefficient(reduction, &family, [-2, -4, 1], "6*m2");
    assert!(reduction.required_nonzero().is_empty());
    assert!(
        reduction
            .specialized_relation()
            .guarded_nonzero_conditions()
            .is_empty()
    );
    assert!(
        reduction
            .rhs()
            .keys()
            .eq(reduction.descent_witnesses().keys())
    );
    for (target, witness) in reduction.descent_witnesses() {
        let target_sector = SectorMask::try_from_indices(target.powers()).unwrap();
        assert!(target_sector.is_subsector_of(reduction.sector()).unwrap());
        assert_eq!(witness.policy(), reduction.ordering_policy());
        assert!(witness.verify());
        assert_eq!(
            *witness,
            reduction
                .ordering_policy()
                .prove_strict_descent(reduction.source().powers(), target.powers())
                .unwrap()
        );
    }
    assert!(
        reduction
            .verify_application(
                family.coefficient_context(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Default::default(),
            )
            .unwrap()
    );
    reduction.replay(&family, &context).unwrap();

    let debug = format!("{application:?} {reduction:?}");
    assert!(debug.contains("GeneratedAffine"));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("ParametricRelation"));
    assert!(!debug.contains("pullback"));
    assert!(!debug.contains("polynomial"));

    owner.replay(&family, &context).unwrap();
    let replayed_seam_baseline = owner
        .concrete_application_for_indices_from_replayed_owner(
            &family,
            &context,
            &source,
            GeneratedSectorAffineRuleApplicationLimits::default(),
        )
        .unwrap();
    let replayed_seam_stats = replayed_seam_baseline.stats();
    assert_eq!(
        (
            replayed_seam_stats.owner_replays(),
            replayed_seam_stats.group_pass_lookups(),
            replayed_seam_stats.sealed_rule_scans(),
            replayed_seam_stats.symbolic_rhs_terms(),
            replayed_seam_stats.specialized_rhs_terms(),
            replayed_seam_stats.required_nonzero_conditions(),
            replayed_seam_stats.required_nonzero_origins(),
            replayed_seam_stats.retained_authority_references(),
        ),
        (0, 1, 2, 4, 4, 0, 0, 1),
    );
    let replayed_seam_exact = owner
        .concrete_application_for_indices_from_replayed_owner(
            &family,
            &context,
            &source,
            exact_owner_rule_application_limits(replayed_seam_stats),
        )
        .unwrap();
    assert_eq!(replayed_seam_exact.stats(), replayed_seam_stats);
    let GeneratedSectorAffineConcretePointOutcome::Reduction(replayed_seam_reduction) =
        replayed_seam_exact.into_outcome()
    else {
        panic!("the replayed-owner seam must reproduce the concrete reduction")
    };
    assert_eq!(replayed_seam_reduction.source(), reduction.source());
    assert_eq!(replayed_seam_reduction.rhs(), reduction.rhs());
    assert_eq!(
        replayed_seam_reduction.descent_witnesses(),
        reduction.descent_witnesses()
    );
    assert!(replayed_seam_reduction.required_nonzero().is_empty());
    replayed_seam_reduction.replay(&family, &context).unwrap();
}

#[test]
fn generated_001_sealed_owner_application_enforces_exact_positive_budgets() {
    let (family, context, owner) =
        owner_fixture("001", "sector-affine-sealed-budgets-generated-001");
    let owner = Arc::new(owner);
    let source = [-4, -4, 2];
    let baseline = owner
        .concrete_application_for_indices(
            &family,
            &context,
            &source,
            GeneratedSectorAffineRuleApplicationLimits::default(),
        )
        .unwrap();
    let stats = baseline.stats();
    let exact = exact_owner_rule_application_limits(stats);
    let exact_application = owner
        .concrete_application_for_indices(&family, &context, &source, exact)
        .unwrap();
    assert_eq!(exact_application.stats(), stats);
    assert!(matches!(
        exact_application.outcome(),
        GeneratedSectorAffineConcretePointOutcome::Reduction(_)
    ));

    let mut tested_positive_limits = 0usize;
    macro_rules! one_below_application {
        ($limit_field:ident, $observed:expr) => {{
            let observed = $observed;
            assert!(observed > 0);
            tested_positive_limits += 1;
            let mut one_below = exact;
            one_below.$limit_field = observed - 1;
            let (_, requested, limit) = owner_rule_application_resource_limit(
                owner
                    .concrete_application_for_indices(&family, &context, &source, one_below)
                    .unwrap_err(),
            );
            assert_eq!((requested, limit), (observed, observed - 1));
        }};
    }
    one_below_application!(max_owner_replays, stats.owner_replays());
    one_below_application!(max_group_pass_lookups, stats.group_pass_lookups());
    one_below_application!(max_sealed_rule_scans, stats.sealed_rule_scans());
    one_below_application!(max_symbolic_rhs_terms, stats.symbolic_rhs_terms());
    one_below_application!(max_specialized_rhs_terms, stats.specialized_rhs_terms());
    one_below_application!(
        max_retained_authority_references,
        stats.retained_authority_references()
    );
    one_below_application!(
        max_concrete_reduction_retained_byte_bound,
        stats.concrete_reduction_retained_byte_bound()
    );
    one_below_application!(
        max_peak_visible_application_byte_bound,
        stats.peak_visible_application_byte_bound()
    );
    assert_eq!(tested_positive_limits, 8);
    assert_eq!(stats.required_nonzero_conditions(), 0);
    assert_eq!(stats.required_nonzero_origins(), 0);
    assert!(stats.concrete_reduction_retained_bytes() > 0);
    assert!(
        stats.concrete_reduction_retained_bytes() <= stats.concrete_reduction_retained_byte_bound()
    );
}

#[test]
fn generated_001_sealed_owner_application_returns_exceptional_and_refuses_tampering() {
    let (family, context, owner) =
        owner_fixture("001", "sector-affine-sealed-exceptional-generated-001");
    let exceptional_source = [-4, -4, 1];
    let expected = GeneratedSectorAffinePointDisposition::Exceptional(
        GeneratedSectorAffineExceptionalChildLocator {
            group_pass_ordinal: 0,
            accepted_attempt_ordinal: 0,
            leaf_ordinal: 0,
        },
    );
    let classification = owner
        .classification_for_indices(
            &family,
            &context,
            &exceptional_source,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(classification.disposition(), expected);
    let owner = Arc::new(owner);
    let application = owner
        .concrete_application_for_indices(
            &family,
            &context,
            &exceptional_source,
            GeneratedSectorAffineRuleApplicationLimits::default(),
        )
        .unwrap();
    assert!(matches!(
        application.outcome(),
        GeneratedSectorAffineConcretePointOutcome::Disposition(disposition)
            if *disposition == expected
    ));
    let stats = application.stats();
    assert_eq!(stats.point(), classification.stats());
    assert_eq!(
        (
            stats.owner_replays(),
            stats.group_pass_lookups(),
            stats.sealed_rule_scans(),
            stats.symbolic_rhs_terms(),
            stats.specialized_rhs_terms(),
            stats.required_nonzero_conditions(),
            stats.required_nonzero_origins(),
            stats.retained_authority_references(),
        ),
        (1, 0, 0, 0, 0, 0, 0, 0),
    );

    let (tampered_family, tampered_context, mut tampered_owner) =
        owner_fixture("001", "sector-affine-sealed-tampered-generated-001");
    assert!(tampered_owner.test_only_corrupt_first_ordered_child_output());
    let tampered_owner = Arc::new(tampered_owner);
    assert!(matches!(
        tampered_owner.concrete_application_for_indices(
            &tampered_family,
            &tampered_context,
            &[-4, -4, 2],
            GeneratedSectorAffineRuleApplicationLimits::default(),
        ),
        Err(GeneratedSectorAffineRuleApplicationError::OwnerReplay(_))
    ));
}

#[test]
fn generated_011_sealed_owner_application_retains_public_safe_mass_guards() {
    let (family, context, owner) =
        owner_fixture("011", "sector-affine-sealed-guards-generated-011");
    let owner = Arc::new(owner);
    let source = [0, 1, 2];
    let expected_locator = GeneratedSectorAffineRuleLocator {
        group_pass_ordinal: 0,
        accepted_attempt_ordinal: 2,
        leaf_ordinal: 1,
    };
    let classification = owner
        .classification_for_indices(
            &family,
            &context,
            &source,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        classification.disposition(),
        GeneratedSectorAffinePointDisposition::Rule(expected_locator)
    );

    owner.replay(&family, &context).unwrap();
    let application = owner
        .concrete_application_for_indices_from_replayed_owner(
            &family,
            &context,
            &source,
            GeneratedSectorAffineRuleApplicationLimits::default(),
        )
        .unwrap();
    let stats = application.stats();
    assert_eq!(
        (
            stats.owner_replays(),
            stats.group_pass_lookups(),
            stats.sealed_rule_scans(),
            stats.symbolic_rhs_terms(),
            stats.specialized_rhs_terms(),
            stats.required_nonzero_conditions(),
            stats.required_nonzero_origins(),
            stats.retained_authority_references(),
        ),
        (0, 1, 1, 1, 1, 2, 2, 1),
    );
    assert!(stats.concrete_reduction_retained_bytes() > 0);
    assert!(
        stats.concrete_reduction_retained_bytes() <= stats.concrete_reduction_retained_byte_bound()
    );
    assert!(
        stats.sealed().temporary_condition_retained_bytes()
            <= stats.sealed().temporary_condition_retained_byte_bound()
    );

    let exact_limits = exact_owner_rule_application_limits(stats);
    let exact_application = owner
        .concrete_application_for_indices_from_replayed_owner(
            &family,
            &context,
            &source,
            exact_limits,
        )
        .unwrap();
    assert_eq!(exact_application.stats(), stats);
    for (resource, one_below) in [
        ("required conditions", {
            let mut limits = exact_limits;
            limits.max_required_nonzero_conditions = stats.required_nonzero_conditions() - 1;
            limits
        }),
        ("required origins", {
            let mut limits = exact_limits;
            limits.max_required_nonzero_origins = stats.required_nonzero_origins() - 1;
            limits
        }),
        ("retained reduction", {
            let mut limits = exact_limits;
            limits.max_concrete_reduction_retained_byte_bound =
                stats.concrete_reduction_retained_byte_bound() - 1;
            limits
        }),
        ("visible peak", {
            let mut limits = exact_limits;
            limits.max_peak_visible_application_byte_bound =
                stats.peak_visible_application_byte_bound() - 1;
            limits
        }),
    ] {
        let (_, requested, limit) = owner_rule_application_resource_limit(
            owner
                .concrete_application_for_indices_from_replayed_owner(
                    &family, &context, &source, one_below,
                )
                .unwrap_err(),
        );
        assert_eq!(
            requested,
            limit + 1,
            "wrong one-below failure for {resource}"
        );
    }
    let GeneratedSectorAffineConcretePointOutcome::Reduction(reduction) = application.outcome()
    else {
        panic!("the authenticated guarded leaf must produce a reduction")
    };
    assert_eq!(reduction.pivot_ordinal(), 2);
    assert_eq!(reduction.source().powers(), &source);
    assert_concrete_reduction_coefficient(reduction, &family, [0, 1, 1], "(d-2)/(2*m2)");
    assert_eq!(reduction.required_nonzero().len(), 2);
    assert_eq!(
        reduction.required_nonzero(),
        reduction
            .specialized_relation()
            .guarded_nonzero_conditions()
    );
    for condition in reduction.required_nonzero() {
        assert!(!condition.polynomial().is_nonzero_constant());
        assert_eq!(condition.origins().len(), 1);
        assert!(
            condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        );
    }
    let guarded_polynomials = reduction
        .required_nonzero()
        .iter()
        .map(|condition| condition.polynomial().raw().clone().into())
        .collect::<Vec<_>>();
    for expected in ["-2*m2", "2*m2"] {
        let expected = family.coefficient_context().parse(expected).unwrap();
        assert!(guarded_polynomials.iter().any(|actual| {
            family
                .coefficient_context()
                .try_sub(actual, &expected, Default::default())
                .is_ok_and(|delta| delta.is_zero())
        }));
    }
    let public_guard_debug = format!("{:?}", reduction.required_nonzero());
    assert!(public_guard_debug.contains("GeneratedAffineSealedCondition"));
    for private_token in [
        "RelationAffineFreeRecentering",
        "RelationInputTermDenominator",
        "RelationCollectedTermDenominator",
        "IndexTranslation",
        "coefficient_offset",
        "key_center",
    ] {
        assert!(!public_guard_debug.contains(private_token));
    }
    assert!(
        reduction
            .verify_application(
                family.coefficient_context(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Default::default(),
            )
            .unwrap()
    );
    reduction.replay(&family, &context).unwrap();
}

#[test]
fn generated_001_fixture_point_classifier_finds_rule_and_exceptional_children() {
    let (family, context, owner) = owner_fixture("001", "sector-affine-point-generated-001");
    let mut rule_witness = None;
    let mut exceptional_witness = None;
    let mut first_child_witness = None;
    for point in bounded_fixture_points(&owner) {
        let expected = independent_owner_point(&owner, &context, &point);
        let actual = owner
            .classification_for_indices(
                &family,
                &context,
                &point,
                GeneratedSectorAffinePointLimits::default(),
            )
            .unwrap();
        assert_eq!(
            actual.disposition(),
            expected.disposition,
            "point {point:?}"
        );
        match expected.disposition {
            GeneratedSectorAffinePointDisposition::Rule(_) => {
                rule_witness.get_or_insert(point);
            }
            GeneratedSectorAffinePointDisposition::Exceptional(_) => {
                exceptional_witness.get_or_insert(point);
            }
            GeneratedSectorAffinePointDisposition::OutsideSector
            | GeneratedSectorAffinePointDisposition::CoveredByGlobal { .. }
            | GeneratedSectorAffinePointDisposition::ResidualRoot(_) => {}
        }
        if expected.child_output_ordinal == Some(0) {
            first_child_witness.get_or_insert(point);
        }
        if rule_witness.is_some() && exceptional_witness.is_some() && first_child_witness.is_some()
        {
            break;
        }
    }
    let rule_point = rule_witness.expect("the generated fixture must expose an integer rule leaf");
    let exceptional_point =
        exceptional_witness.expect("the generated fixture must expose an integer exceptional leaf");
    let first_child_point =
        first_child_witness.expect("the first flattened child must have a nearby integer witness");

    let rule = owner
        .classification_for_indices(
            &family,
            &context,
            &rule_point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    let exceptional = owner
        .classification_for_indices(
            &family,
            &context,
            &exceptional_point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert!(rule.stats().map().is_some());
    assert!(rule.stats().relative().is_some());
    assert_eq!(
        rule.stats().inventory_case_lookups(),
        2,
        "the actionable path resolves the queried case and its group anchor"
    );
    assert!(exceptional.stats().map().is_some());
    assert!(exceptional.stats().relative().is_some());
    assert_eq!(
        exceptional.stats().inventory_case_lookups(),
        2,
        "the actionable path resolves the queried case and its group anchor"
    );
    let mut exact_case_lookups = GeneratedSectorAffinePointLimits::default();
    exact_case_lookups.max_inventory_case_lookups = 2;
    assert_eq!(
        owner
            .classification_for_indices(&family, &context, &rule_point, exact_case_lookups,)
            .unwrap()
            .disposition(),
        rule.disposition()
    );
    let mut one_below_case_lookups = exact_case_lookups;
    one_below_case_lookups.max_inventory_case_lookups = 1;
    let (_, requested, limit) = owner_point_resource_limit(
        owner
            .classification_for_indices(&family, &context, &rule_point, one_below_case_lookups)
            .unwrap_err(),
    );
    assert_eq!((requested, limit), (2, 1));
    let independent_rule = independent_owner_point(&owner, &context, &rule_point);
    let rule_terminal_ordinal = independent_rule
        .inventory_terminal_ordinal
        .expect("an independently resolved rule owns one source terminal");
    assert_eq!(
        rule.stats().context_fingerprint_comparison_bytes(),
        independent_through_boolean_context_comparison_bytes(
            &owner,
            &context,
            rule_terminal_ordinal,
        ),
        "full rule classification authenticates initial scope, delegated global cover, source-cover provenance, and delegated Boolean cover"
    );
    let debug = format!("{rule:?} {exceptional:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("indices"));
    assert!(!debug.contains("polynomial"));
    assert!(!debug.contains("ParametricRelation"));
    assert!(!debug.contains("pullback"));

    assert_exact_and_one_below_owner_point_limits(&owner, &family, &context, &rule_point);
    assert_exact_and_one_below_owner_point_limits(&owner, &family, &context, &exceptional_point);

    let wrong_family = equal_mass_two_loop_family("sector-affine-point-wrong-family");
    assert!(matches!(
        owner.classification_for_indices(
            &wrong_family,
            &context,
            &rule_point,
            GeneratedSectorAffinePointLimits::default(),
        ),
        Err(GeneratedSectorAffinePointError::WrongFamily)
    ));
    let wrong_context = ParametricCoefficientContext::try_new(
        context.base(),
        "sector-affine-point-wrong-context",
        context.index_count(),
    )
    .unwrap();
    assert!(matches!(
        owner.classification_for_indices(
            &family,
            &wrong_context,
            &rule_point,
            GeneratedSectorAffinePointLimits::default(),
        ),
        Err(GeneratedSectorAffinePointError::WrongContext)
    ));
    assert!(matches!(
        owner.classification_for_indices(
            &family,
            &context,
            &rule_point[..2],
            GeneratedSectorAffinePointLimits::default(),
        ),
        Err(GeneratedSectorAffinePointError::WrongArity {
            expected: 3,
            actual: 2,
        })
    ));

    let mut corrupted = owner.clone();
    assert!(corrupted.test_only_corrupt_first_ordered_child_output());
    assert!(
        corrupted
            .classification_for_indices(
                &family,
                &context,
                &first_child_point,
                GeneratedSectorAffinePointLimits::default(),
            )
            .is_err()
    );
}

#[test]
fn generated_011_and_101_fixture_points_cover_global_residual_and_short_circuits() {
    let (family_011, context_011, owner_011) =
        owner_fixture("011", "sector-affine-point-generated-011");
    let global_point = bounded_fixture_points(&owner_011)
        .into_iter()
        .find(|point| {
            matches!(
                independent_owner_point(&owner_011, &context_011, point).disposition,
                GeneratedSectorAffinePointDisposition::CoveredByGlobal { .. }
            )
        })
        .expect("the generated 011 fixture must contain a nearby V1-covered point");
    let independent_global = independent_owner_point(&owner_011, &context_011, &global_point);
    let global = owner_011
        .classification_for_indices(
            &family_011,
            &context_011,
            &global_point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(global.disposition(), independent_global.disposition);
    assert!(global.stats().global_cases() > 0);
    assert_eq!(global.stats().work_items_scanned(), 0);
    assert_eq!(global.stats().inventory_case_lookups(), 0);
    assert!(global.stats().map().is_none());
    assert!(global.stats().relative().is_none());
    assert_eq!(
        global.stats().context_fingerprint_comparison_bytes(),
        independent_through_global_context_comparison_bytes(&owner_011, &context_011),
        "a global hit performs the initial and delegated global comparisons only"
    );

    let (residual_011_point, independent_residual_011) = bounded_fixture_points(&owner_011)
        .into_iter()
        .find_map(|point| {
            let independent = independent_owner_point(&owner_011, &context_011, &point);
            matches!(
                independent.disposition,
                GeneratedSectorAffinePointDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot { .. }
                )
            )
            .then_some((point, independent))
        })
        .expect("the generated 011 fixture must contain a nearby unconsumed target root");
    let residual_011 = owner_011
        .classification_for_indices(
            &family_011,
            &context_011,
            &residual_011_point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        residual_011.disposition(),
        independent_residual_011.disposition
    );
    assert!(residual_011.stats().map().is_some());
    assert!(residual_011.stats().relative().is_none());
    assert_eq!(
        residual_011.stats().inventory_case_lookups(),
        2,
        "an actionable residual resolves the queried case and its group anchor"
    );
    assert_eq!(
        residual_011.stats().context_fingerprint_comparison_bytes(),
        independent_through_boolean_context_comparison_bytes(
            &owner_011,
            &context_011,
            independent_residual_011
                .inventory_terminal_ordinal
                .expect("the independently resolved residual owns one source terminal"),
        ),
        "an actionable residual reaches both delegated point classifiers"
    );

    let outside_point = [0, 0, 0];
    assert_eq!(
        independent_owner_point(&owner_011, &context_011, &outside_point).disposition,
        GeneratedSectorAffinePointDisposition::OutsideSector
    );
    let outside = owner_011
        .classification_for_indices(
            &family_011,
            &context_011,
            &outside_point,
            GeneratedSectorAffinePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        outside.disposition(),
        GeneratedSectorAffinePointDisposition::OutsideSector
    );
    assert_eq!(outside.stats().global_cases(), 0);
    assert_eq!(outside.stats().work_items_scanned(), 0);
    assert_eq!(outside.stats().inventory_case_lookups(), 0);
    assert!(outside.stats().map().is_none());
    assert!(outside.stats().relative().is_none());
    assert_eq!(
        outside.stats().context_fingerprint_comparison_bytes(),
        independent_initial_context_comparison_bytes(&owner_011, &context_011),
        "an outside point stops after initial scope authentication"
    );

    // Exact limits set every unvisited stage to zero, directly checking the
    // outside -> global -> residual staged short-circuit boundaries.
    assert_exact_and_one_below_owner_point_limits(
        &owner_011,
        &family_011,
        &context_011,
        &outside_point,
    );
    assert_exact_and_one_below_owner_point_limits(
        &owner_011,
        &family_011,
        &context_011,
        &global_point,
    );
    assert_exact_and_one_below_owner_point_limits(
        &owner_011,
        &family_011,
        &context_011,
        &residual_011_point,
    );

    let (family_101, context_101, owner_101) =
        owner_fixture("101", "sector-affine-point-generated-101");
    let (residual_101_point, independent_residual_101) = bounded_fixture_points(&owner_101)
        .into_iter()
        .find_map(|point| {
            let independent = independent_owner_point(&owner_101, &context_101, &point);
            matches!(
                independent.disposition,
                GeneratedSectorAffinePointDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot { .. }
                )
            )
            .then_some((point, independent))
        })
        .expect("the generated 101 fixture must contain a nearby unconsumed target root");
    assert_eq!(
        owner_101
            .classification_for_indices(
                &family_101,
                &context_101,
                &residual_101_point,
                GeneratedSectorAffinePointLimits::default(),
            )
            .unwrap()
            .disposition(),
        independent_residual_101.disposition
    );
}

fn assert_terminal_mapping_partition_ranges_and_child_authority(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
) {
    let inventory = owner.inventory();
    assert_eq!(owner.terminal_records().len(), inventory.terminals().len());
    let mut partition_ranges = Vec::new();

    for (terminal_ordinal, (record, source)) in owner
        .terminal_records()
        .iter()
        .zip(inventory.terminals())
        .enumerate()
    {
        assert_eq!(record.inventory_terminal_ordinal(), terminal_ordinal);
        assert_eq!(record.source_locator(), source.locator());
        assert_eq!(record.source_outcome(), source.outcome());

        match (record.source_outcome(), record.disposition()) {
            (
                GeneratedResidualAffineInventoryTerminalOutcome::SourceCoordinateLeafProvedEmpty
                | GeneratedResidualAffineInventoryTerminalOutcome::BooleanProvedEmpty
                | GeneratedResidualAffineInventoryTerminalOutcome::AffineProvedEmpty
                | GeneratedResidualAffineInventoryTerminalOutcome::GuardContradiction { .. },
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty,
            ) => {}
            (
                GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported,
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                        terminal_ordinal: retained,
                    },
                ),
            ) => assert_eq!(retained, terminal_ordinal),
            (
                GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal },
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase {
                        case_ordinal: retained,
                    },
                ),
            ) => assert_eq!(retained, case_ordinal),
            (
                GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal },
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                        group_pass_ordinal,
                        target_case_ordinal,
                    },
                ),
            ) => {
                assert_eq!(target_case_ordinal, case_ordinal);
                let pass = &owner.group_passes()[group_pass_ordinal];
                assert!(matches!(
                    pass.outcome(),
                    GeneratedSectorAffineGroupPassOutcome::Effective(_)
                ));
            }
            (
                GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal },
                GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                    group_pass_ordinal,
                    target_case_ordinal,
                    first_child_output_ordinal,
                    child_output_count,
                },
            ) => {
                assert_eq!(target_case_ordinal, case_ordinal);
                assert!(child_output_count > 0);
                let end = first_child_output_ordinal
                    .checked_add(child_output_count)
                    .expect("small generated child range fits usize");
                assert!(end <= owner.ordered_child_outputs().len());
                partition_ranges.push((first_child_output_ordinal, end));
                let children = &owner.ordered_child_outputs()[first_child_output_ordinal..end];
                let pass = &owner.group_passes()[group_pass_ordinal];
                let GeneratedSectorAffineGroupPassOutcome::Effective(effective) = pass.outcome()
                else {
                    panic!("a partitioned target must be owned by an effective group pass");
                };
                assert!(Arc::ptr_eq(
                    effective.matcher().inventory(),
                    owner.inventory()
                ));
                let target_position = inventory.cases()[case_ordinal].ordinal_within_group();
                let GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal,
                    ..
                } = effective.target_dispositions()[target_position].disposition()
                else {
                    panic!("a partitioned target must resolve to a consumed group disposition");
                };

                for (leaf_ordinal, child) in children.iter().enumerate() {
                    match child {
                        GeneratedSectorAffineOrderedChildOutput::Rule(locator) => {
                            assert_eq!(locator.group_pass_ordinal, group_pass_ordinal);
                            assert_eq!(locator.accepted_attempt_ordinal, *accepted_attempt_ordinal);
                            assert_eq!(locator.leaf_ordinal, leaf_ordinal);
                            assert_eq!(
                                effective
                                    .sealed_rules()
                                    .iter()
                                    .filter(|rule| {
                                        rule.target_case_ordinal() == case_ordinal
                                            && rule.accepted_attempt_ordinal()
                                                == *accepted_attempt_ordinal
                                            && rule.leaf_ordinal() == leaf_ordinal
                                    })
                                    .count(),
                                1,
                                "every rule locator resolves uniquely"
                            );
                        }
                        GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) => {
                            assert_eq!(locator.group_pass_ordinal, group_pass_ordinal);
                            assert_eq!(locator.accepted_attempt_ordinal, *accepted_attempt_ordinal);
                            assert_eq!(locator.leaf_ordinal, leaf_ordinal);
                            assert_eq!(
                                effective
                                    .residual_work()
                                    .iter()
                                    .filter(|leaf| {
                                        leaf.target_case_ordinal() == case_ordinal
                                            && leaf.accepted_attempt_ordinal()
                                                == Some(*accepted_attempt_ordinal)
                                            && leaf.leaf_ordinal() == Some(leaf_ordinal)
                                            && !matches!(
                                                leaf.kind(),
                                                GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
                                            )
                                    })
                                    .count(),
                                1,
                                "every exceptional locator resolves uniquely"
                            );
                        }
                    }
                }
            }
            (source, disposition) => {
                panic!("source terminal outcome {source:?} mapped to {disposition:?}")
            }
        }
    }

    partition_ranges.sort_unstable();
    let mut next_child_output = 0usize;
    for (first, end) in partition_ranges {
        assert_eq!(first, next_child_output);
        assert!(end > first);
        next_child_output = end;
    }
    assert_eq!(next_child_output, owner.ordered_child_outputs().len());
    for (pass_ordinal, (pass, group)) in owner
        .group_passes()
        .iter()
        .zip(inventory.groups())
        .enumerate()
    {
        assert_eq!(pass.pass_ordinal(), pass_ordinal);
        assert_eq!(pass.group_ordinal(), group.ordinal());
        assert_eq!(pass.source_case_ordinal(), group.anchor_case_ordinal());
        if let GeneratedSectorAffineGroupPassOutcome::Effective(effective) = pass.outcome() {
            assert!(Arc::ptr_eq(effective.matcher().inventory(), inventory));
        }
    }
}

#[test]
fn generated_001_owner_conserves_all_terminals_and_replays() {
    let (family, context, inventory) =
        inventory_fixture("001", "sector-affine-owner-generated-001");
    let owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory.clone(),
        GeneratedSectorAffineEffectiveCoverageConfig::new(0),
        GeneratedSectorAffineEffectiveCoverageLimits::default(),
    )
    .unwrap();

    assert!(Arc::ptr_eq(owner.inventory(), &inventory));
    assert!(Arc::ptr_eq(owner.source_queue(), inventory.source_queue()));
    assert_eq!(owner.group_passes().len(), inventory.groups().len());
    assert_eq!(owner.terminal_records().len(), inventory.terminals().len());
    assert_terminal_mapping_partition_ranges_and_child_authority(&owner);
    assert!(owner.stats().consumed_targets() > 0);
    assert!(owner.stats().rule_locators() > 0);
    assert_eq!(
        owner.stats().ordered_child_outputs(),
        owner.stats().rule_locators() + owner.stats().exceptional_child_locators()
    );
    assert_eq!(
        owner.stats().actionable_terminals(),
        owner.stats().unprocessed_actionable_roots()
            + owner.stats().consumed_targets()
            + owner.stats().unconsumed_target_roots()
    );
    owner.replay(&family, &context).unwrap();

    let debug = format!("{owner:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("ParametricRelation"));
    assert!(!debug.contains("pullback"));

    let mut corrupted = owner.clone();
    assert!(corrupted.test_only_corrupt_first_pass_group_ordinal());
    assert!(corrupted.replay(&family, &context).is_err());
    let mut corrupted = owner.clone();
    assert!(corrupted.test_only_corrupt_first_terminal_disposition());
    assert!(corrupted.replay(&family, &context).is_err());

    let retained_inventory = Arc::downgrade(&inventory);
    drop(inventory);
    assert!(retained_inventory.upgrade().is_some());
    owner.replay(&family, &context).unwrap();
}

fn assert_two_unconsumed_group_roots(bits: &str, name: &str) {
    let (family, context, inventory) = inventory_fixture(bits, name);
    let owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory,
        GeneratedSectorAffineEffectiveCoverageConfig::new(0),
        GeneratedSectorAffineEffectiveCoverageLimits::default(),
    )
    .unwrap();
    let roots = owner
        .terminal_records()
        .iter()
        .filter_map(|record| match record.disposition() {
            GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                    group_pass_ordinal: 1,
                    target_case_ordinal,
                },
            ) => Some(target_case_ordinal),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(roots, vec![1, 3]);
    assert!(
        !owner
            .ordered_child_outputs()
            .iter()
            .any(|output| match output {
                GeneratedSectorAffineOrderedChildOutput::Rule(locator) => {
                    locator.group_pass_ordinal == 1
                }
                GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) => {
                    locator.group_pass_ordinal == 1
                }
            })
    );
    assert_terminal_mapping_partition_ranges_and_child_authority(&owner);
    owner.replay(&family, &context).unwrap();
}

#[test]
fn generated_011_owner_retains_the_two_case_unconsumed_group() {
    assert_two_unconsumed_group_roots("011", "sector-affine-owner-generated-011");
}

#[test]
fn generated_101_owner_retains_the_two_case_unconsumed_group() {
    assert_two_unconsumed_group_roots("101", "sector-affine-owner-generated-101");
}

#[test]
fn generated_001_owner_accepts_exact_aggregate_limits_and_rejects_one_below() {
    let (family, context, inventory) =
        inventory_fixture("001", "sector-affine-owner-generated-001-limits");
    let baseline = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory.clone(),
        GeneratedSectorAffineEffectiveCoverageConfig::new(0),
        GeneratedSectorAffineEffectiveCoverageLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();
    assert!(stats.owned_child_arc_control_and_padding_bytes() > 0);
    assert!(
        stats.outer_retained_bytes() > stats.owned_child_arc_control_and_padding_bytes(),
        "the outer retained-byte census must include both its tables and owned Arc overhead"
    );
    let mut exact = baseline.limits();
    exact.max_group_passes = stats.group_passes();
    exact.max_group_case_references = stats.group_case_references();
    exact.max_terminal_records = stats.terminal_records();
    exact.max_ordered_child_outputs = stats.ordered_child_outputs();
    exact.max_rule_locators = stats.rule_locators();
    exact.max_residual_locators = stats.residual_locators();
    exact.max_cumulative_ordering_matrix_entries_inspected =
        stats.cumulative_ordering_matrix_entries_inspected();
    exact.max_cumulative_schedule_retained_points = stats.cumulative_schedule_retained_points();
    exact.max_cumulative_reelimination_expanded_rows =
        stats.cumulative_reelimination_expanded_rows();
    exact.max_cumulative_matcher_pivots = stats.cumulative_matcher_pivots();
    exact.max_cumulative_local_when_bad_compilations =
        stats.cumulative_local_when_bad_compilations();
    exact.max_scratch_bytes = stats.scratch_bytes();
    exact.max_outer_retained_bytes = stats.outer_retained_bytes();
    exact.max_outer_payload_comparison_units = stats.outer_payload_comparison_units();

    let exact_owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory.clone(),
        baseline.config(),
        exact,
    )
    .unwrap();
    assert_eq!(exact_owner.stats(), stats);

    assert!(stats.group_passes() > 0);
    let mut one_below = exact;
    one_below.max_group_passes = stats.group_passes() - 1;
    assert!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        )
        .is_err()
    );

    assert!(stats.rule_locators() > 0);
    let mut one_below = exact;
    one_below.max_rule_locators = stats.rule_locators() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource: "sector affine rule locators",
            ..
        })
    ));

    assert!(stats.residual_locators() > 0);
    let mut one_below = exact;
    one_below.max_residual_locators = stats.residual_locators() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource: "sector affine residual locators",
            ..
        })
    ));

    assert!(stats.outer_retained_bytes() > 0);
    let mut one_below = exact;
    one_below.max_outer_retained_bytes = stats.outer_retained_bytes() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource: "sector affine outer retained bytes",
            ..
        })
    ));

    assert!(stats.cumulative_ordering_matrix_entries_inspected() > 0);
    let mut one_below = exact;
    one_below.max_cumulative_ordering_matrix_entries_inspected =
        stats.cumulative_ordering_matrix_entries_inspected() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::Ordering(
            AffineParametricOrderingError::ResourceLimit { .. }
        ))
    ));

    assert!(stats.cumulative_schedule_retained_points() > 0);
    let mut one_below = exact;
    one_below.max_cumulative_schedule_retained_points =
        stats.cumulative_schedule_retained_points() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::Schedule(
            AffinePreparePointScheduleError::CumulativeResourceLimit { .. }
        ))
    ));

    assert!(stats.cumulative_reelimination_expanded_rows() > 0);
    let mut one_below = exact;
    one_below.max_cumulative_reelimination_expanded_rows =
        stats.cumulative_reelimination_expanded_rows() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::Reelimination(
            GeneratedResidualAffineBranchReeliminationError::ResourceLimit { .. }
        ))
    ));

    assert!(stats.cumulative_matcher_pivots() > 0);
    let mut one_below = exact;
    one_below.max_cumulative_matcher_pivots = stats.cumulative_matcher_pivots() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory.clone(),
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::Matcher(
            GeneratedResidualAffinePivotTargetMatchingError::ResourceLimit { .. }
        ))
    ));

    assert!(stats.cumulative_local_when_bad_compilations() > 0);
    let mut one_below = exact;
    one_below.max_cumulative_local_when_bad_compilations =
        stats.cumulative_local_when_bad_compilations() - 1;
    assert!(matches!(
        GeneratedSectorAffineEffectiveCoverageCompiler::compile(
            &family,
            &context,
            inventory,
            baseline.config(),
            one_below,
        ),
        Err(GeneratedSectorAffineEffectiveCoverageError::GroupEffective(
            _
        ))
    ));
}
