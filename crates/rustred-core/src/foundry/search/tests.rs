use crate::family::IntegralKey;
use crate::sector::Mask;

use super::build::checked_coordinate_cells_for_test;
use super::{SectorSearchDiamond, SectorSearchError, SectorSearchLimits};

fn limits(depth: usize, offsets: usize, coordinate_cells: usize) -> SectorSearchLimits {
    SectorSearchLimits {
        max_depth: depth,
        max_offsets: offsets,
        max_offset_coordinate_cells: coordinate_cells,
    }
}

fn values(diamond: &SectorSearchDiamond) -> Vec<Vec<i64>> {
    diamond
        .offsets()
        .iter()
        .map(|offset| offset.values().to_vec())
        .collect()
}

#[test]
fn k6_corner_sign_cone_has_exact_depth_counts() {
    for (depth, expected) in [(0, 1), (1, 7), (2, 28)] {
        let anchor = IntegralKey::try_new([0, 1, 1, 1, 1, 0]).unwrap();
        let diamond =
            SectorSearchDiamond::try_new(anchor, depth, limits(depth, expected, 6 * expected))
                .unwrap();
        assert_eq!(diamond.depth(), depth);
        assert_eq!(diamond.offset_count(), expected);
        assert_eq!(diamond.anchor().powers(), [0, 1, 1, 1, 1, 0]);
        assert!(diamond.offsets().iter().all(|offset| {
            offset.values()[0] <= 0
                && offset.values()[5] <= 0
                && offset.values()[1..5]
                    .iter()
                    .all(|&component| component >= 0)
        }));
    }
}

#[test]
fn noncorner_anchor_admits_both_signs_without_leaving_its_sector() {
    let anchor = IntegralKey::try_new([3, -2]).unwrap();
    let expected_sector = Mask::try_from_indices(anchor.powers()).unwrap();
    let diamond = SectorSearchDiamond::try_new(anchor, 2, limits(2, 13, 26)).unwrap();

    assert_eq!(diamond.offset_count(), 13);
    assert!(
        diamond
            .offsets()
            .iter()
            .any(|offset| offset.values()[0] < 0)
    );
    assert!(
        diamond
            .offsets()
            .iter()
            .any(|offset| offset.values()[0] > 0)
    );
    assert!(
        diamond
            .offsets()
            .iter()
            .any(|offset| offset.values()[1] < 0)
    );
    assert!(
        diamond
            .offsets()
            .iter()
            .any(|offset| offset.values()[1] > 0)
    );
    for offset in diamond.offsets() {
        let shifted = diamond
            .anchor()
            .powers()
            .iter()
            .zip(offset.values())
            .map(|(&power, &shift)| power.checked_add(shift).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(Mask::try_from_indices(&shifted).unwrap(), expected_sector);
    }
}

#[test]
fn offsets_are_complete_unique_lexicographic_and_inside_the_l1_bound() {
    let first = SectorSearchDiamond::try_new(
        IntegralKey::try_new([2, -1, 3]).unwrap(),
        2,
        limits(2, 25, 75),
    )
    .unwrap();
    let second = SectorSearchDiamond::try_new(
        IntegralKey::try_new([2, -1, 3]).unwrap(),
        2,
        limits(2, 25, 75),
    )
    .unwrap();
    assert_eq!(first, second);
    assert!(first.offsets().windows(2).all(|pair| pair[0] < pair[1]));
    assert!(first.offsets().iter().all(|offset| {
        offset
            .values()
            .iter()
            .map(|value| value.unsigned_abs())
            .sum::<u64>()
            <= 2
    }));

    let mut expected = Vec::new();
    for first in -2_i64..=2 {
        for second in -2_i64..=2 {
            for third in -2_i64..=2 {
                let offset = [first, second, third];
                if offset.iter().map(|value| value.unsigned_abs()).sum::<u64>() <= 2
                    && [2_i64, -1, 3].iter().zip(offset).all(|(&power, shift)| {
                        let shifted = power.checked_add(shift).unwrap();
                        (power >= 1) == (shifted >= 1)
                    })
                {
                    expected.push(offset.to_vec());
                }
            }
        }
    }
    assert_eq!(values(&first), expected);
}

#[test]
fn endpoint_anchors_discard_only_unrepresentable_directions() {
    let maximum = SectorSearchDiamond::try_new(
        IntegralKey::try_new([i64::MAX]).unwrap(),
        2,
        limits(2, 3, 3),
    )
    .unwrap();
    assert_eq!(values(&maximum), vec![vec![-2], vec![-1], vec![0]]);

    let minimum = SectorSearchDiamond::try_new(
        IntegralKey::try_new([i64::MIN]).unwrap(),
        2,
        limits(2, 3, 3),
    )
    .unwrap();
    assert_eq!(values(&minimum), vec![vec![0], vec![1], vec![2]]);

    for diamond in [&maximum, &minimum] {
        for offset in diamond.offsets() {
            assert!(
                diamond.anchor().powers()[0]
                    .checked_add(offset.values()[0])
                    .is_some()
            );
        }
    }
}

#[test]
fn exact_resource_boundaries_are_admitted_and_one_below_is_typed() {
    let anchor = || IntegralKey::try_new([0, 1, 1, 1, 1, 0]).unwrap();
    SectorSearchDiamond::try_new(anchor(), 2, limits(2, 28, 168)).unwrap();

    assert_eq!(
        SectorSearchDiamond::try_new(anchor(), 2, limits(1, 28, 168)),
        Err(SectorSearchError::ResourceLimit {
            resource: "sector-search depth",
            requested: 2,
            limit: 1,
        })
    );
    assert_eq!(
        SectorSearchDiamond::try_new(anchor(), 2, limits(2, 27, 168)),
        Err(SectorSearchError::ResourceLimit {
            resource: "sector-search retained offsets",
            requested: 28,
            limit: 27,
        })
    );
    assert_eq!(
        SectorSearchDiamond::try_new(anchor(), 2, limits(2, 28, 167)),
        Err(SectorSearchError::ResourceLimit {
            resource: "sector-search retained offset coordinate cells",
            requested: 168,
            limit: 167,
        })
    );
}

#[test]
fn unrepresentable_depth_and_checked_count_overflows_are_typed() {
    if usize::BITS >= i64::BITS {
        let depth = usize::try_from(i64::MAX).unwrap() + 1;
        assert_eq!(
            SectorSearchDiamond::try_new(
                IntegralKey::try_new([1]).unwrap(),
                depth,
                limits(usize::MAX, usize::MAX, usize::MAX),
            ),
            Err(SectorSearchError::DepthNotRepresentable { depth })
        );
    }

    assert_eq!(
        checked_coordinate_cells_for_test(usize::MAX, 2),
        Err(SectorSearchError::ResourceCountOverflow {
            resource: "sector-search retained offset coordinate cells",
        })
    );

    let overflow = SectorSearchDiamond::try_new(
        IntegralKey::try_new([100; 64]).unwrap(),
        64,
        limits(64, usize::MAX, usize::MAX),
    );
    assert_eq!(
        overflow,
        Err(SectorSearchError::ResourceCountOverflow {
            resource: "sector-search retained offsets",
        })
    );
}
