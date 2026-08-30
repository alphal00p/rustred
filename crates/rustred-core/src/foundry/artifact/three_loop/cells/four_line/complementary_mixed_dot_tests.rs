use std::collections::BTreeMap;

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::ParametricRuleError;
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::FOUR_LINE_SECTOR;
use super::complementary_mixed_dot::{
    COMPLEMENTARY_MIXED_DOT_TARGET_SHIFT, ComplementaryMixedDotCell,
    complementary_mixed_dot_search_depth, derive_complementary_mixed_dot_candidate,
    derive_complementary_mixed_dot_cell,
};
use super::corner::fixed_base_corner;

const TARGET: [i64; 6] = [0, 1, 2, 3, 2, 0];
const PRIMARY_RAY_ORIENTATION: [i64; 6] = [0, 1, 2, 2, 3, 0];
const EXPECTED_RHS: [[i64; 6]; 4] = [
    [0, 0, 0, 1, 3, 0],
    [0, 0, 0, 0, 4, 0],
    [0, 0, 0, 0, 0, 0],
    [0, -1, 0, 0, 1, 0],
];

#[test]
fn complementary_mixed_dot_singleton_retains_complete_exact_evidence() {
    let ComplementaryMixedDotCell { context, cell } =
        derive_complementary_mixed_dot_cell().unwrap();
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        complementary_mixed_dot_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(search.offset_count(), 84);
    assert_eq!(cell.sources().len(), 84 * 9);

    let ordinary_rows = [
        "ordinary-ibp:0:0",
        "ordinary-ibp:0:1",
        "ordinary-ibp:0:2",
        "ordinary-ibp:1:0",
        "ordinary-ibp:1:1",
        "ordinary-ibp:1:2",
        "ordinary-ibp:2:0",
        "ordinary-ibp:2:1",
        "ordinary-ibp:2:2",
    ];
    for (offset, provenance) in search
        .offsets()
        .iter()
        .zip(cell.sources().provenance().chunks(ordinary_rows.len()))
    {
        assert_eq!(provenance.len(), ordinary_rows.len());
        for (source, row) in provenance.iter().zip(ordinary_rows) {
            assert_eq!(source.translated().offset(), offset);
            assert_eq!(source.translated().source_row().stable_string(), row);
            assert!(source.symmetry().is_none());
        }
    }

    let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction() else {
        panic!("the complementary singleton must retain residual-projection evidence")
    };
    assert_eq!(
        evidence.domain().bounds(),
        FOUR_LINE_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(evidence.fixed_restrictions(), fixed_base_corner());
    assert_eq!(
        evidence.stabilizer_group_elements(),
        [0, 1, 2, 3, 20, 21, 22, 23]
    );
    assert_eq!(evidence.original_relations().len(), 84 * 9);
    assert_eq!(evidence.term_projections().len(), 84 * 9);
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    assert!(
        cell.sources()
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );

    assert_eq!(cell.rule().anchor().powers(), FOUR_LINE_SECTOR);
    assert_eq!(
        cell.rule().pivot().values(),
        COMPLEMENTARY_MIXED_DOT_TARGET_SHIFT
    );
    assert_eq!(
        cell.rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        EXPECTED_RHS
            .iter()
            .map(|shift| shift.as_slice())
            .collect::<Vec<_>>()
    );
    assert_eq!(cell.rule().source_combination().len(), 46);
    assert!(
        cell.rule()
            .source_combination()
            .windows(2)
            .all(|pair| pair[0].source_ordinal() < pair[1].source_ordinal())
    );
    for contribution in cell.rule().source_combination() {
        let provenance = &cell.sources().provenance()[contribution.source_ordinal()];
        assert_eq!(contribution.row_id(), provenance.translated().source_row());
    }
    assert_eq!(cell.rule().replay().source_rows_used(), 46);
    assert_eq!(cell.rule().replay().exact_operations(), 683);
    let concrete = cell.rule().concrete_replay();
    assert_eq!(concrete.source_contributions_checked(), 46);
    assert_eq!(concrete.source_terms_checked(), 310);
    assert_eq!(concrete.right_hand_side_terms_checked(), 4);
    assert_eq!(concrete.integral_keys_checked(), 315);
    assert_eq!(concrete.nonzero_guards_checked(), 22);
    assert_eq!(concrete.exact_operations(), 939);
    assert_eq!(cell.rule().nonzero_guards().len(), 22);
    assert_eq!(cell.guards().len(), 22);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        cell.application_domain().bounds(),
        FOUR_LINE_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(cell.fixed_restrictions(), fixed_base_corner());
    assert!(cell.terms().iter().all(|term| term.descent().verify()));

    let children = cell
        .rule()
        .right_hand_side()
        .iter()
        .map(|term| {
            let raw = IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
                FOUR_LINE_SECTOR[position] + term.shift().values()[position]
            }))
            .unwrap();
            canonicalizer
                .canonicalize(&raw)
                .unwrap()
                .canonical()
                .powers()
                .to_vec()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        children,
        [
            vec![0, 1, 1, 2, 4, 0],
            vec![0, 1, 1, 1, 5, 0],
            FOUR_LINE_SECTOR.to_vec(),
            vec![0, 0, 1, 0, 2, 1],
        ]
    );
}

#[test]
fn complementary_mixed_dot_singleton_owns_only_its_s4_orbit_point() {
    let ComplementaryMixedDotCell { context: _, cell } =
        derive_complementary_mixed_dot_cell().unwrap();
    assert_eq!(
        cell.assignment_for_target(&IntegralKey::try_new(TARGET).unwrap())
            .unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    for unowned in [
        PRIMARY_RAY_ORIENTATION,
        [0, 1, 2, 4, 2, 0],
        [0, 1, 2, 5, 2, 0],
    ] {
        assert!(
            cell.assignment_for_target(&IntegralKey::try_new(unowned).unwrap())
                .unwrap()
                .is_none()
        );
    }

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    assert_eq!(
        canonical_permutation_counts(&canonicalizer, [1, 2, 2, 3]),
        BTreeMap::from([(PRIMARY_RAY_ORIENTATION.to_vec(), 16), (TARGET.to_vec(), 8)])
    );
}

#[test]
fn complementary_mixed_dot_singleton_first_appears_at_depth_three() {
    for depth in 0..complementary_mixed_dot_search_depth() {
        assert_eq!(
            derive_complementary_mixed_dot_candidate(depth),
            Err(ArtifactError::ParametricRule(
                ParametricRuleError::TargetShiftAbsent
            ))
        );
    }
    assert_eq!(complementary_mixed_dot_search_depth(), 3);
}

fn canonical_permutation_counts(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    active_powers: [i64; 4],
) -> BTreeMap<Vec<i64>, usize> {
    let mut counts = BTreeMap::new();
    for first in 0..4 {
        for second in 0..4 {
            if second == first {
                continue;
            }
            for third in 0..4 {
                if third == first || third == second {
                    continue;
                }
                for fourth in 0..4 {
                    if fourth == first || fourth == second || fourth == third {
                        continue;
                    }
                    let permutation = [first, second, third, fourth];
                    let raw = IntegralKey::try_new([
                        0,
                        active_powers[permutation[0]],
                        active_powers[permutation[1]],
                        active_powers[permutation[2]],
                        active_powers[permutation[3]],
                        0,
                    ])
                    .unwrap();
                    let canonical = canonicalizer.canonicalize(&raw).unwrap();
                    *counts
                        .entry(canonical.canonical().powers().to_vec())
                        .or_default() += 1;
                }
            }
        }
    }
    counts
}
