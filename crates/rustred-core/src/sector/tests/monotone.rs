use super::super::{
    ComplexityComponent, Error, Mask, OrderingPolicy, SectorMonotoneDomain,
    SectorMonotonePointClass,
};

#[test]
fn sunset_box_and_single_coordinate_slabs_are_exact() {
    let sector = Mask::try_new([true, true, true]).unwrap();
    let pivot = [1, 0, 0];
    let targets = [[0, 1, -1], [0, 0, 0], [0, -1, 1], [-1, 1, 0]];
    let domain = SectorMonotoneDomain::try_maximal_for_rule(sector, &pivot, &targets).unwrap();
    assert_eq!(
        domain
            .bounds()
            .iter()
            .map(|bounds| (bounds.lower(), bounds.upper()))
            .collect::<Vec<_>>(),
        vec![(1, i64::MAX - 1), (1, i64::MAX - 1), (1, i64::MAX - 1),]
    );

    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &pivot, &[0, 1, -1])
        .unwrap();
    assert!(witness.verify());
    assert_eq!(witness.thresholds().len(), 1);
    assert_eq!(witness.thresholds()[0].position(), 2);
    assert_eq!(witness.thresholds()[0].pinched_upper(), 1);
    assert_eq!(witness.thresholds()[0].same_sector_lower(), Some(2));
    assert_eq!(
        witness.classify(&[1, 1, 1]).unwrap(),
        Some(SectorMonotonePointClass::ProperSubsector {
            cylinder_ordinal: 0,
            pinched_position: 2,
        })
    );
    assert_eq!(
        witness.classify(&[1, 1, 2]).unwrap(),
        Some(SectorMonotonePointClass::SameSector)
    );
    assert!(witness.same_sector_descent().unwrap().verify());
}

#[test]
fn minus_two_and_multi_coordinate_pinches_form_compact_unique_cylinders() {
    let one_sector = Mask::try_new([true]).unwrap();
    let one_domain = SectorMonotoneDomain::try_maximal_for_rule(one_sector, &[1], &[[-2]]).unwrap();
    let minus_two = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&one_domain, &[1], &[-2])
        .unwrap();
    assert!(minus_two.verify());
    assert_eq!(minus_two.thresholds()[0].pinched_upper(), 2);
    assert_eq!(minus_two.thresholds()[0].same_sector_lower(), Some(3));
    for index in [1, 2] {
        assert!(matches!(
            minus_two.classify(&[index]).unwrap(),
            Some(SectorMonotonePointClass::ProperSubsector { .. })
        ));
    }
    assert_eq!(
        minus_two.classify(&[3]).unwrap(),
        Some(SectorMonotonePointClass::SameSector)
    );

    let sector = Mask::try_new([true, true, true]).unwrap();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &[1, 0, 0], &[[-1, -2, 0]]).unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[1, 0, 0], &[-1, -2, 0])
        .unwrap();
    assert!(witness.verify());
    assert_eq!(witness.thresholds().len(), 2);
    assert_eq!(witness.proper_subsector_cylinder_count(), 2);
    assert_eq!(
        witness.classify(&[1, 1, 1]).unwrap(),
        Some(SectorMonotonePointClass::ProperSubsector {
            cylinder_ordinal: 0,
            pinched_position: 0,
        })
    );
    assert_eq!(
        witness.classify(&[2, 2, 1]).unwrap(),
        Some(SectorMonotonePointClass::ProperSubsector {
            cylinder_ordinal: 1,
            pinched_position: 1,
        })
    );
    assert_eq!(
        witness.classify(&[2, 3, 1]).unwrap(),
        Some(SectorMonotonePointClass::SameSector)
    );
    assert_eq!(
        witness.proper_subsector_decisive_component(),
        ComplexityComponent::PropagatorCount
    );
}

#[test]
fn monotone_proofs_handle_extreme_and_pivot_tightened_bounds() {
    let sector = Mask::try_new([true]).unwrap();
    let extreme_domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), &[0], &[[i64::MIN]]).unwrap();
    let extreme = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&extreme_domain, &[0], &[i64::MIN])
        .unwrap();
    assert!(extreme.verify());
    assert_eq!(extreme.thresholds()[0].pinched_upper(), i64::MAX);
    assert_eq!(extreme.thresholds()[0].same_sector_lower(), None);
    assert!(extreme.same_sector_descent().is_none());
    assert_eq!(extreme.proper_subsector_cylinder_count(), 1);

    let compact_domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        &[0, 0],
        &[[i64::MIN, -1]],
    )
    .unwrap();
    let compact = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&compact_domain, &[0, 0], &[i64::MIN, -1])
        .unwrap();
    assert!(compact.verify());
    assert_eq!(compact.thresholds().len(), 1);
    assert_eq!(compact.thresholds()[0].position(), 0);
    assert_eq!(compact.proper_subsector_cylinder_count(), 1);

    let tightened = SectorMonotoneDomain::try_maximal_for_rule(sector, &[-1], &[[-2]]).unwrap();
    assert_eq!(tightened.bounds()[0].lower(), 2);
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&tightened, &[-1], &[-2])
        .unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.classify(&[2]).unwrap(),
        Some(SectorMonotonePointClass::ProperSubsector {
            cylinder_ordinal: 0,
            pinched_position: 0,
        })
    );
}

#[test]
fn activation_and_same_sector_harder_shifts_are_rejected() {
    let inactive = Mask::try_new([false]).unwrap();
    let inactive_domain =
        SectorMonotoneDomain::try_maximal_for_rule(inactive, &[0], &[[1]]).unwrap();
    assert_eq!(
        OrderingPolicy::default()
            .prove_sector_monotone_shift_descent(&inactive_domain, &[0], &[1],),
        Err(Error::InactiveLineActivation {
            position: 0,
            shift: 1,
        })
    );

    let active = Mask::try_new([true]).unwrap();
    let harder_domain = SectorMonotoneDomain::try_maximal_for_rule(active, &[0], &[[1]]).unwrap();
    assert_eq!(
        OrderingPolicy::default().prove_sector_monotone_shift_descent(&harder_domain, &[0], &[1],),
        Err(Error::NotStrictDescent)
    );
}

#[test]
fn sector_monotone_construction_is_deterministic() {
    let sector = Mask::try_new([true, true]).unwrap();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &[1, 0], &[[-1, -2], [0, 0]]).unwrap();
    let first = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[1, 0], &[-1, -2])
        .unwrap();
    let second = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[1, 0], &[-1, -2])
        .unwrap();
    assert_eq!(first, second);
}
