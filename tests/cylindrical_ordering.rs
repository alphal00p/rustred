use std::cmp::Ordering;
use std::collections::BTreeSet;

use rustred::{
    CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
    CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA, CylindricalOrderingError,
    CylindricalOrderingLimits, CylindricalParametricEliminationOrdering, IndexShift,
    IntegralOrderingPolicy, PartialIndexAssignment,
    RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA, SectorMask,
};

fn shift(values: &[i64]) -> IndexShift {
    IndexShift::try_new(values.iter().copied(), values.len()).unwrap()
}

fn add_shift(base: &[i64], displacement: &IndexShift) -> Vec<i64> {
    base.iter()
        .zip(displacement.values())
        .map(|(&value, &delta)| value.checked_add(delta).unwrap())
        .collect()
}

fn vectors<T: Copy>(choices: &[T], arity: usize) -> Vec<Vec<T>> {
    fn recurse<T: Copy>(
        choices: &[T],
        arity: usize,
        current: &mut Vec<T>,
        output: &mut Vec<Vec<T>>,
    ) {
        if current.len() == arity {
            output.push(current.clone());
            return;
        }
        for &choice in choices {
            current.push(choice);
            recurse(choices, arity, current, output);
            current.pop();
        }
    }

    let mut output = Vec::new();
    recurse(choices, arity, &mut Vec::new(), &mut output);
    output
}

fn assignment_states(sector: &SectorMask) -> Vec<Vec<Option<i64>>> {
    fn recurse(
        sector: &SectorMask,
        position: usize,
        current: &mut Vec<Option<i64>>,
        output: &mut Vec<Vec<Option<i64>>>,
    ) {
        if position == sector.arity() {
            output.push(current.clone());
            return;
        }
        let fixed_values = if sector.active_bits()[position] {
            [1, 3]
        } else {
            [0, -2]
        };
        current.push(None);
        recurse(sector, position + 1, current, output);
        current.pop();
        for value in fixed_values {
            current.push(Some(value));
            recurse(sector, position + 1, current, output);
            current.pop();
        }
    }

    let mut output = Vec::new();
    recurse(sector, 0, &mut Vec::new(), &mut output);
    output
}

#[test]
fn symbolic_sector_11_orders_the_documented_pair_without_a_corner() {
    let policy = IntegralOrderingPolicy::RustRedUnshiftedV1;
    let ordering = CylindricalParametricEliminationOrdering::try_new(
        policy,
        SectorMask::try_from_bit_string("11").unwrap(),
        PartialIndexAssignment::try_new([], 2, 0).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let displaced = shift(&[-1, 2]);
    let origin = shift(&[0, 0]);

    assert_eq!(
        ordering.compare_shifts(&displaced, &origin).unwrap(),
        Ordering::Greater,
        "the signed symbolic dot offset is +1"
    );
    assert_eq!(
        policy.compare(&[0, 3], &[1, 1]).unwrap(),
        Ordering::Less,
        "fabricating the concrete corner changes the sector and gives the wrong order"
    );
    for interior in [[3, 3], [10, 10], [100, 100]] {
        assert_eq!(
            policy
                .compare(
                    &add_shift(&interior, &displaced),
                    &add_shift(&interior, &origin),
                )
                .unwrap(),
            Ordering::Greater
        );
    }
    assert_eq!(ordering.free_positions(), &[0, 1]);
    assert!(ordering.assignment().is_empty());
    ordering.replay().unwrap();
}

#[test]
fn exhaustive_small_cylinders_equal_every_sufficiently_interior_realization() {
    let policy = IntegralOrderingPolicy::RustRedUnshiftedV1;
    for arity in 1..=3 {
        let shifts = vectors(&[-2, -1, 0, 1, 2], arity)
            .into_iter()
            .map(|values| IndexShift::try_new(values, arity).unwrap())
            .collect::<Vec<_>>();
        for bits in vectors(&[false, true], arity) {
            let sector = SectorMask::try_new(bits).unwrap();
            for state in assignment_states(&sector) {
                let entries = state
                    .iter()
                    .enumerate()
                    .filter_map(|(position, value)| value.map(|value| (position, value)))
                    .collect::<Vec<_>>();
                let assignment = PartialIndexAssignment::try_new(entries, arity, arity).unwrap();
                let ordering = CylindricalParametricEliminationOrdering::try_new(
                    policy,
                    sector.clone(),
                    assignment,
                    CylindricalOrderingLimits::default(),
                )
                .unwrap();
                let interior = state
                    .iter()
                    .zip(sector.active_bits())
                    .map(|(fixed, &active)| fixed.unwrap_or(if active { 10 } else { -10 }))
                    .collect::<Vec<_>>();

                let cylindrical_keys = shifts
                    .iter()
                    .map(|displacement| ordering.key_for_shift(displacement).unwrap())
                    .collect::<Vec<_>>();
                let concrete_keys = shifts
                    .iter()
                    .map(|displacement| {
                        policy
                            .complexity_key(&add_shift(&interior, displacement))
                            .unwrap()
                    })
                    .collect::<Vec<_>>();

                assert_eq!(
                    cylindrical_keys.iter().collect::<BTreeSet<_>>().len(),
                    shifts.len(),
                    "the lattice-shift tie-break must make every key injective"
                );
                for left in 0..shifts.len() {
                    ordering.replay_key(&cylindrical_keys[left]).unwrap();
                    for right in 0..shifts.len() {
                        assert_eq!(
                            cylindrical_keys[left].cmp(&cylindrical_keys[right]),
                            concrete_keys[left].cmp(&concrete_keys[right]),
                            "arity={arity}, sector={sector}, state={state:?}, left={:?}, right={:?}",
                            shifts[left].values(),
                            shifts[right].values(),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fixed_coordinates_cross_exactly_while_inactive_free_coordinates_stay_formal() {
    let ordering = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("10").unwrap(),
        PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let key = ordering.key_for_shift(&shift(&[-1, i64::MAX])).unwrap();

    assert_eq!(key.formal_sector().to_bit_string(), "00");
    assert_eq!(key.signed_index_excess(), &[0, -i128::from(i64::MAX)]);
    assert_eq!(key.propagators(), 0);
    assert_eq!(key.dots_offset(), 0);
    assert_eq!(key.numerators_offset(), -i128::from(i64::MAX));

    let inactive = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("0").unwrap(),
        PartialIndexAssignment::try_new([], 1, 0).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let minimum = inactive.key_for_shift(&shift(&[i64::MIN])).unwrap();
    assert_eq!(minimum.formal_sector().to_bit_string(), "0");
    assert_eq!(minimum.signed_index_excess(), &[i128::from(i64::MAX) + 1]);
}

#[test]
fn arity_fixed_index_and_resource_failures_are_typed_and_checked() {
    let sector = SectorMask::try_from_bit_string("11").unwrap();
    let wrong_assignment = PartialIndexAssignment::try_new([], 1, 0).unwrap();
    assert_eq!(
        CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            sector.clone(),
            wrong_assignment,
            CylindricalOrderingLimits::default(),
        ),
        Err(CylindricalOrderingError::WrongAssignmentArity {
            expected: 2,
            actual: 1,
        })
    );

    let outside = PartialIndexAssignment::try_new([(0, 0)], 2, 1).unwrap();
    assert!(matches!(
        CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            sector.clone(),
            outside,
            CylindricalOrderingLimits::default(),
        ),
        Err(
            CylindricalOrderingError::FixedAssignmentOutsideSourceSector {
                position: 0,
                value: 0,
                source_active: true,
            }
        )
    ));

    let ordering = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        sector.clone(),
        PartialIndexAssignment::try_new([(0, i64::MAX)], 2, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    assert_eq!(
        ordering.key_for_shift(&shift(&[1, 0])),
        Err(CylindricalOrderingError::FixedIndexOverflow {
            position: 0,
            value: i64::MAX,
            displacement: 1,
        })
    );
    let wrong_shift = IndexShift::try_new([0], 1).unwrap();
    assert_eq!(
        ordering.key_for_shift(&wrong_shift),
        Err(CylindricalOrderingError::WrongShiftArity {
            expected: 2,
            actual: 1,
        })
    );

    let inactive_overflow = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("0").unwrap(),
        PartialIndexAssignment::try_new([(0, i64::MIN)], 1, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    assert_eq!(
        inactive_overflow.key_for_shift(&shift(&[-1])),
        Err(CylindricalOrderingError::FixedIndexOverflow {
            position: 0,
            value: i64::MIN,
            displacement: -1,
        })
    );

    for limits in [
        CylindricalOrderingLimits {
            max_arity: 1,
            ..CylindricalOrderingLimits::default()
        },
        CylindricalOrderingLimits {
            max_fixed_assignments: 0,
            ..CylindricalOrderingLimits::default()
        },
        CylindricalOrderingLimits {
            // V1 retains 5 + 3*2 scalar components.
            max_key_components: 10,
            ..CylindricalOrderingLimits::default()
        },
    ] {
        assert!(matches!(
            CylindricalParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                sector.clone(),
                PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
                limits,
            ),
            Err(CylindricalOrderingError::ResourceLimitExceeded { .. })
        ));
    }

    let complete = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        sector.clone(),
        PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let one_byte_short = CylindricalOrderingLimits {
        max_manifest_bytes: complete.stable_manifest().len() - 1,
        ..CylindricalOrderingLimits::default()
    };
    assert!(matches!(
        CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            sector,
            PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
            one_byte_short,
        ),
        Err(CylindricalOrderingError::ResourceLimitExceeded {
            resource: "cylindrical ordering manifest bytes",
            ..
        })
    ));
    CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("11").unwrap(),
        PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
        CylindricalOrderingLimits {
            max_manifest_bytes: complete.stable_manifest().len(),
            ..CylindricalOrderingLimits::default()
        },
    )
    .unwrap();
}

#[test]
fn schemas_and_manifests_are_stable_and_keys_are_context_bound() {
    assert_eq!(
        CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA,
        "rustred-cylindrical-parametric-elimination-ordering-v1"
    );
    assert_eq!(
        CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
        "rustred-cylindrical-integral-complexity-key-v1"
    );
    assert_eq!(
        RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
        "arity,propagators,formal-sector-bits,signed-corner-distance-offset,signed-dots-offset,signed-numerators-offset,signed-index-excess,lattice-shift"
    );

    let ordering = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("101").unwrap(),
        PartialIndexAssignment::try_new([(1, -2)], 3, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let expected_manifest = "rustred-cylindrical-parametric-elimination-ordering-v1|policy=rustred.unshifted-sector-order.v1|key-schema=arity,propagators,formal-sector-bits,signed-corner-distance-offset,signed-dots-offset,signed-numerators-offset,signed-index-excess,lattice-shift|sector=101|assignment=[1:-2]|free=[0,2]";
    assert_eq!(
        ordering.schema(),
        CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA
    );
    assert_eq!(
        ordering.key_schema(),
        RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA
    );
    assert_eq!(ordering.stable_manifest(), expected_manifest);
    assert_eq!(ordering.free_positions(), &[0, 2]);

    let key = ordering.key_for_shift(&shift(&[2, 3, -1])).unwrap();
    assert_eq!(key.formal_sector().to_bit_string(), "111");
    assert_eq!(key.corner_distance_offset(), 1);
    assert_eq!(key.dots_offset(), 1);
    assert_eq!(key.numerators_offset(), 0);
    assert_eq!(key.signed_index_excess(), &[2, 0, -1]);
    assert_eq!(
        key.to_stable_string(),
        format!(
            "rustred-cylindrical-integral-complexity-key-v1|ordering-bytes={}|ordering={}|arity=3|propagators=3|sector=111|corner-offset=1|dots-offset=1|numerators-offset=0|excess=[2,0,-1]|shift=[2,3,-1]",
            expected_manifest.len(),
            expected_manifest,
        )
    );
    ordering.replay_key(&key).unwrap();

    let other = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("101").unwrap(),
        PartialIndexAssignment::try_new([(1, -3)], 3, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    let other_key = other.key_for_shift(&shift(&[2, 3, -1])).unwrap();
    assert_ne!(key, other_key);
    assert_ne!(key.cmp(&other_key), Ordering::Equal);
}
