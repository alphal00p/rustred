use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::artifact::{derive_k6_terminal_authority, derive_one_loop_unit_mass_tadpole};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::frame::{OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan};
use super::{
    DecoratedStratum, ForbiddenColumnReason, GuardBranch, GuardBranchIdentity,
    GuardPredicateAuthority, ImmutableOwnerKind, ImmutableOwnerSnapshot, StratumRegistryError,
    StratumRegistryLimits, TargetColumnPartition,
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
    let snapshot = ImmutableOwnerSnapshot::try_from_closed_artifact(
        &artifact,
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
    let zero_owner = snapshot.owner_for(&zero_domain).unwrap();
    assert_eq!(zero_owner.kind(), ImmutableOwnerKind::ZeroSector);
    assert!(zero_owner.owner_ordinal() < snapshot.owner_count());
    assert!(snapshot.verifies_witness(&zero_domain, zero_owner));

    let master_domain =
        SectorInteriorDomain::try_new(Mask::try_new([true]).unwrap(), [InteriorBounds::new(1, 1)])
            .unwrap();
    let master_owner = snapshot.owner_for(&master_domain).unwrap();
    assert_eq!(master_owner.kind(), ImmutableOwnerKind::Master);
    assert!(snapshot.verifies_witness(&master_domain, master_owner));

    let nonterminal_domain =
        SectorInteriorDomain::try_new(Mask::try_new([true]).unwrap(), [InteriorBounds::new(1, 2)])
            .unwrap();
    assert!(snapshot.owner_for(&nonterminal_domain).is_none());
}

#[test]
fn terminal_authority_snapshot_retains_and_cheaply_rejoins_its_exact_owner() {
    let authority = derive_k6_terminal_authority().unwrap();
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        authority,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(snapshot.owner_count(), 26 + 3 + 3);
    assert!(
        snapshot
            .try_verify(StratumRegistryLimits::default())
            .unwrap()
    );
    assert!(
        snapshot
            .id()
            .as_str()
            .contains("rustred.test.three-loop-k6-terminal-authority.v1")
    );

    let zero = SectorInteriorDomain::try_new(
        Mask::try_new([false; 6]).unwrap(),
        [InteriorBounds::new(-7, 0); 6],
    )
    .unwrap();
    let zero_owner = snapshot.owner_for(&zero).unwrap();
    assert_eq!(zero_owner.kind(), ImmutableOwnerKind::ZeroSector);
    assert!(snapshot.verifies_witness(&zero, zero_owner));

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
    let factorization_owner = snapshot.owner_for(&factorized).unwrap();
    assert_eq!(
        factorization_owner.kind(),
        ImmutableOwnerKind::Factorization
    );
    assert!(snapshot.verifies_witness(&factorized, factorization_owner));

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
        snapshot.owner_for(&embedded_corner).unwrap().kind(),
        ImmutableOwnerKind::Factorization,
        "compiled factorization must precede its embedded terminal corner"
    );

    let unresolved = SectorInteriorDomain::try_new(
        Mask::try_new([true; 6]).unwrap(),
        [InteriorBounds::new(1, 1); 6],
    )
    .unwrap();
    assert!(snapshot.owner_for(&unresolved).is_none());

    let clone = snapshot.clone();
    assert_eq!(clone, snapshot);
    assert!(clone.try_verify(StratumRegistryLimits::default()).unwrap());
    drop(clone);
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
        for allowed in partition.allowed_columns() {
            saw_allowed = true;
            assert!(allowed.descent().verify());
            assert!(allowed.proper_subsector_owners().is_empty());
            assert_eq!(
                partition.allowed_descriptor(allowed.column()),
                Some(allowed)
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
