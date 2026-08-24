use std::sync::Arc;

use crate::generated_sector_affine_effective_coverage::{
    GeneratedSectorAffineEffectiveCoverageCertificate,
    GeneratedSectorAffineEffectiveCoverageCompiler, GeneratedSectorAffineEffectiveCoverageConfig,
    GeneratedSectorAffineEffectiveCoverageLimits, GeneratedSectorAffineExceptionalChildLocator,
    GeneratedSectorAffineOrderedChildOutput, GeneratedSectorAffineResidualRootLocator,
    GeneratedSectorAffineTerminalDisposition,
};
use crate::generated_sector_affine_effective_residual_queue::{
    GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA,
    GeneratedSectorAffineEffectiveResidualAtomPolarity,
    GeneratedSectorAffineEffectiveResidualQueueCompiler,
    GeneratedSectorAffineEffectiveResidualQueueError,
    GeneratedSectorAffineEffectiveResidualQueueLimits,
    GeneratedSectorAffineEffectiveResidualQueuePointDisposition,
    GeneratedSectorAffineEffectiveResidualQueuePointLimits,
    GeneratedSectorAffineEffectiveResidualSourceView,
    GeneratedSectorAffineEffectiveResidualSourceViewError,
    GeneratedSectorAffineEffectiveResidualTargetSourceView,
    GeneratedSectorAffineEffectiveResidualWorkLocator,
};
use crate::{
    AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
    GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator, SectorMask,
};

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

fn owner_fixture(
    name: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
) {
    owner_fixture_for_sector(name, "001")
}

fn owner_fixture_for_sector(
    name: &str,
    sector: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
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
        SectorMask::try_from_bit_string(sector).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let source_queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let inventory = Arc::new(
        GeneratedResidualAffineCaseInventoryCompiler::compile(
            &family,
            &context,
            source_queue,
            GeneratedResidualAffineCaseInventoryLimits::default(),
        )
        .unwrap(),
    );
    let owner = GeneratedSectorAffineEffectiveCoverageCompiler::compile(
        &family,
        &context,
        inventory,
        GeneratedSectorAffineEffectiveCoverageConfig::new(0),
        GeneratedSectorAffineEffectiveCoverageLimits::default(),
    )
    .unwrap();
    (family, context, Arc::new(owner))
}

fn assert_target_projection(
    owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    target: GeneratedSectorAffineEffectiveResidualTargetSourceView<'_>,
) {
    let source = &owner.inventory().cases()[target.case_ordinal()];
    let group = &owner.inventory().groups()[target.group_ordinal()];
    assert_eq!(target.source_locator(), source.locator());
    assert_eq!(target.terminal().source_locator(), source.locator());
    assert_eq!(target.group_ordinal(), source.group_ordinal());
    assert_eq!(target.ordinal_within_group(), source.ordinal_within_group());
    assert_eq!(target.anchor_case_ordinal(), group.anchor_case_ordinal());
    assert!(std::ptr::eq(target.affine_map(), source.affine_map()));
    assert_eq!(
        target.guard_entries().as_ptr(),
        source.guard_composition().entries().as_ptr()
    );
    assert_eq!(target.guard_entry_count(), target.guard_entries().len());
    for (position, expected) in target.guard_entries().iter().enumerate() {
        assert!(std::ptr::eq(
            target.guard_entry(position).unwrap(),
            expected
        ));
    }
    assert!(target.guard_entry(target.guard_entry_count()).is_none());
    assert_eq!(target.constants(), source.constants());
    assert_eq!(target.constant_count(), target.constants().len());
    for (position, expected) in target.constants().iter().enumerate() {
        assert!(std::ptr::eq(target.constant(position).unwrap(), expected));
    }
    assert!(target.constant(target.constant_count()).is_none());
    assert_eq!(target.free_positions(), group.free_positions());
    assert_eq!(target.free_position_count(), target.free_positions().len());
    for (position, &expected) in target.free_positions().iter().enumerate() {
        assert_eq!(target.free_position(position), Some(expected));
    }
    assert!(target.free_position(target.free_position_count()).is_none());
    assert_eq!(
        target.free_positions(),
        source.affine_map().free_positions()
    );
}

#[test]
fn generated_001_effective_residual_queue_is_exact_bounded_and_point_complete() {
    let (family, context, owner) = owner_fixture("effective-residual-queue-generated-001");
    let queue = GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
        &family,
        &context,
        Arc::clone(&owner),
        GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
    )
    .unwrap();

    assert_eq!(
        queue.schema(),
        GENERATED_SECTOR_AFFINE_EFFECTIVE_RESIDUAL_QUEUE_V1_SCHEMA
    );
    assert!(Arc::ptr_eq(queue.owner(), &owner));
    assert_eq!(queue.len(), owner.stats().residual_locators());
    assert_eq!(queue.stats().work_items(), queue.len());
    assert_eq!(
        queue.stats().owner_authority_retained_bytes(),
        owner.stats().outer_retained_bytes()
    );
    assert!(queue.stats().retained_bytes() > owner.stats().outer_retained_bytes());
    assert_eq!(queue.is_empty(), queue.len() == 0);
    assert_eq!(
        queue.limits(),
        GeneratedSectorAffineEffectiveResidualQueueLimits::default()
    );
    for rendered in [
        format!("{queue:?}"),
        format!("{:?}", queue.stats()),
        format!("{:?}", queue.work_items().first().unwrap()),
    ] {
        assert!(rendered.contains("<redacted>"));
        for private in [
            "terminal_record_ordinal",
            "child_output_ordinal",
            "private_predicate",
            "ParametricRelation",
            "split_recentered_relation",
            "polynomial",
            "pullback",
        ] {
            assert!(!rendered.contains(private), "leaked {private}: {rendered}");
        }
    }

    let mut unsupported_source_views = 0usize;
    let mut unprocessed_source_views = 0usize;
    let mut unconsumed_source_views = 0usize;
    let mut exceptional_source_views = 0usize;
    for (work_item_ordinal, item) in queue.work_items().iter().enumerate() {
        let view = queue
            .authenticated_source_view(work_item_ordinal)
            .expect("every retained queue item has one exact borrowed source");
        let terminal = view.terminal();
        assert_eq!(terminal.work_item_ordinal(), work_item_ordinal);
        assert_eq!(terminal.locator(), item.locator());
        let retained_terminal_ordinal = owner
            .inventory()
            .terminals()
            .iter()
            .position(|retained| {
                retained.locator() == terminal.source_locator()
                    && retained.outcome() == terminal.source_outcome()
            })
            .expect("the scalar projection resolves one retained source terminal");
        assert_eq!(
            owner.terminal_records()[retained_terminal_ordinal].source_locator(),
            terminal.source_locator()
        );
        assert_eq!(
            owner.terminal_records()[retained_terminal_ordinal].source_outcome(),
            terminal.source_outcome()
        );

        match view {
            GeneratedSectorAffineEffectiveResidualSourceView::UnsupportedInventoryTerminal(
                unsupported,
            ) => {
                unsupported_source_views += 1;
                let retained = &owner.inventory().terminals()[retained_terminal_ordinal];
                let branch = retained
                    .source_branch()
                    .expect("an unsupported terminal retains its affine branch");
                assert_eq!(
                    unsupported.ready_terminal_ordinal(),
                    unsupported.terminal().source_locator().terminal_ordinal()
                );
                assert_eq!(
                    unsupported.nonzero_locus_ordinals(),
                    branch.nonzero_guard_locus_ordinals()
                );
                let structural_locus_count = owner
                    .source_queue()
                    .discovery()
                    .coverage()
                    .structural_loci()
                    .len();
                for &locus in unsupported
                    .equal_zero_locus_ordinals()
                    .iter()
                    .chain(unsupported.nonzero_locus_ordinals())
                {
                    assert!(locus < structural_locus_count);
                    assert!(unsupported.polynomial_for_locus_ordinal(locus).is_some());
                }
                if let Some(unauthorized) = (0..structural_locus_count).find(|ordinal| {
                    unsupported
                        .equal_zero_locus_ordinals()
                        .binary_search(ordinal)
                        .is_err()
                        && unsupported
                            .nonzero_locus_ordinals()
                            .binary_search(ordinal)
                            .is_err()
                }) {
                    assert!(
                        unsupported
                            .polynomial_for_locus_ordinal(unauthorized)
                            .is_none(),
                        "an in-range locus outside the authenticated manifests is not projected"
                    );
                }
                for (polarity, expected_ordinals) in [
                    (
                        GeneratedSectorAffineEffectiveResidualAtomPolarity::EqualZero,
                        unsupported.equal_zero_locus_ordinals(),
                    ),
                    (
                        GeneratedSectorAffineEffectiveResidualAtomPolarity::NonZero,
                        unsupported.nonzero_locus_ordinals(),
                    ),
                ] {
                    assert_eq!(unsupported.atom_count(polarity), expected_ordinals.len());
                    for (position, &expected_ordinal) in expected_ordinals.iter().enumerate() {
                        let atom = unsupported.atom(polarity, position).unwrap();
                        assert_eq!(atom.locus_ordinal(), expected_ordinal);
                        assert!(std::ptr::eq(
                            atom.polynomial(),
                            &owner
                                .source_queue()
                                .discovery()
                                .coverage()
                                .structural_loci()[expected_ordinal]
                        ));
                        let rendered = format!("{atom:?}");
                        assert!(rendered.contains("<redacted>"));
                        assert!(!rendered.contains("polynomial:"));
                    }
                    assert!(
                        unsupported
                            .atom(polarity, unsupported.atom_count(polarity))
                            .is_none()
                    );
                }
                assert!(!unsupported.unsupported_reasons().is_empty());
                assert_eq!(
                    unsupported.unsupported_reason_count(),
                    unsupported.unsupported_reasons().len()
                );
                for (position, expected) in unsupported.unsupported_reasons().iter().enumerate() {
                    assert!(std::ptr::eq(
                        unsupported.unsupported_reason(position).unwrap(),
                        expected
                    ));
                }
                assert!(
                    unsupported
                        .unsupported_reason(unsupported.unsupported_reason_count())
                        .is_none()
                );
            }
            GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(target) => {
                unprocessed_source_views += 1;
                assert_target_projection(&owner, target);
            }
            GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(target) => {
                unconsumed_source_views += 1;
                assert_target_projection(&owner, target);
            }
            GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalDomain(exceptional)
            | GeneratedSectorAffineEffectiveResidualSourceView::ExceptionalLeak(exceptional) => {
                exceptional_source_views += 1;
                let relative = exceptional.exceptional();
                assert_eq!(
                    relative.leaf_ordinal(),
                    match item.locator() {
                        GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(locator) =>
                            locator.leaf_ordinal,
                        GeneratedSectorAffineEffectiveResidualWorkLocator::Root(_) => {
                            panic!("an exceptional source must retain an exceptional locator")
                        }
                    }
                );
                assert_eq!(
                    relative.predicates().as_ptr(),
                    relative.relative_case().predicates().as_ptr()
                );
                assert_eq!(exceptional.predicate_count(), relative.predicates().len());
                for (position, expected) in relative.predicates().iter().enumerate() {
                    let predicate = exceptional.predicate(position).unwrap();
                    assert_eq!(predicate.locus_ordinal(), expected.locus_ordinal());
                    assert_eq!(predicate.kind(), expected.kind());
                    assert!(std::ptr::eq(predicate.polynomial(), expected.polynomial()));
                    let rendered = format!("{predicate:?}");
                    assert!(rendered.contains("<redacted>"));
                    assert!(!rendered.contains("polynomial:"));
                }
                assert!(
                    exceptional
                        .predicate(exceptional.predicate_count())
                        .is_none()
                );
                assert_target_projection(&owner, exceptional.target());
            }
        }

        let rendered = format!("{view:?}");
        assert!(rendered.contains("<redacted>"));
        for private in [
            "ParametricRelation",
            "split_recentered_relation",
            "private_predicate split",
            "polynomial:",
            "InventoryTerminal",
            "InventoryCase",
            "BooleanCoverCertificate",
            "source_branch",
            "guard_composition",
        ] {
            assert!(!rendered.contains(private), "leaked {private}: {rendered}");
        }
    }
    assert!(exceptional_source_views > 0);
    assert_eq!(
        unsupported_source_views,
        owner.stats().unsupported_residual_roots()
    );
    assert_eq!(
        unprocessed_source_views,
        owner.stats().unprocessed_actionable_roots()
    );
    assert_eq!(
        unconsumed_source_views,
        owner.stats().unconsumed_target_roots()
    );
    assert_eq!(
        exceptional_source_views,
        owner.stats().exceptional_child_locators()
    );
    assert_eq!(
        unsupported_source_views
            + unprocessed_source_views
            + unconsumed_source_views
            + exceptional_source_views,
        queue.len()
    );
    assert!(matches!(
        queue.authenticated_source_view(queue.len()),
        Err(GeneratedSectorAffineEffectiveResidualSourceViewError::WorkItemOutOfRange)
    ));

    let mut expected = Vec::new();
    for terminal in owner.terminal_records() {
        match terminal.disposition() {
            GeneratedSectorAffineTerminalDisposition::ProvedEmpty => {}
            GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator) => {
                expected.push(GeneratedSectorAffineEffectiveResidualWorkLocator::Root(
                    locator,
                ));
            }
            GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                first_child_output_ordinal,
                child_output_count,
                ..
            } => {
                for child in &owner.ordered_child_outputs()
                    [first_child_output_ordinal..first_child_output_ordinal + child_output_count]
                {
                    if let GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) = child {
                        expected.push(
                            GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(
                                *locator,
                            ),
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        queue
            .work_items()
            .iter()
            .map(|item| item.locator())
            .collect::<Vec<_>>(),
        expected
    );

    // Replay compares against the retained items in place: neither the output
    // allocation nor its capacity changes.
    let before_ptr = queue.work_items().as_ptr();
    let before_capacity = queue.work_items().len();
    queue.replay(&family, &context).unwrap();
    assert_eq!(queue.work_items().as_ptr(), before_ptr);
    assert_eq!(queue.work_items().len(), before_capacity);

    // A generated applicable leaf has been consumed into a rule and is not
    // work for the next epoch.
    let rule = queue
        .classification_for_indices(
            &family,
            &context,
            &[-4, -4, 2],
            GeneratedSectorAffineEffectiveResidualQueuePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        rule.disposition(),
        GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Excluded
    );
    assert_eq!(rule.work_item_scans(), 0);
    assert!(rule.owner_stats().global_cases() > 0);

    // The adjacent generated exceptional leaf is retained exactly once.
    let exceptional_locator = GeneratedSectorAffineExceptionalChildLocator {
        group_pass_ordinal: 0,
        accepted_attempt_ordinal: 0,
        leaf_ordinal: 0,
    };
    let mut exact_point_limits = GeneratedSectorAffineEffectiveResidualQueuePointLimits::default();
    exact_point_limits.max_work_item_scans = queue.len();
    let exceptional = queue
        .classification_for_indices(&family, &context, &[-4, -4, 1], exact_point_limits)
        .unwrap();
    let expected_locator =
        GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(exceptional_locator);
    match exceptional.disposition() {
        GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Work {
            work_item_ordinal,
            locator,
        } => {
            assert_eq!(locator, expected_locator);
            assert_eq!(queue.work_items()[work_item_ordinal].locator(), locator);
        }
        GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Excluded => {
            panic!("generated exceptional point must remain in the effective queue")
        }
    }
    assert_eq!(exceptional.work_item_scans(), queue.len());

    let mut one_below_point = GeneratedSectorAffineEffectiveResidualQueuePointLimits::default();
    one_below_point.max_work_item_scans = queue.len() - 1;
    assert!(matches!(
        queue.classification_for_indices(&family, &context, &[-4, -4, 1], one_below_point,),
        Err(GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit { .. })
    ));

    let outside = queue
        .classification_for_indices(
            &family,
            &context,
            &[-4, -4, 0],
            GeneratedSectorAffineEffectiveResidualQueuePointLimits::default(),
        )
        .unwrap();
    assert_eq!(
        outside.disposition(),
        GeneratedSectorAffineEffectiveResidualQueuePointDisposition::Excluded
    );
    assert_eq!(outside.work_item_scans(), 0);

    // The exact construction envelope succeeds. Each positive aggregate
    // counter is independently rejected one below by the compiler's O(1)
    // declared-census preflight, before any owner replay or linear scan.
    let stats = queue.stats();
    let exact_limits = GeneratedSectorAffineEffectiveResidualQueueLimits {
        max_owner_replays: stats.owner_replays(),
        max_terminal_record_visits: stats.terminal_record_visits(),
        max_ordered_child_output_visits: stats.ordered_child_output_visits(),
        max_authority_index_comparison_bound: stats.authority_index_comparison_bound(),
        max_projection_payload_comparison_bound: stats.projection_payload_comparison_bound(),
        max_work_items: stats.work_items(),
        max_retained_bytes: stats.retained_bytes(),
        max_temporary_bytes: stats.temporary_bytes(),
        max_peak_visible_bytes: stats.peak_visible_bytes(),
    };
    let exact = GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
        &family,
        &context,
        Arc::clone(&owner),
        exact_limits,
    )
    .unwrap();
    assert_eq!(exact.stats(), stats);

    macro_rules! rejects_one_below {
        ($field:ident, $getter:ident) => {{
            let value = stats.$getter();
            assert!(value > 0, concat!(stringify!($getter), " must be positive"));
            let mut one_below = exact_limits;
            one_below.$field = value - 1;
            assert!(matches!(
                GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
                    &family,
                    &context,
                    Arc::clone(&owner),
                    one_below,
                ),
                Err(GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit { .. })
            ));
        }};
    }
    rejects_one_below!(max_owner_replays, owner_replays);
    rejects_one_below!(max_terminal_record_visits, terminal_record_visits);
    rejects_one_below!(max_ordered_child_output_visits, ordered_child_output_visits);
    rejects_one_below!(
        max_authority_index_comparison_bound,
        authority_index_comparison_bound
    );
    rejects_one_below!(
        max_projection_payload_comparison_bound,
        projection_payload_comparison_bound
    );
    rejects_one_below!(max_work_items, work_items);
    rejects_one_below!(max_retained_bytes, retained_bytes);
    rejects_one_below!(max_temporary_bytes, temporary_bytes);
    rejects_one_below!(max_peak_visible_bytes, peak_visible_bytes);

    let mut corrupted = queue.clone();
    assert!(corrupted.test_only_corrupt_first_authority());
    assert!(matches!(
        corrupted.authenticated_source_view(0),
        Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)
    ));
    assert!(matches!(
        corrupted.replay(&family, &context),
        Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch)
    ));

    let mut corrupted = queue.clone();
    let corrupted_projection = corrupted
        .test_only_corrupt_first_projection_witness()
        .expect("the nonempty queue has one private projection witness");
    assert!(matches!(
        corrupted.authenticated_source_view(corrupted_projection),
        Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)
    ));
    assert!(matches!(
        corrupted.replay(&family, &context),
        Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch)
    ));

    let first_exceptional = queue
        .work_items()
        .iter()
        .position(|item| {
            matches!(
                item.locator(),
                GeneratedSectorAffineEffectiveResidualWorkLocator::Exceptional(_)
            )
        })
        .unwrap();
    macro_rules! rejects_corrupted_exceptional_index {
        ($method:ident) => {{
            let mut corrupted = queue.clone();
            assert!(corrupted.$method());
            assert!(matches!(
                corrupted.authenticated_source_view(first_exceptional),
                Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)
            ));
            assert!(matches!(
                corrupted.replay(&family, &context),
                Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch)
            ));
        }};
    }
    rejects_corrupted_exceptional_index!(
        test_only_corrupt_first_exceptional_target_disposition_index
    );
    rejects_corrupted_exceptional_index!(test_only_corrupt_first_exceptional_attempt_index);
    rejects_corrupted_exceptional_index!(test_only_corrupt_first_exceptional_selected_position);
    rejects_corrupted_exceptional_index!(test_only_corrupt_first_exceptional_residual_index);
}

#[test]
fn generated_011_unconsumed_sources_use_direct_authenticated_indices() {
    let (family, context, owner) =
        owner_fixture_for_sector("effective-residual-queue-generated-011", "011");
    let queue = GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
        &family,
        &context,
        owner.clone(),
        GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
    )
    .unwrap();
    let unconsumed = queue
        .work_items()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, item)| match item.locator() {
            GeneratedSectorAffineEffectiveResidualWorkLocator::Root(
                GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot { .. },
            ) => Some(ordinal),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(unconsumed.len(), 2);
    for &ordinal in &unconsumed {
        let GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(target) =
            queue.authenticated_source_view(ordinal).unwrap()
        else {
            panic!("an unconsumed locator must resolve to an unconsumed source")
        };
        assert_target_projection(&owner, target);
    }

    macro_rules! rejects_corrupted_unconsumed_index {
        ($method:ident) => {{
            let mut corrupted = queue.clone();
            assert!(corrupted.$method());
            assert!(matches!(
                corrupted.authenticated_source_view(unconsumed[0]),
                Err(GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch)
            ));
            assert!(matches!(
                corrupted.replay(&family, &context),
                Err(GeneratedSectorAffineEffectiveResidualQueueError::ReplayMismatch)
            ));
        }};
    }
    rejects_corrupted_unconsumed_index!(
        test_only_corrupt_first_unconsumed_target_disposition_index
    );
    rejects_corrupted_unconsumed_index!(test_only_corrupt_first_unconsumed_residual_index);
}

#[test]
fn effective_residual_queue_debug_and_errors_redact_private_authority() {
    const SENTINEL: &str = "private_predicate split_recentered_relation";
    let error = GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit {
        resource: SENTINEL,
        requested: 2,
        limit: 1,
    };
    let rendered = format!("{error} {error:?}");
    assert!(!rendered.contains("private_predicate"));
    assert!(!rendered.contains("split_recentered_relation"));

    let source_error = GeneratedSectorAffineEffectiveResidualSourceViewError::AuthorityMismatch;
    let rendered = format!("{source_error} {source_error:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("private_predicate"));
    assert!(!rendered.contains("split_recentered_relation"));
    assert!(std::error::Error::source(&source_error).is_none());

    let exceptional_error =
        GeneratedSectorAffineEffectiveResidualSourceViewError::ExceptionalAuthenticationFailed;
    let rendered = format!("{exceptional_error} {exceptional_error:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("leaf_ordinal"));
    assert!(!rendered.contains("private_predicate"));
    assert!(std::error::Error::source(&exceptional_error).is_none());
}
