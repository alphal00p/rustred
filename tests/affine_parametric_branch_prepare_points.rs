//! Natural generated-sunset validation for Boolean-branch affine ordering.
//! Loop count appears only in this concrete integration oracle.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::{
    AffineDenominator, AffineParametricOrderingError, AffineParametricOrderingLimits,
    AffinePreparePointError, AffinePreparePointLayer, AffinePreparePointLimits,
    AffinePreparePointScheduleCertificate, AffinePreparePointScheduleError,
    AffinePreparePointScheduleLimits, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, AffineStartSourceKind, CoefficientContext,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpGenerator,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError,
    ResidualAffineBranchSystemLimits, ResidualAffineBranchSystemOutcome,
    ResidualProductLocusBooleanCoverCertificate, ResidualProductLocusBooleanCoverCompiler,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanNodeOutcome, SectorMask,
};
use symbolica::prelude::Integer;

fn sunset(name: &str) -> IntegralFamily {
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

fn generated_cover(
    bits: &str,
) -> (
    IntegralFamily,
    ParametricCoefficientContext,
    Arc<ResidualProductLocusBooleanCoverCertificate>,
) {
    let family = sunset(&format!("affine-ordering-branch-sunset-{bits}"));
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
    assert_eq!(queue.work_items().len(), 1, "sector {bits}");
    let cover = Arc::new(
        ResidualProductLocusBooleanCoverCompiler::compile(
            &family,
            &context,
            queue,
            0,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap(),
    );
    (family, context, cover)
}

fn authority<'a>(
    family: &'a IntegralFamily,
    context: &'a ParametricCoefficientContext,
    cover: &'a Arc<ResidualProductLocusBooleanCoverCertificate>,
) -> AffineStartReplayAuthority<'a> {
    AffineStartReplayAuthority::ResidualBooleanBranch {
        family,
        context,
        cover,
    }
}

fn exercise_sector(bits: &str, negative_replay_checks: bool, require_nonzero_guard: bool) {
    let mut all_identities = BTreeSet::new();
    let mut exercised_guarded_terminal = false;
    let mut checked_negative_replay = false;

    let (family, context, cover) = generated_cover(bits);
    let ready_ordinals: Vec<_> = cover
        .nodes()
        .iter()
        .filter(|node| {
            matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )
        })
        .map(|node| node.ordinal())
        .collect();
    assert!(!ready_ordinals.is_empty(), "sector {bits}");
    let ready_count = ready_ordinals.len();

    for terminal_ordinal in ready_ordinals {
        let branch = Arc::new(
            ResidualAffineBranchSystemCertificate::compile(
                &family,
                &context,
                cover.clone(),
                terminal_ordinal,
                ResidualAffineBranchSystemLimits::default(),
            )
            .unwrap(),
        );
        assert!(matches!(
            branch.outcome(),
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ));
        exercised_guarded_terminal |= !branch.nonzero_guard_locus_ordinals().is_empty();

        let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
            &family,
            &context,
            cover.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            branch.clone(),
            AffineParametricOrderingLimits::default(),
        )
        .unwrap();
        assert_eq!(
            ordering.source_kind(),
            AffineStartSourceKind::ResidualBooleanBranch
        );
        assert!(Arc::ptr_eq(ordering.residual_branch().unwrap(), &branch));
        assert_eq!(
            ordering.uncomposed_nonzero_guard_locus_ordinals(),
            branch.nonzero_guard_locus_ordinals()
        );
        assert!(all_identities.insert(ordering.stable_manifest().to_owned()));

        let map = branch.affine_map().unwrap();
        let geometry = ordering.geometry();
        assert_eq!(geometry.ambient_arity(), map.ambient_arity());
        assert_eq!(geometry.free_positions(), map.free_positions());
        for row in 0..map.ambient_arity() {
            assert_eq!(geometry.constant(row), map.constant(row));
            for (free_ordinal, &ambient_column) in map.free_positions().iter().enumerate() {
                assert_eq!(
                    geometry.linear_coefficient(row, free_ordinal),
                    map.linear_coefficient(row, ambient_column)
                );
            }
        }

        assert!(matches!(
            ordering.replay(&context),
            Err(AffineParametricOrderingError::BranchReplayAuthorityRequired)
        ));
        ordering
            .replay_with_authority(authority(&family, &context, &cover))
            .unwrap();

        let layer = AffinePreparePointLayer::compile_with_authority(
            authority(&family, &context, &cover),
            ordering.clone(),
            1,
            AffinePreparePointLimits::default(),
        )
        .unwrap();
        layer
            .replay_with_authority(authority(&family, &context, &cover))
            .unwrap();
        assert!(Arc::ptr_eq(
            layer.ordering().residual_branch().unwrap(),
            &branch
        ));
        assert_eq!(
            layer.ordering().uncomposed_nonzero_guard_locus_ordinals(),
            branch.nonzero_guard_locus_ordinals()
        );
        for shift in layer.ordered_translations() {
            assert_eq!(
                shift
                    .values()
                    .iter()
                    .map(|value| value.unsigned_abs())
                    .sum::<u64>(),
                1
            );
            for &position in ordering.constant_positions() {
                let mut shifted = ordering.constant_start_value(position).unwrap().clone();
                shifted += Integer::from(shift.values()[position]);
                assert_eq!(
                    shifted >= Integer::from(1),
                    ordering.sector().active_bits()[position]
                );
            }
        }

        let schedule = AffinePreparePointScheduleCertificate::compile_with_authority(
            authority(&family, &context, &cover),
            ordering.clone(),
            1,
            AffinePreparePointScheduleLimits::default(),
        )
        .unwrap();
        schedule
            .replay_with_authority(authority(&family, &context, &cover))
            .unwrap();
        assert_eq!(schedule.layers().len(), 2);
        assert!(Arc::ptr_eq(
            schedule.ordering().residual_branch().unwrap(),
            &branch
        ));
        assert_eq!(
            schedule
                .ordering()
                .uncomposed_nonzero_guard_locus_ordinals(),
            branch.nonzero_guard_locus_ordinals()
        );

        if negative_replay_checks && !checked_negative_replay {
            assert!(matches!(
                AffinePreparePointLayer::compile(
                    &context,
                    ordering.clone(),
                    1,
                    AffinePreparePointLimits::default(),
                ),
                Err(AffinePreparePointError::Ordering(
                    AffineParametricOrderingError::BranchReplayAuthorityRequired
                ))
            ));
            assert!(matches!(
                AffinePreparePointScheduleCertificate::compile(
                    &context,
                    ordering.clone(),
                    1,
                    AffinePreparePointScheduleLimits::default(),
                ),
                Err(AffinePreparePointScheduleError::Ordering(
                    AffineParametricOrderingError::BranchReplayAuthorityRequired
                ))
            ));
            let foreign_family = sunset("affine-ordering-branch-foreign-family");
            assert!(matches!(
                ordering.replay_with_authority(authority(&foreign_family, &context, &cover)),
                Err(AffineParametricOrderingError::Branch(
                    ResidualAffineBranchSystemError::WrongFamily
                ))
            ));
            assert!(matches!(
                layer.replay_with_authority(authority(&foreign_family, &context, &cover)),
                Err(AffinePreparePointError::Ordering(
                    AffineParametricOrderingError::Branch(
                        ResidualAffineBranchSystemError::WrongFamily
                    )
                ))
            ));
            assert!(matches!(
                schedule.replay_with_authority(authority(&foreign_family, &context, &cover)),
                Err(AffinePreparePointScheduleError::Ordering(
                    AffineParametricOrderingError::Branch(
                        ResidualAffineBranchSystemError::WrongFamily
                    )
                ))
            ));
            let foreign_context = ParametricCoefficientContext::try_new(
                family.coefficient_context(),
                "affine-ordering-branch-foreign-context",
                context.index_count(),
            )
            .unwrap();
            assert!(matches!(
                ordering.replay_with_authority(authority(&family, &foreign_context, &cover)),
                Err(AffineParametricOrderingError::WrongContext)
                    | Err(AffineParametricOrderingError::Branch(
                        ResidualAffineBranchSystemError::WrongContext
                    ))
            ));
            assert!(matches!(
                layer.replay_with_authority(authority(&family, &foreign_context, &cover)),
                Err(AffinePreparePointError::Ordering(
                    AffineParametricOrderingError::WrongContext
                        | AffineParametricOrderingError::Branch(
                            ResidualAffineBranchSystemError::WrongContext
                        )
                ))
            ));
            assert!(matches!(
                schedule.replay_with_authority(authority(&family, &foreign_context, &cover)),
                Err(AffinePreparePointScheduleError::Ordering(
                    AffineParametricOrderingError::WrongContext
                        | AffineParametricOrderingError::Branch(
                            ResidualAffineBranchSystemError::WrongContext
                        )
                ))
            ));

            let mut other_cover_limits = cover.limits();
            other_cover_limits.max_atoms += 1;
            let other_cover = Arc::new(
                ResidualProductLocusBooleanCoverCompiler::compile(
                    &family,
                    &context,
                    cover.source_queue().clone(),
                    cover.source_work_item_ordinal(),
                    other_cover_limits,
                )
                .unwrap(),
            );
            assert!(matches!(
                ordering.replay_with_authority(authority(&family, &context, &other_cover)),
                Err(AffineParametricOrderingError::Branch(
                    ResidualAffineBranchSystemError::SourceCoverMismatch
                ))
            ));
            assert!(matches!(
                layer.replay_with_authority(authority(&family, &context, &other_cover)),
                Err(AffinePreparePointError::Ordering(
                    AffineParametricOrderingError::Branch(
                        ResidualAffineBranchSystemError::SourceCoverMismatch
                    )
                ))
            ));
            assert!(matches!(
                schedule.replay_with_authority(authority(&family, &context, &other_cover)),
                Err(AffinePreparePointScheduleError::Ordering(
                    AffineParametricOrderingError::Branch(
                        ResidualAffineBranchSystemError::SourceCoverMismatch
                    )
                ))
            ));

            let identity_bytes = ordering.stats().map_identity_bytes();
            assert!(identity_bytes > 0);
            let mut exact_identity_limits = AffineParametricOrderingLimits::default();
            exact_identity_limits.max_map_identity_bytes = identity_bytes;
            let exact_identity_ordering =
                AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
                    &family,
                    &context,
                    cover.clone(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    branch.clone(),
                    exact_identity_limits,
                )
                .unwrap();
            assert_eq!(
                exact_identity_ordering.stats().map_identity_bytes(),
                identity_bytes
            );

            let mut one_below_identity_limits = exact_identity_limits;
            one_below_identity_limits.max_map_identity_bytes = identity_bytes - 1;
            assert!(matches!(
                AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
                    &family,
                    &context,
                    cover.clone(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    branch.clone(),
                    one_below_identity_limits,
                ),
                Err(AffineParametricOrderingError::ResourceLimit {
                    resource: "affine map identity bytes",
                    requested,
                    limit,
                }) if requested > limit && limit == identity_bytes - 1
            ));

            let detached_branch = Arc::new(branch.as_ref().clone());
            assert!(!Arc::ptr_eq(&branch, &detached_branch));
            let detached_ordering =
                AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
                    &family,
                    &context,
                    cover.clone(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    detached_branch,
                    AffineParametricOrderingLimits::default(),
                )
                .unwrap();
            assert_eq!(ordering, detached_ordering);
            checked_negative_replay = true;
        }
    }

    assert_eq!(all_identities.len(), ready_count);
    if require_nonzero_guard {
        assert!(exercised_guarded_terminal);
    }
    if negative_replay_checks {
        assert!(checked_negative_replay);
    }
}

#[test]
fn natural_sunset_011_branch_prepare_points() {
    exercise_sector("011", false, false);
}

#[test]
fn natural_sunset_101_branch_prepare_points() {
    exercise_sector("101", false, false);
}

#[test]
fn natural_sunset_110_branch_prepare_points() {
    exercise_sector("110", false, false);
}

#[test]
fn natural_sunset_111_branch_prepare_points_and_replay_boundaries() {
    exercise_sector("111", true, true);
}
