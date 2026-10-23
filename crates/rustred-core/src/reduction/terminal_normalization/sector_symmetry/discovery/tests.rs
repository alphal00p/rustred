use super::*;
use crate::family::AffineDenominator;

fn family(name: &str, vectors: &[&[i64]], masses: &[i64]) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    let loops = vectors[0].len();
    let denominators = vectors
        .iter()
        .zip(masses)
        .map(|(row, &mass)| {
            let mut coefficients = Vec::new();
            for left in 0..loops {
                for right in left..loops {
                    coefficients.push(
                        context.integer(row[left] * row[right] * if left == right { 1 } else { 2 }),
                    );
                }
            }
            AffineDenominator::new(context.integer(-mass), coefficients)
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            name,
            (0..loops).map(|index| format!("k{index}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); vectors.len()],
        )
        .unwrap(),
    )
}

fn support(family: &Arc<IntegralFamily>, bits: &[bool]) -> Arc<PreparedSupport> {
    prepare_support(
        family,
        &Mask::try_new(bits.iter().copied()).unwrap(),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap()
    .unwrap()
}

fn image(map: &VerifiedMap, slot: usize) -> usize {
    let DenominatorAction::Monomial { target, scale } = &map.row_actions()[slot] else {
        panic!("active image is affine");
    };
    assert!(scale.is_one());
    *target
}

#[test]
fn simultaneous_three_cycle_is_discovered_without_any_individual_swap() {
    // Directed coefficient ratios distinguish the cyclic order. All three
    // basis lines and all three adjacent lines must move simultaneously.
    let family = family(
        "cyclic_line_configuration",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[0, 0, 1],
            &[1, 2, 0],
            &[0, 1, 2],
            &[2, 0, 1],
        ],
        &[1, 1, 1, 2, 2, 2],
    );
    let maps = discover(
        &family,
        &Mask::try_new([true; 6]).unwrap(),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert!(
        maps.iter()
            .any(|map| (0..3).all(|slot| image(map, slot) != slot))
    );
    assert!(maps.iter().all(|map| {
        let moved = (0..3).filter(|&slot| image(map, slot) != slot).count();
        moved == 0 || moved == 3
    }));
    for map in maps {
        assert!(matches!(map.jacobian(), Jacobian::Unit { .. }));
        assert_eq!(map.source_family_fingerprint(), family.fingerprint());
        assert_eq!(map.target_family_fingerprint(), family.fingerprint());
    }
}

#[test]
fn bridge_orientation_generators_keep_affine_inactive_isp_actions() {
    let family = family(
        "bridge_orientation",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let maps = discover(
        &family,
        &Mask::try_new([true, true, false]).unwrap(),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert!(
        maps.iter()
            .any(|map| matches!(map.row_actions()[2], DenominatorAction::Affine))
    );
    assert!(maps.iter().any(|map| {
        let entries = map.momentum().loop_linear().entries();
        entries[0] != entries[3] && entries[1].is_zero() && entries[2].is_zero()
    }));
}

#[test]
fn canonical_projector_and_cross_family_map_are_loop_basis_invariant() {
    let source = family("projector_source", &[&[1, 0], &[0, 1], &[1, 1]], &[1, 1, 1]);
    let target = family(
        "projector_sheared",
        &[&[1, 1], &[0, 1], &[1, 2]],
        &[1, 1, 1],
    );
    let source = support(&source, &[true; 3]);
    let target = support(&target, &[true; 3]);
    assert_eq!(source.graph, target.graph);
    let map = match_supports(
        &source,
        &target,
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap()
    .unwrap();
    assert!(matches!(map.jacobian(), Jacobian::Unit { .. }));
    assert_eq!(map.source_family_fingerprint(), source.family.fingerprint());
    assert_eq!(map.target_family_fingerprint(), target.family.fingerprint());
}

#[test]
fn exact_mass_colors_prevent_invalid_active_interchange() {
    let family = family(
        "unequal_line_masses",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 2, 1],
    );
    let maps = discover(
        &family,
        &Mask::try_new([true, true, false]).unwrap(),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert!(!maps.is_empty());
    for map in maps {
        assert_eq!(image(&map, 0), 0);
        assert_eq!(image(&map, 1), 1);
    }
}

#[test]
fn cross_support_canonical_route_is_discovered_independently_of_powers() {
    let family = family(
        "different_active_supports",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let source = support(&family, &[true, true, false]);
    let target = support(&family, &[true, false, true]);
    let map = match_supports(
        &source,
        &target,
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap()
    .unwrap();
    let mut images = [image(&map, 0), image(&map, 1)];
    images.sort();
    assert_eq!(images, [0, 2]);
}

#[test]
fn nonunit_cross_family_jacobian_is_rejected_after_exact_verification() {
    let source = family(
        "unit_volume_source",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let target = family(
        "doubled_line_target",
        &[&[2, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let source = support(&source, &[true, true, false]);
    let target = support(&target, &[true, true, false]);
    assert_eq!(source.graph, target.graph);
    assert!(
        match_supports(
            &source,
            &target,
            Default::default(),
            &TerminalSectorSymmetryControl::default()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn rank_deficiency_supplies_no_projector_proposal() {
    let family = family(
        "rank_deficient_support",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[1, 1, 0],
            &[0, 0, 1],
            &[1, 0, 1],
            &[0, 1, 1],
        ],
        &[1; 6],
    );
    // Three active lines meet the count threshold but span only two loops.
    assert!(
        prepare_support(
            &family,
            &Mask::try_new([true, true, true, false, false, false]).unwrap(),
            Default::default(),
            &TerminalSectorSymmetryControl::default()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn finite_root_gate_rejects_a_valid_shear_on_a_deficient_support() {
    let family = family(
        "deficient_support_shear",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[1, 1, 0],
            &[0, 0, 1],
            &[1, 0, 1],
            &[0, 1, 1],
        ],
        &[1; 6],
    );
    let shear = MomentumMap::new(
        CoefficientMatrix::try_new(
            3,
            3,
            [1, 0, 0, 0, 1, 0, 0, 1, 1].map(|value| family.coefficient_context().integer(value)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(3, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    );
    let verified = symmetry::verify(&family, &family, shear, Default::default()).unwrap();
    assert!(matches!(verified.jacobian(), Jacobian::Unit { .. }));
    for slot in 0..3 {
        assert_eq!(image(&verified, slot), slot);
    }
    assert!(
        !verify_finite_root(
            &family,
            &Mask::try_new([true, true, true, false, false, false]).unwrap(),
            Default::default(),
            &TerminalSectorSymmetryControl::default(),
        )
        .unwrap()
    );
    assert!(
        verify_finite_root(
            &family,
            &Mask::try_new([true; 6]).unwrap(),
            Default::default(),
            &TerminalSectorSymmetryControl::default(),
        )
        .unwrap()
    );
}

#[test]
fn independent_native_basis_rows_need_not_be_an_active_prefix() {
    let family = family(
        "nonprefix_pivots",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[1, 1, 0],
            &[0, 0, 1],
            &[1, 0, 1],
            &[0, 1, 1],
        ],
        &[1; 6],
    );
    let prepared = support(&family, &[true, true, true, true, false, false]);
    assert_eq!(prepared.basis_slots, [0, 1, 3]);
    assert!(
        !discover_prepared(
            &prepared,
            Default::default(),
            &TerminalSectorSymmetryControl::default()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn cached_geometry_is_shared_only_for_declared_supports() {
    let family = family(
        "shared_support_cache",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let root = Mask::try_new([true, true, false]).unwrap();
    let mut cache = DiscoveryCache::new(family);
    let first = cache
        .prepare_support(
            &root,
            Default::default(),
            &TerminalSectorSymmetryControl::default(),
        )
        .unwrap()
        .unwrap();
    let second = cache
        .prepare_support(
            &root,
            Default::default(),
            &TerminalSectorSymmetryControl::default(),
        )
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.supports.len(), 1);
    assert!(matches!(cache.momenta[2], Momentum::Unprepared));
}

#[test]
fn cancellation_precedes_native_algebra() {
    let family = family(
        "cancelled_support",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 1],
    );
    let control = TerminalSectorSymmetryControl {
        cancelled: Some(&|| true),
        progress: None,
    };
    assert!(
        prepare_support(
            &family,
            &Mask::try_new([true; 3]).unwrap(),
            Default::default(),
            &control
        )
        .is_err()
    );
}
