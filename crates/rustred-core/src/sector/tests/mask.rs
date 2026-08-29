use std::sync::Arc;

use super::super::{Mask, OrderingPolicy};
use super::support::all_indices;

#[test]
fn raw_membership_is_exhaustive_and_power_shift_independent() {
    for arity in 1..=4 {
        for indices in all_indices(arity, -2, 2) {
            let sector = Mask::try_from_indices(&indices).unwrap();
            assert_eq!(
                sector.active_bits(),
                indices.iter().map(|&index| index >= 1).collect::<Vec<_>>()
            );
            // Deliberately vary arbitrary would-be PowerShifts. The API has no
            // slot for them: membership and ordering remain a function of raw
            // n alone.
            for ignored_power_shifts in
                [vec![0_i64; arity], vec![1_i64; arity], vec![-7_i64; arity]]
            {
                let _ = ignored_power_shifts;
                assert_eq!(Mask::try_from_indices(&indices).unwrap(), sector);
                assert_eq!(
                    OrderingPolicy::default().complexity_key(&indices).unwrap(),
                    OrderingPolicy::default().complexity_key(&indices).unwrap()
                );
            }
        }
    }
}

#[test]
fn bit_orientation_display_and_corner_iterator_match_litered() {
    let sector = Mask::try_new([true, false, true, false, false, true]).unwrap();
    assert_eq!(
        sector.active_bits(),
        &[true, false, true, false, false, true]
    );
    assert_eq!(sector.to_string(), "101001");
    assert_eq!(
        sector.corner_indices().collect::<Vec<_>>(),
        vec![1, 0, 1, 0, 0, 1]
    );
    assert_eq!(sector.with_activity(1, true).unwrap().to_string(), "111001");

    let cloned = sector.clone();
    assert!(Arc::ptr_eq(&sector.active, &cloned.active));
    assert_eq!(sector, cloned);
}

#[test]
fn subset_relations_form_the_boolean_lattice_exhaustively() {
    let sectors = (0_u8..16)
        .map(|bits| {
            Mask::try_new((0..4).map(|position| bits & (1 << (3 - position)) != 0)).unwrap()
        })
        .collect::<Vec<_>>();
    for (left_bits, left) in sectors.iter().enumerate() {
        for (right_bits, right) in sectors.iter().enumerate() {
            let expected = (left_bits & !right_bits) == 0;
            assert_eq!(left.is_subsector_of(right).unwrap(), expected);
            assert_eq!(
                left.is_strict_subsector_of(right).unwrap(),
                expected && left_bits != right_bits
            );
        }
    }
}
