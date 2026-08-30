use super::super::{
    ComplexityComponent, Error, InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain,
    SectorMonotonePointClass, SectorMonotoneTargetCellKind,
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
fn tightened_inactive_cell_accepts_a_positive_shift_that_cannot_activate() {
    let inactive = Mask::try_new([false]).unwrap();
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(inactive.clone(), &[0], &[[1]]).unwrap();
    assert_eq!(
        OrderingPolicy::default().prove_sector_monotone_shift_descent(&maximal, &[0], &[1]),
        Err(Error::InactiveLineActivation {
            position: 0,
            shift: 1,
        })
    );

    let tightened = SectorMonotoneDomain::try_new_for_rule(
        inactive,
        [InteriorBounds::new(-8, -1)],
        &[0],
        &[[1]],
    )
    .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&tightened, &[0], &[1])
        .unwrap();
    assert!(witness.verify());
    assert_eq!(
        witness.classify(&[-1]).unwrap(),
        Some(SectorMonotonePointClass::SameSector)
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

#[test]
fn exact_target_partition_refines_every_optional_pinch_combination() {
    let sector = Mask::try_new([true, true, true]).unwrap();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &[1, 0, 0], &[[-1, -2, 0]]).unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[1, 0, 0], &[-1, -2, 0])
        .unwrap();
    let census = witness.target_sector_partition_census().unwrap();
    assert_eq!(census.optional_coordinate_count(), 2);
    assert_eq!(census.cell_count(), 4);
    assert_eq!(census.proper_subsector_cell_count(), 3);
    let partition = witness.try_target_sector_partition().unwrap();

    assert!(partition.try_verify().unwrap());
    assert_eq!(partition.optional_coordinate_count(), 2);
    assert_eq!(partition.cell_count(), 4);
    assert_eq!(partition.proper_subsector_cell_count(), 3);
    assert_eq!(
        (0..4)
            .map(|ordinal| partition.cell_kind(ordinal).unwrap())
            .collect::<Vec<_>>(),
        vec![
            SectorMonotoneTargetCellKind::ProperSubsector,
            SectorMonotoneTargetCellKind::ProperSubsector,
            SectorMonotoneTargetCellKind::ProperSubsector,
            SectorMonotoneTargetCellKind::SameSector,
        ]
    );
    assert_eq!(
        (0..=4)
            .map(|ordinal| {
                partition
                    .proper_subsector_cell_count_before(ordinal)
                    .unwrap()
            })
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 3]
    );
    let cells = partition.cells().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        cells
            .iter()
            .map(|cell| cell.target_domain().sector().active_bits())
            .collect::<Vec<_>>(),
        vec![
            &[false, false, true][..],
            &[true, false, true][..],
            &[false, true, true][..],
            &[true, true, true][..],
        ]
    );
    assert_eq!(cells[3].kind(), SectorMonotoneTargetCellKind::SameSector);
    assert!(
        cells[..3]
            .iter()
            .all(|cell| cell.kind() == SectorMonotoneTargetCellKind::ProperSubsector)
    );
    assert_eq!(cells[0].pinched_positions().collect::<Vec<_>>(), vec![0, 1]);
    for cell in &cells {
        assert!(partition.try_verifies_cell(cell).unwrap());
        assert_eq!(cell.base_domain().sector().active_bits(), &[true; 3]);
        assert_eq!(cell.pivot_domain().sector().active_bits(), &[true; 3]);
    }
    assert_eq!(
        partition.cell(4),
        Err(Error::TargetSectorCellOutOfRange {
            ordinal: 4,
            cell_count: 4,
        })
    );
}

#[test]
fn always_pinched_coordinate_does_not_hide_later_target_sectors() {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        &[0, 0],
        &[[i64::MIN, -1]],
    )
    .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[0, 0], &[i64::MIN, -1])
        .unwrap();
    // The compact first-pinched proof stops after coordinate zero.
    assert_eq!(witness.thresholds().len(), 1);

    let partition = witness.try_target_sector_partition().unwrap();
    assert_eq!(partition.optional_coordinate_count(), 1);
    assert_eq!(partition.cell_count(), 2);
    assert_eq!(partition.proper_subsector_cell_count(), 2);
    assert_eq!(
        partition
            .cells()
            .map(|cell| cell.unwrap().target_domain().sector().clone())
            .collect::<Vec<_>>(),
        vec![
            Mask::try_new([false, false]).unwrap(),
            Mask::try_new([false, true]).unwrap(),
        ]
    );
}

#[test]
fn exact_target_cell_cursor_resumes_without_replaying_prior_cells() {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        &[0, 0],
        &[[-1, -1]],
    )
    .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[0, 0], &[-1, -1])
        .unwrap();
    let partition = witness.try_target_sector_partition().unwrap();
    let mut first_wave = partition.cells();
    assert_eq!(first_wave.next().unwrap().unwrap().ordinal(), 0);
    assert_eq!(first_wave.next().unwrap().unwrap().ordinal(), 1);
    let cursor = first_wave.next_ordinal();
    assert_eq!(cursor, 2);
    assert_eq!(first_wave.remaining_cell_count(), 2);
    assert_eq!(
        partition
            .cells_from(cursor)
            .unwrap()
            .map(|cell| cell.unwrap().ordinal())
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(
        partition.cells_from(partition.cell_count()).unwrap().next(),
        None
    );
    assert!(matches!(
        partition.cells_from(partition.cell_count() + 1),
        Err(Error::TargetSectorCellOutOfRange { .. })
    ));
    assert_eq!(partition.cells().size_hint(), (0, None));
}

#[test]
fn noncontiguous_optional_coordinates_have_stable_inactive_first_ordinals() {
    let parent = Mask::try_new([true, false, true, true]).unwrap();
    let pivot = [1, 0, 0, 0];
    let target = [-1, 0, 0, -2];
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(parent.clone(), &pivot, &[target]).unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &pivot, &target)
        .unwrap();
    let partition = witness.try_target_sector_partition().unwrap();
    assert_eq!(partition.optional_coordinate_count(), 2);
    assert_eq!(
        partition
            .cells()
            .map(|cell| cell.unwrap().target_domain().sector().clone())
            .collect::<Vec<_>>(),
        vec![
            Mask::try_new([false, false, true, false]).unwrap(),
            Mask::try_new([true, false, true, false]).unwrap(),
            Mask::try_new([false, false, true, true]).unwrap(),
            parent,
        ]
    );
}

#[test]
fn zero_optional_coordinate_partitions_cover_same_and_forced_pinch_extremes() {
    let sector = Mask::try_new([true]).unwrap();
    let same_domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), &[1], &[[0]]).unwrap();
    let same = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&same_domain, &[1], &[0])
        .unwrap();
    let same_partition = same.try_target_sector_partition().unwrap();
    assert_eq!(same_partition.cell_count(), 1);
    assert_eq!(same_partition.proper_subsector_cell_count(), 0);
    assert_eq!(
        same_partition.cell(0).unwrap().kind(),
        SectorMonotoneTargetCellKind::SameSector
    );

    let forced_domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &[0], &[[i64::MIN]]).unwrap();
    let forced = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&forced_domain, &[0], &[i64::MIN])
        .unwrap();
    let forced_partition = forced.try_target_sector_partition().unwrap();
    assert_eq!(forced_partition.cell_count(), 1);
    assert_eq!(forced_partition.proper_subsector_cell_count(), 1);
    let cell = forced_partition.cell(0).unwrap();
    assert_eq!(cell.base_domain().bounds()[0].lower(), 1);
    assert_eq!(cell.base_domain().bounds()[0].upper(), i64::MAX);
    assert_eq!(cell.target_domain().bounds()[0].lower(), i64::MIN + 1);
    assert_eq!(cell.target_domain().bounds()[0].upper(), -1);
    assert_eq!(cell.kind(), SectorMonotoneTargetCellKind::ProperSubsector);
}

#[test]
fn exact_target_cells_are_disjoint_and_exhaust_a_small_parent_box() {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        &[1, 0],
        &[[-1, -1]],
    )
    .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &[1, 0], &[-1, -1])
        .unwrap();
    let partition = witness.try_target_sector_partition().unwrap();
    let cells = partition.cells().collect::<Result<Vec<_>, _>>().unwrap();

    for first in 1..=3 {
        for second in 1..=3 {
            let point = [first, second];
            let containing = cells
                .iter()
                .filter(|cell| cell.base_domain().contains(&point).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(containing.len(), 1);
            let target = [first - 1, second - 1];
            assert!(containing[0].target_domain().contains(&target).unwrap());
            assert_eq!(
                containing[0].target_domain().sector(),
                &Mask::try_from_indices(&target).unwrap()
            );
        }
    }
}

#[test]
fn exponential_target_cell_count_overflow_is_typed_before_enumeration() {
    let arity = usize::BITS as usize;
    let sector = Mask::try_new(std::iter::repeat_n(true, arity)).unwrap();
    let pivot = vec![0; arity];
    let target = vec![-1; arity];
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &pivot, std::slice::from_ref(&target))
            .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &pivot, &target)
        .unwrap();

    assert_eq!(
        witness.try_target_sector_partition(),
        Err(Error::ComplexityOverflow {
            measure: "target-sector cell count",
        })
    );
}

#[test]
fn largest_representable_cell_count_still_materializes_one_streamed_cell() {
    let arity = usize::BITS as usize - 1;
    let sector = Mask::try_new(std::iter::repeat_n(true, arity)).unwrap();
    let pivot = vec![0; arity];
    let target = vec![-1; arity];
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &pivot, std::slice::from_ref(&target))
            .unwrap();
    let witness = OrderingPolicy::default()
        .prove_sector_monotone_shift_descent(&domain, &pivot, &target)
        .unwrap();
    let census = witness.target_sector_partition_census().unwrap();
    assert_eq!(census.cell_count(), 1usize << arity);
    let partition = witness.try_target_sector_partition().unwrap();
    let first = partition.cell(0).unwrap();
    assert_eq!(first.pinched_positions().count(), arity);
}
