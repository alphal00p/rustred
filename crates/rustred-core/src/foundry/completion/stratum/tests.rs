use std::sync::Arc;

use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::artifact::{
    derive_k6_terminal_authority, derive_one_loop_unit_mass_tadpole,
    derive_two_loop_unit_mass_sunset, fresh_k6_terminal_authority_for_test,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::frame::{OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan};
use super::{
    CampaignStratumAnchor, DecoratedStratum, ForbiddenColumnReason, GuardBranch,
    GuardBranchIdentity, GuardPredicateAuthority, ImmutableOwnerKind, ImmutableOwnerSnapshot,
    MaximalStratumAnchor, ProspectiveColumnKind, StratumRegistryError, StratumRegistryLimits,
    TargetColumnPartition,
};

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_frame(degree: usize) -> (crate::foundry::artifact::ClosedArtifact, PhysicalFramePlan) {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let frame = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        degree,
        PhysicalFrameLimits::default(),
    )
    .unwrap()
    .into_plan();
    (artifact, frame)
}

fn two_loop_frames(degrees: &[usize]) -> Vec<PhysicalFramePlan> {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, true, true]).unwrap();
    degrees
        .iter()
        .map(|&degree| {
            OneSidedChartFrame::try_new(
                &generator,
                &completed,
                sector.clone(),
                degree,
                PhysicalFrameLimits::default(),
            )
            .unwrap()
            .into_plan()
        })
        .collect()
}

fn zero_shift_target(frame: &PhysicalFramePlan) -> usize {
    frame
        .columns()
        .iter()
        .position(|shift| shift.values().iter().all(|&value| value == 0))
        .expect("ordinary vacuum frame must retain the zero integral shift")
}

fn maximal_stratum(frame: &PhysicalFramePlan, target: usize) -> DecoratedStratum {
    let physical_shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        frame.sector().clone(),
        frame.columns()[target].values(),
        &physical_shifts,
    )
    .unwrap();
    DecoratedStratum::try_guard_blind(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

#[test]
fn restricted_campaign_sequence_materializes_then_shrinks_without_releasing_singletons_or_guards() {
    let frames = two_loop_frames(&[0, 1]);
    let first_target = zero_shift_target(&frames[0]);
    let second_target = zero_shift_target(&frames[1]);
    let first_maximal = maximal_stratum(&frames[0], first_target);
    let second_maximal = maximal_stratum(&frames[1], second_target);
    let shrinking_position = first_maximal
        .domain()
        .bounds()
        .iter()
        .zip(second_maximal.domain().bounds())
        .position(|(&first, &second)| {
            second.lower() > first.lower() || second.upper() < first.upper()
        })
        .expect("the degree-one frame must tighten a carrier endpoint");
    let singleton_position = (0..first_maximal.domain().arity())
        .find(|&position| position != shrinking_position)
        .unwrap();
    let mut initial_bounds = first_maximal.domain().bounds().to_vec();
    initial_bounds[singleton_position] = InteriorBounds::new(1, 1);
    let physical_shifts = frames[0]
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let initial_domain = SectorMonotoneDomain::try_new_for_rule(
        frames[0].sector().clone(),
        initial_bounds,
        frames[0].columns()[first_target].values(),
        &physical_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let guard = GuardBranchIdentity::try_new("restricted", GuardBranch::NonZero, limits).unwrap();
    let initial = DecoratedStratum::try_new(
        frames[0].family_fingerprint(),
        frames[0].context_fingerprint(),
        initial_domain,
        [guard.clone()],
        limits,
    )
    .unwrap();
    let mut sequence = CampaignStratumAnchor::try_restricted(initial.clone(), limits)
        .unwrap()
        .into_sequence();

    assert_eq!(sequence.scope(), &initial);
    let first = sequence
        .try_materialize(&frames[0], first_target, limits)
        .unwrap();
    assert_eq!(first, initial);

    let second = sequence
        .try_materialize(&frames[1], second_target, limits)
        .unwrap();
    assert_eq!(second.guards(), &[guard]);
    assert_eq!(
        second.domain().bounds()[singleton_position],
        InteriorBounds::new(1, 1)
    );
    assert!(
        second.domain().bounds()[shrinking_position].lower()
            > first.domain().bounds()[shrinking_position].lower()
            || second.domain().bounds()[shrinking_position].upper()
                < first.domain().bounds()[shrinking_position].upper()
    );
    for (&before, &after) in first.domain().bounds().iter().zip(second.domain().bounds()) {
        assert!(before.lower() <= after.lower());
        assert!(after.upper() <= before.upper());
    }
}

#[test]
fn restricted_campaign_empty_intersection_is_transactional_and_can_be_retried() {
    let frames = two_loop_frames(&[0, 1]);
    let first_target = zero_shift_target(&frames[0]);
    let second_target = zero_shift_target(&frames[1]);
    let first_maximal = maximal_stratum(&frames[0], first_target);
    let second_maximal = maximal_stratum(&frames[1], second_target);
    let (excluded_position, excluded_value) = first_maximal
        .domain()
        .bounds()
        .iter()
        .zip(second_maximal.domain().bounds())
        .enumerate()
        .find_map(|(position, (&first, &second))| {
            if second.lower() > first.lower() {
                Some((position, first.lower()))
            } else if second.upper() < first.upper() {
                Some((position, first.upper()))
            } else {
                None
            }
        })
        .expect("the degree-one frame must exclude a degree-zero endpoint");
    let mut initial_bounds = first_maximal.domain().bounds().to_vec();
    for bound in &mut initial_bounds {
        *bound = InteriorBounds::new(1, 1);
    }
    initial_bounds[excluded_position] = InteriorBounds::new(excluded_value, excluded_value);
    let physical_shifts = frames[0]
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let initial_domain = SectorMonotoneDomain::try_new_for_rule(
        frames[0].sector().clone(),
        initial_bounds,
        frames[0].columns()[first_target].values(),
        &physical_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let initial = DecoratedStratum::try_guard_blind(
        frames[0].family_fingerprint(),
        frames[0].context_fingerprint(),
        initial_domain,
        limits,
    )
    .unwrap();
    let mut sequence = CampaignStratumAnchor::try_restricted(initial.clone(), limits)
        .unwrap()
        .into_sequence();
    assert_eq!(
        sequence
            .try_materialize(&frames[0], first_target, limits)
            .unwrap(),
        initial
    );

    assert!(matches!(
        sequence.try_materialize(&frames[1], second_target, limits),
        Err(StratumRegistryError::Sector(_))
    ));
    assert_eq!(
        sequence
            .try_materialize(&frames[0], first_target, limits)
            .unwrap(),
        initial
    );
}

#[test]
fn campaign_maximal_lane_still_rejects_initial_mismatch_and_later_widening() {
    let frames = two_loop_frames(&[0, 1]);
    let first_target = zero_shift_target(&frames[0]);
    let second_target = zero_shift_target(&frames[1]);
    let limits = StratumRegistryLimits::default();

    let first_maximal = maximal_stratum(&frames[0], first_target);
    let mut tightened_bounds = first_maximal.domain().bounds().to_vec();
    let position = tightened_bounds
        .iter()
        .position(|bounds| bounds.lower() < bounds.upper())
        .unwrap();
    tightened_bounds[position] = InteriorBounds::new(
        tightened_bounds[position].lower(),
        tightened_bounds[position].upper() - 1,
    );
    let physical_shifts = frames[0]
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let tightened_domain = SectorMonotoneDomain::try_new_for_rule(
        frames[0].sector().clone(),
        tightened_bounds,
        frames[0].columns()[first_target].values(),
        &physical_shifts,
    )
    .unwrap();
    let tightened = DecoratedStratum::try_guard_blind(
        frames[0].family_fingerprint(),
        frames[0].context_fingerprint(),
        tightened_domain,
        limits,
    )
    .unwrap();
    let mut mismatched =
        CampaignStratumAnchor::from(MaximalStratumAnchor::try_new(tightened, limits).unwrap())
            .into_sequence();
    assert_eq!(
        mismatched
            .try_materialize(&frames[0], first_target, limits)
            .unwrap_err(),
        StratumRegistryError::InitialMaximalDomainMismatch
    );

    let second_maximal = maximal_stratum(&frames[1], second_target);
    let mut sequence = CampaignStratumAnchor::from(
        MaximalStratumAnchor::try_new(second_maximal.clone(), limits).unwrap(),
    )
    .into_sequence();
    assert_eq!(
        sequence
            .try_materialize(&frames[1], second_target, limits)
            .unwrap(),
        second_maximal
    );
    assert_eq!(
        sequence
            .try_materialize(&frames[0], first_target, limits)
            .unwrap_err(),
        StratumRegistryError::NonMonotoneMaximalDomain
    );
}

#[test]
fn decorated_guard_identity_is_canonical_and_rejects_ambiguous_branches() {
    let (_, frame) = one_loop_frame(0);
    let base = maximal_stratum(&frame, 0);
    let limits = StratumRegistryLimits::default();
    let nonzero = GuardBranchIdentity::try_new("g", GuardBranch::NonZero, limits).unwrap();
    let zero = GuardBranchIdentity::try_new("g", GuardBranch::Zero, limits).unwrap();
    let second = GuardBranchIdentity::try_new("h", GuardBranch::Zero, limits).unwrap();

    let ordered = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        base.domain().clone(),
        [second.clone(), nonzero.clone()],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(ordered.guards(), &[nonzero.clone(), second]);
    assert!(
        ordered
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert!(
        ordered
            .id()
            .as_str()
            .contains("1#g=external/nonzero,1#h=external/zero")
    );

    assert_eq!(
        DecoratedStratum::try_new(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            base.domain().clone(),
            [nonzero.clone(), nonzero],
            StratumRegistryLimits::default(),
        )
        .unwrap_err(),
        StratumRegistryError::DuplicateGuardPredicate {
            predicate: "g".to_owned(),
        }
    );
    assert_eq!(
        DecoratedStratum::try_new(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            base.domain().clone(),
            [
                zero,
                GuardBranchIdentity::try_new("g", GuardBranch::NonZero, limits).unwrap()
            ],
            StratumRegistryLimits::default(),
        )
        .unwrap_err(),
        StratumRegistryError::ContradictoryGuardPredicate {
            predicate: "g".to_owned(),
        }
    );

    let mut predicate_limits = limits;
    predicate_limits.max_guard_identity_bytes = 0;
    assert_eq!(
        GuardBranchIdentity::try_new("g", GuardBranch::NonZero, predicate_limits).unwrap_err(),
        StratumRegistryError::ResourceLimit {
            resource: "guard predicate identity bytes",
            requested: 1,
            limit: 0,
        }
    );

    let guard = GuardBranchIdentity::try_new("g", GuardBranch::NonZero, limits).unwrap();
    let mut iterator_limits = limits;
    iterator_limits.max_guard_branches = 2;
    assert_eq!(
        DecoratedStratum::try_new(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            base.domain().clone(),
            std::iter::repeat(guard),
            iterator_limits,
        )
        .unwrap_err(),
        StratumRegistryError::ResourceLimit {
            resource: "decorated-stratum guard branches",
            requested: 3,
            limit: 2,
        }
    );

    let mut identity_limits = limits;
    identity_limits.max_stratum_identity_bytes = 0;
    assert_eq!(
        DecoratedStratum::try_guard_blind(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            base.domain().clone(),
            identity_limits,
        )
        .unwrap_err(),
        StratumRegistryError::ResourceLimit {
            resource: "decorated-stratum identity bytes",
            requested: "rustred.decorated-stratum.v3:".len(),
            limit: 0,
        }
    );
}

#[test]
fn exact_guard_identity_uses_symbolica_primitive_associates_and_context_order() {
    let base = CoefficientContext::new(["d"]);
    let context = IndexedCoefficientContext::try_new(&base, "guard-associates", 1).unwrap();
    let n_plus_one = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let minus_two_times = context.mul(&context.integer(-2), &n_plus_one).unwrap();
    let positive = context
        .numerator_condition_with_limits(&n_plus_one, Default::default())
        .unwrap();
    let associate = context
        .numerator_condition_with_limits(&minus_two_times, Default::default())
        .unwrap();
    let limits = StratumRegistryLimits::default();
    let first = GuardBranchIdentity::try_from_indexed_polynomial(
        &context,
        &positive,
        GuardBranch::NonZero,
        Default::default(),
        limits,
    )
    .unwrap();
    let second = GuardBranchIdentity::try_from_indexed_polynomial(
        &context,
        &associate,
        GuardBranch::NonZero,
        Default::default(),
        limits,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.authority(),
        GuardPredicateAuthority::IndexedPolynomial
    );
    assert!(
        first
            .predicate()
            .starts_with("rustred.indexed-polynomial-guard.v1:")
    );

    let external_same_label = GuardBranchIdentity::try_new(
        first.predicate(),
        GuardBranch::Zero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(!first.same_predicate(&external_same_label));
    let (_, frame) = one_loop_frame(0);
    let base_stratum = maximal_stratum(&frame, 0);
    let authority_namespaced = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        base_stratum.domain().clone(),
        [first.clone(), external_same_label],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(authority_namespaced.guards().len(), 2);

    let foreign = IndexedCoefficientContext::try_new(&base, "guard-associates-foreign", 1).unwrap();
    let foreign_polynomial = foreign
        .numerator_condition_with_limits(&foreign.index(0).unwrap(), Default::default())
        .unwrap();
    let foreign_identity = GuardBranchIdentity::try_from_indexed_polynomial(
        &foreign,
        &foreign_polynomial,
        GuardBranch::NonZero,
        Default::default(),
        limits,
    )
    .unwrap();
    assert!(!first.same_predicate(&foreign_identity));

    let zero = context
        .numerator_condition_with_limits(&context.zero(), Default::default())
        .unwrap();
    assert_eq!(
        GuardBranchIdentity::try_from_indexed_polynomial(
            &context,
            &zero,
            GuardBranch::NonZero,
            Default::default(),
            limits,
        )
        .unwrap_err(),
        StratumRegistryError::ZeroGuardPolynomial
    );
}

#[test]
fn artifact_snapshot_exposes_only_proof_backed_terminal_regions() {
    let (artifact, _) = one_loop_frame(0);
    let artifact = Arc::new(artifact);
    let snapshot = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(snapshot.owner_count(), 2);
    assert!(
        snapshot
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert!(snapshot.id().as_str().contains(artifact.algorithm_id()));

    let zero_domain = SectorInteriorDomain::try_new(
        Mask::try_new([false]).unwrap(),
        [InteriorBounds::new(-17, 0)],
    )
    .unwrap();
    let parent = Mask::try_new([true]).unwrap();
    let ordering = OrderingPolicy::default();
    let zero_owner = snapshot.owner_for(&parent, ordering, &zero_domain).unwrap();
    assert_eq!(zero_owner.kind(), ImmutableOwnerKind::ZeroSector);
    assert!(zero_owner.owner_ordinal() < snapshot.owner_count());
    assert!(snapshot.verifies_witness(&parent, ordering, &zero_domain, zero_owner));

    let master_domain =
        SectorInteriorDomain::try_new(Mask::try_new([true]).unwrap(), [InteriorBounds::new(1, 1)])
            .unwrap();
    assert!(
        snapshot
            .owner_for(&parent, ordering, &master_domain)
            .is_none()
    );

    let nonterminal_domain =
        SectorInteriorDomain::try_new(Mask::try_new([true]).unwrap(), [InteriorBounds::new(1, 2)])
            .unwrap();
    assert!(
        snapshot
            .owner_for(&parent, ordering, &nonterminal_domain)
            .is_none()
    );
}

#[test]
fn terminal_authority_snapshot_retains_and_cheaply_rejoins_its_exact_owner() {
    let authority = derive_k6_terminal_authority().unwrap();
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        authority,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(snapshot.owner_count(), 26 + 3 + 6);
    assert!(
        snapshot
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert!(
        snapshot
            .id()
            .as_str()
            .contains("rustred.three-loop-unit-mass-vacuum-k6.terminal-authority.v1")
    );

    let zero = SectorInteriorDomain::try_new(
        Mask::try_new([false; 6]).unwrap(),
        [InteriorBounds::new(-7, 0); 6],
    )
    .unwrap();
    let parent = Mask::try_new([true; 6]).unwrap();
    let ordering = OrderingPolicy::default();
    let zero_owner = snapshot.owner_for(&parent, ordering, &zero).unwrap();
    assert_eq!(zero_owner.kind(), ImmutableOwnerKind::ZeroSector);
    assert!(snapshot.verifies_witness(&parent, ordering, &zero, zero_owner));

    let factorized = SectorInteriorDomain::try_new(
        Mask::try_new([false, false, true, false, true, true]).unwrap(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(2, 2),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(3, 3),
            InteriorBounds::new(4, 4),
        ],
    )
    .unwrap();
    let factorization_owner = snapshot.owner_for(&parent, ordering, &factorized).unwrap();
    assert_eq!(
        factorization_owner.kind(),
        ImmutableOwnerKind::Factorization
    );
    assert!(snapshot.verifies_witness(&parent, ordering, &factorized, factorization_owner));

    let embedded_corner = SectorInteriorDomain::try_new(
        Mask::try_new([false, false, true, false, true, true]).unwrap(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
    )
    .unwrap();
    assert_eq!(
        snapshot
            .owner_for(&parent, ordering, &embedded_corner)
            .unwrap()
            .kind(),
        ImmutableOwnerKind::Factorization,
        "compiled factorization must precede its embedded terminal corner"
    );

    let unresolved = SectorInteriorDomain::try_new(
        Mask::try_new([true; 6]).unwrap(),
        [InteriorBounds::new(1, 1); 6],
    )
    .unwrap();
    assert!(snapshot.owner_for(&parent, ordering, &unresolved).is_none());

    let clone = snapshot.clone();
    assert_eq!(clone, snapshot);
    assert!(clone.same_authority_as(&snapshot));
    assert!(clone.try_verify(StratumRegistryLimits::default()).unwrap());
    drop(clone);
}

#[test]
fn k6_snapshot_routes_every_factorization_and_master_orbit_image_exactly() {
    let authority = derive_k6_terminal_authority().unwrap();
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        Arc::clone(&authority),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let canonicalizer = authority.canonicalizer().unwrap();
    let parent = Mask::try_new([true; 6]).unwrap();
    let ordering = OrderingPolicy::default();
    let factorization_base = authority.zero_sectors().len();

    for (factorization_ordinal, rule) in authority.factorization_rules().iter().enumerate() {
        let mut raw_domains = Vec::new();
        for route in canonicalizer.routing_witnesses() {
            let raw = preimage_domain(rule.application_domain(), route.source_for_target());
            if !raw_domains.contains(&raw) {
                raw_domains.push(raw);
            }
        }
        let expected_orbit = k6_product_orbit_size(rule.application_domain().sector());
        assert_eq!(raw_domains.len(), expected_orbit);
        let expected_owner = factorization_base + factorization_ordinal;
        assert_eq!(
            snapshot.route_count_for_owner(expected_owner),
            expected_orbit
        );

        for raw in raw_domains {
            let owner = snapshot.owner_for(&parent, ordering, &raw).unwrap();
            assert_eq!(owner.owner_ordinal(), expected_owner);
            assert_eq!(owner.kind(), ImmutableOwnerKind::Factorization);
            assert!(snapshot.verifies_witness(&parent, ordering, &raw, owner));

            let corner = SectorInteriorDomain::try_new(
                raw.sector().clone(),
                raw.sector().active_bits().iter().map(|&active| {
                    if active {
                        InteriorBounds::new(1, 1)
                    } else {
                        InteriorBounds::new(0, 0)
                    }
                }),
            )
            .unwrap();
            let corner_owner = snapshot.owner_for(&parent, ordering, &corner).unwrap();
            assert_eq!(corner_owner.owner_ordinal(), expected_owner);
            assert_eq!(corner_owner.kind(), ImmutableOwnerKind::Factorization);

            let widened_slot = raw
                .sector()
                .active_bits()
                .iter()
                .position(|&active| !active)
                .unwrap();
            let widened = SectorInteriorDomain::try_new(
                raw.sector().clone(),
                raw.sector()
                    .active_bits()
                    .iter()
                    .enumerate()
                    .map(|(slot, &active)| {
                        if active {
                            InteriorBounds::new(1, 1)
                        } else if slot == widened_slot {
                            InteriorBounds::new(-1, 0)
                        } else {
                            InteriorBounds::new(0, 0)
                        }
                    }),
            )
            .unwrap();
            assert!(snapshot.owner_for(&parent, ordering, &widened).is_none());
        }
    }

    let master_base = factorization_base + authority.factorization_rules().len();
    for (master_ordinal, master) in authority.parent_terminals().iter().enumerate() {
        let sector = Mask::try_from_indices(master.powers()).unwrap();
        assert_eq!(
            snapshot.route_count_for_owner(master_base + master_ordinal),
            k6_product_orbit_size(&sector),
            "every shadowed master alias must still be retained and cold-verified"
        );
    }
}

fn preimage_domain(
    owner: &SectorInteriorDomain,
    source_for_target: &[usize],
) -> SectorInteriorDomain {
    let mut raw_bits = vec![false; owner.arity()];
    let mut raw_bounds = vec![InteriorBounds::new(0, 0); owner.arity()];
    for (owner_slot, &raw_slot) in source_for_target.iter().enumerate() {
        raw_bits[raw_slot] = owner.sector().active_bits()[owner_slot];
        raw_bounds[raw_slot] = owner.bounds()[owner_slot];
    }
    SectorInteriorDomain::try_new(Mask::try_new(raw_bits).unwrap(), raw_bounds).unwrap()
}

fn k6_product_orbit_size(sector: &Mask) -> usize {
    match sector.active_bits() {
        [false, false, true, true, true, true] => 12,
        [false, false, true, true, false, true] => 4,
        [false, false, true, false, true, true] => 12,
        unexpected => panic!("unexpected canonical K6 product sector: {unexpected:?}"),
    }
}

#[test]
fn structurally_equal_terminal_snapshots_do_not_alias_distinct_installed_authorities() {
    let installed = derive_k6_terminal_authority().unwrap();
    let same_installed = ImmutableOwnerSnapshot::try_from_terminal_authority(
        installed.clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let same_installed_again = ImmutableOwnerSnapshot::try_from_terminal_authority(
        installed,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(same_installed, same_installed_again);
    assert!(same_installed.same_authority_as(&same_installed_again));

    let independently_installed = ImmutableOwnerSnapshot::try_from_terminal_authority(
        fresh_k6_terminal_authority_for_test().unwrap(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(same_installed.id(), independently_installed.id());
    assert_eq!(same_installed, independently_installed);
    assert!(!same_installed.same_authority_as(&independently_installed));
}

#[test]
fn every_one_loop_physical_column_gets_exactly_one_target_local_role() {
    let (_, frame) = one_loop_frame(2);
    let empty = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let mut saw_allowed = false;
    let mut saw_forbidden = false;

    for target in 0..frame.columns().len() {
        let partition = TargetColumnPartition::try_new(
            &frame,
            target,
            maximal_stratum(&frame, target),
            empty.clone(),
            OrderingPolicy::default(),
            StratumRegistryLimits::default(),
        )
        .unwrap();
        assert!(partition.try_verify().unwrap());
        assert!(std::ptr::eq(partition.frame(), &frame));
        assert_eq!(partition.target_column(), target);
        assert_eq!(partition.stratum().id(), partition.stratum_id());
        assert_eq!(
            partition.allowed_columns().len() + partition.forbidden_columns().len(),
            frame.columns().len() - 1
        );
        assert!(!partition.is_allowed(target));
        assert!(!partition.forbidden_columns().contains(&target));
        assert_eq!(
            partition
                .try_classify_prospective_shift(frame.columns()[target].values())
                .unwrap(),
            ProspectiveColumnKind::Target
        );
        for allowed in partition.allowed_columns() {
            saw_allowed = true;
            assert!(allowed.descent().verify());
            assert!(allowed.proper_subsector_owners().is_empty());
            assert_eq!(
                partition.allowed_descriptor(allowed.column()),
                Some(allowed)
            );
            assert_eq!(
                partition
                    .try_classify_prospective_shift(frame.columns()[allowed.column()].values(),)
                    .unwrap(),
                ProspectiveColumnKind::Allowed
            );
        }
        for forbidden in partition.forbidden_descriptors() {
            saw_forbidden = true;
            assert_eq!(
                partition.forbidden_reason(forbidden.column()),
                Some(forbidden.reason())
            );
            assert!(matches!(
                forbidden.reason(),
                ForbiddenColumnReason::NotStrictDescent
                    | ForbiddenColumnReason::InactiveLineActivation { .. }
                    | ForbiddenColumnReason::UnownedProperSubsector { .. }
            ));
            assert_eq!(
                partition
                    .try_classify_prospective_shift(frame.columns()[forbidden.column()].values(),)
                    .unwrap(),
                ProspectiveColumnKind::Forbidden
            );
        }
    }
    assert!(saw_allowed);
    assert!(saw_forbidden);
}

#[test]
fn frame_scope_and_resource_mismatches_are_typed() {
    let (_, frame) = one_loop_frame(1);
    let target = frame.columns().len() - 1;
    let stratum = maximal_stratum(&frame, target);
    let owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let wrong_family = DecoratedStratum::try_guard_blind(
        "another-family",
        frame.context_fingerprint(),
        stratum.domain().clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        TargetColumnPartition::try_new(
            &frame,
            target,
            wrong_family,
            owners.clone(),
            OrderingPolicy::default(),
            StratumRegistryLimits::default(),
        )
        .unwrap_err(),
        StratumRegistryError::WrongFrameFamily
    );

    let mut limits = StratumRegistryLimits::default();
    limits.max_physical_columns = frame.columns().len() - 1;
    assert_eq!(
        TargetColumnPartition::try_new(
            &frame,
            target,
            stratum,
            owners,
            OrderingPolicy::default(),
            limits,
        )
        .unwrap_err(),
        StratumRegistryError::ResourceLimit {
            resource: "decorated-stratum physical columns",
            requested: frame.columns().len(),
            limit: frame.columns().len() - 1,
        }
    );

    let guarded = DecoratedStratum::try_new(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        maximal_stratum(&frame, target).domain().clone(),
        [GuardBranchIdentity::try_new(
            "bounded-before-partition",
            GuardBranch::NonZero,
            StratumRegistryLimits::default(),
        )
        .unwrap()],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let mut stored_limits = StratumRegistryLimits::default();
    stored_limits.max_guard_branches = 0;
    assert_eq!(
        TargetColumnPartition::try_new(
            &frame,
            target,
            guarded,
            owners,
            OrderingPolicy::default(),
            stored_limits,
        )
        .unwrap_err(),
        StratumRegistryError::ResourceLimit {
            resource: "decorated-stratum guard branches",
            requested: 1,
            limit: 0,
        }
    );
}
