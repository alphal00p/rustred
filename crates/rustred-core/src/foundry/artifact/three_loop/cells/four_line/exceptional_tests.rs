use std::collections::{BTreeMap, BTreeSet};

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, SourceViewConstruction};
use crate::foundry::parametric::ParametricRuleError;
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;

use super::exceptional::{
    ADJACENT_DOT_PAIR_TARGET_SHIFT, ExceptionalFourLineCells, OPPOSITE_DOT_PAIR_TARGET_SHIFT,
    THREE_DISTINCT_DOT_TARGET_SHIFT, TRIPLE_DOT_TARGET_SHIFT, adjacent_pair_search_depth,
    derive_adjacent_same_sector_candidate, derive_exceptional_four_line_cells,
    derive_three_distinct_dot_same_sector_candidate, derive_triple_dot_same_sector_candidate,
    fixed_base_corner, three_distinct_dot_search_depth, triple_dot_search_depth,
};
use super::*;

const BASE_CORNER: [i64; 6] = FOUR_LINE_SECTOR;
const ISOLATED_DOT: [i64; 6] = [0, 1, 1, 1, 2, 0];
const ADJACENT_DOT_PAIR: [i64; 6] = [0, 1, 1, 2, 2, 0];
const OPPOSITE_DOT_PAIR: [i64; 6] = [0, 1, 2, 1, 2, 0];
const TRIPLE_DOT: [i64; 6] = [0, 1, 1, 1, 3, 0];
const THREE_DISTINCT_DOTS: [i64; 6] = [0, 1, 2, 2, 2, 0];

#[test]
fn singleton_projections_replay_the_selected_generated_sources_exactly() {
    let ExceptionalFourLineCells {
        context,
        isolated,
        opposite,
        adjacent,
        triple,
        three_distinct,
    } = derive_exceptional_four_line_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    assert_eq!(zero_sectors.len(), 26);

    for (cell, offset, rows) in [
        (
            &isolated,
            [0; 6],
            vec!["ordinary-ibp:0:0", "ordinary-ibp:1:0"],
        ),
        (
            &opposite,
            [0, 0, 1, 0, 0, 0],
            vec![
                "ordinary-ibp:0:0",
                "ordinary-ibp:0:1",
                "ordinary-ibp:0:2",
                "ordinary-ibp:1:0",
                "ordinary-ibp:1:1",
                "ordinary-ibp:1:2",
                "ordinary-ibp:2:0",
                "ordinary-ibp:2:1",
                "ordinary-ibp:2:2",
            ],
        ),
    ] {
        assert_eq!(
            cell.sources()
                .provenance()
                .iter()
                .map(|source| source.translated().offset().values())
                .collect::<Vec<_>>(),
            vec![offset.as_slice(); rows.len()]
        );
        assert_eq!(
            cell.sources()
                .provenance()
                .iter()
                .map(|source| source.translated().source_row().stable_string())
                .collect::<Vec<_>>(),
            rows
        );
        assert!(
            cell.sources()
                .provenance()
                .iter()
                .all(|source| source.symmetry().is_none())
        );
        let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction()
        else {
            panic!("exceptional four-line sources must retain projection evidence")
        };
        assert_eq!(
            evidence.domain().bounds(),
            BASE_CORNER.map(|value| InteriorBounds::new(value, value))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_base_corner());
        assert_eq!(
            evidence.stabilizer_group_elements(),
            [0, 1, 2, 3, 20, 21, 22, 23]
        );
        assert_eq!(evidence.original_relations().len(), rows.len());
        assert_eq!(evidence.term_projections().len(), rows.len());
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
    }

    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(BASE_CORNER).unwrap(),
        adjacent_pair_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(search.offset_count(), 28);
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
    for (label, cell) in [
        ("adjacent", &adjacent),
        ("triple", &triple),
        ("three-distinct", &three_distinct),
    ] {
        assert_eq!(cell.sources().len(), 28 * 9);
        for (offset, sources) in search
            .offsets()
            .iter()
            .zip(cell.sources().provenance().chunks(9))
        {
            assert_eq!(sources.len(), ordinary_rows.len());
            for (source, row) in sources.iter().zip(ordinary_rows) {
                assert_eq!(source.translated().offset(), offset);
                assert_eq!(source.translated().source_row().stable_string(), row);
                assert!(source.symmetry().is_none());
            }
        }
        let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction()
        else {
            panic!("{label} four-line sources must retain projection evidence")
        };
        assert_eq!(
            evidence.domain().bounds(),
            BASE_CORNER.map(|value| InteriorBounds::new(value, value))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_base_corner());
        assert_eq!(
            evidence.stabilizer_group_elements(),
            [0, 1, 2, 3, 20, 21, 22, 23]
        );
        assert_eq!(evidence.original_relations().len(), 28 * 9);
        assert_eq!(evidence.term_projections().len(), 28 * 9);
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
    }
}

#[test]
fn exact_singleton_rules_own_dotted_orbits_and_deeper_descendants() {
    let ExceptionalFourLineCells {
        context,
        isolated,
        opposite,
        adjacent,
        triple,
        three_distinct,
    } = derive_exceptional_four_line_cells().unwrap();
    assert_eq!(isolated.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(opposite.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(adjacent.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(triple.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(three_distinct.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(isolated.rule().pivot().values(), CANONICAL_DOT_TARGET_SHIFT);
    assert_eq!(
        opposite.rule().pivot().values(),
        OPPOSITE_DOT_PAIR_TARGET_SHIFT
    );
    assert_eq!(
        adjacent.rule().pivot().values(),
        ADJACENT_DOT_PAIR_TARGET_SHIFT
    );
    assert_eq!(triple.rule().pivot().values(), TRIPLE_DOT_TARGET_SHIFT);
    assert_eq!(
        three_distinct.rule().pivot().values(),
        THREE_DISTINCT_DOT_TARGET_SHIFT
    );
    assert_eq!(rhs_shifts(&isolated), vec![[0, 0, 0, 0, 0, 0]]);
    assert_eq!(
        rhs_shifts(&opposite),
        vec![[0, 0, 0, 1, 1, 0], [0, 0, 0, 0, 2, 0], [0, 0, 0, 0, 1, 0],]
    );
    assert_eq!(
        rhs_shifts(&adjacent),
        vec![[0, -1, 1, 1, 1, 0], [0, 0, 0, 0, 0, 0]]
    );
    assert_eq!(
        rhs_shifts(&triple),
        vec![[0, -1, 1, 1, 1, 0], [0, 0, 0, 0, 0, 0]]
    );
    assert_eq!(
        rhs_shifts(&three_distinct),
        vec![[0, -1, 1, 1, 1, 0], [0, 0, 0, 0, 0, 0]]
    );
    assert_eq!(
        source_rows(&isolated),
        vec![
            (0, "ordinary-ibp:0:0".to_owned()),
            (1, "ordinary-ibp:1:0".to_owned()),
        ]
    );
    assert_eq!(
        source_rows(&opposite),
        vec![
            (0, "ordinary-ibp:0:0".to_owned()),
            (1, "ordinary-ibp:0:1".to_owned()),
            (2, "ordinary-ibp:0:2".to_owned()),
            (3, "ordinary-ibp:1:0".to_owned()),
            (5, "ordinary-ibp:1:2".to_owned()),
        ]
    );
    assert_eq!(isolated.rule().replay().source_rows_used(), 2);
    assert_eq!(opposite.rule().replay().source_rows_used(), 5);
    assert_eq!(adjacent.rule().replay().source_rows_used(), 16);
    assert_eq!(triple.rule().replay().source_rows_used(), 16);
    assert_eq!(three_distinct.rule().replay().source_rows_used(), 17);
    assert_eq!(three_distinct.rule().source_combination().len(), 17);
    let searched_source_span = [
        (81, "ordinary-ibp:0:0", [0, 0, 0, 0, 0, 0]),
        (84, "ordinary-ibp:1:0", [0, 0, 0, 0, 0, 0]),
        (99, "ordinary-ibp:0:0", [0, 0, 0, 0, 1, 0]),
        (100, "ordinary-ibp:0:1", [0, 0, 0, 0, 1, 0]),
        (101, "ordinary-ibp:0:2", [0, 0, 0, 0, 1, 0]),
        (102, "ordinary-ibp:1:0", [0, 0, 0, 0, 1, 0]),
        (104, "ordinary-ibp:1:2", [0, 0, 0, 0, 1, 0]),
        (108, "ordinary-ibp:0:0", [0, 0, 0, 0, 2, 0]),
        (109, "ordinary-ibp:0:1", [0, 0, 0, 0, 2, 0]),
        (110, "ordinary-ibp:0:2", [0, 0, 0, 0, 2, 0]),
        (135, "ordinary-ibp:0:0", [0, 0, 0, 1, 1, 0]),
        (136, "ordinary-ibp:0:1", [0, 0, 0, 1, 1, 0]),
        (139, "ordinary-ibp:1:1", [0, 0, 0, 1, 1, 0]),
        (140, "ordinary-ibp:1:2", [0, 0, 0, 1, 1, 0]),
        (171, "ordinary-ibp:0:0", [0, 0, 1, 0, 1, 0]),
        (172, "ordinary-ibp:0:1", [0, 0, 1, 0, 1, 0]),
    ];
    for cell in [&adjacent, &triple] {
        assert_eq!(
            cell.rule().source_combination().len(),
            searched_source_span.len()
        );
        for (source, (ordinal, row, offset)) in cell
            .rule()
            .source_combination()
            .iter()
            .zip(searched_source_span)
        {
            assert_eq!(source.source_ordinal(), ordinal);
            assert_eq!(source.row_id().stable_string(), row);
            assert_eq!(
                cell.sources().provenance()[ordinal]
                    .translated()
                    .offset()
                    .values(),
                offset
            );
        }
    }

    let dimension = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let dimension_minus_three = context.sub(&dimension, &context.integer(3)).unwrap();
    let dimension_minus_four = context.sub(&dimension, &context.integer(4)).unwrap();
    let dimension_minus_seven = context.sub(&dimension, &context.integer(7)).unwrap();
    let dimension_minus_two = context.sub(&dimension, &context.integer(2)).unwrap();
    let three_dimension = context.mul(&context.integer(3), &dimension).unwrap();
    let three_dimension_minus_eight = context.sub(&three_dimension, &context.integer(8)).unwrap();
    let three_dimension_minus_ten = context.sub(&three_dimension, &context.integer(10)).unwrap();
    let base_numerator = context
        .mul(&dimension_minus_three, &three_dimension_minus_eight)
        .and_then(|value| context.mul(&value, &three_dimension_minus_ten))
        .unwrap();
    let base_denominator = context
        .mul(&context.integer(64), &dimension_minus_four)
        .unwrap();
    let factorized_denominator = context
        .mul(&context.integer(4), &dimension_minus_four)
        .unwrap();
    let expected_adjacent_coefficients = [
        context
            .div(&context.integer(-1), &factorized_denominator)
            .unwrap(),
        context.div(&base_numerator, &base_denominator).unwrap(),
    ];
    assert_eq!(
        adjacent
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        expected_adjacent_coefficients.iter().collect::<Vec<_>>()
    );
    let triple_numerator = context
        .mul(&dimension_minus_seven, &three_dimension_minus_eight)
        .and_then(|value| context.mul(&value, &three_dimension_minus_ten))
        .unwrap();
    let triple_factorized_denominator = context
        .mul(&context.integer(8), &dimension_minus_four)
        .unwrap();
    let triple_scalar_denominator = context
        .mul(&context.integer(128), &dimension_minus_four)
        .unwrap();
    let expected_triple_coefficients = [
        context
            .div(&context.integer(3), &triple_factorized_denominator)
            .unwrap(),
        context
            .div(&triple_numerator, &triple_scalar_denominator)
            .unwrap(),
    ];
    assert_eq!(
        triple
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        expected_triple_coefficients.iter().collect::<Vec<_>>()
    );
    let eleven_dimension = context.mul(&context.integer(11), &dimension).unwrap();
    let three_distinct_factorized_numerator = context
        .sub(&context.integer(38), &eleven_dimension)
        .unwrap();
    let three_distinct_factorized_denominator = context
        .mul(&context.integer(32), &dimension_minus_four)
        .unwrap();
    let three_distinct_scalar_numerator = context
        .mul(&dimension_minus_three, &dimension_minus_two)
        .and_then(|value| context.mul(&value, &three_dimension_minus_eight))
        .and_then(|value| context.mul(&value, &three_dimension_minus_ten))
        .and_then(|value| context.mul(&context.integer(3), &value))
        .unwrap();
    let three_distinct_scalar_denominator = context
        .mul(&context.integer(512), &dimension_minus_four)
        .unwrap();
    let expected_three_distinct_coefficients = [
        context
            .div(
                &three_distinct_factorized_numerator,
                &three_distinct_factorized_denominator,
            )
            .unwrap(),
        context
            .div(
                &three_distinct_scalar_numerator,
                &three_distinct_scalar_denominator,
            )
            .unwrap(),
    ];
    assert_eq!(
        three_distinct
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        expected_three_distinct_coefficients
            .iter()
            .collect::<Vec<_>>()
    );

    let expected_adjacent_guards = [
        "-4+3*d",
        "6-3*d",
        "4-d",
        "32-8*d",
        "-16+4*d",
        "-256+64*d",
        "-128+32*d",
        "-32+8*d",
        "-8+2*d",
    ];
    assert_eq!(
        adjacent
            .rule()
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_adjacent_guards
    );
    let expected_triple_guards = [
        "-4+3*d",
        "6-3*d",
        "4-d",
        "32-8*d",
        "-32+8*d",
        "-512+128*d",
        "-256+64*d",
        "-64+16*d",
        "-16+4*d",
    ];
    assert_eq!(
        triple
            .rule()
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_triple_guards
    );
    let expected_three_distinct_guards = [
        "-4+3*d",
        "6-3*d",
        "4-d",
        "32-8*d",
        "-128+32*d",
        "-2048+512*d",
        "-1024+256*d",
        "-256+64*d",
        "-64+16*d",
    ];
    assert_eq!(
        three_distinct
            .rule()
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_three_distinct_guards
    );
    assert_eq!(
        three_distinct
            .guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_three_distinct_guards
    );
    assert_eq!(
        triple
            .guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_triple_guards
    );

    assert_eq!(
        adjacent
            .guards()
            .iter()
            .map(|guard| guard.polynomial().to_expression().to_string())
            .collect::<Vec<_>>(),
        expected_adjacent_guards
    );

    let expected_source_weights = [
        context
            .div(&context.integer(-3), &context.integer(2))
            .unwrap(),
        context.integer(-1),
        context
            .div(&context.integer(-1), &context.integer(2))
            .unwrap(),
        context
            .div(&context.integer(1), &context.integer(2))
            .unwrap(),
        context
            .div(&context.integer(-1), &context.integer(2))
            .unwrap(),
    ];
    assert_eq!(
        opposite
            .rule()
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        expected_source_weights.iter().collect::<Vec<_>>()
    );

    let expected_pivot_guards = [
        (
            0,
            "ordinary-ibp:0:0",
            1,
            [0, 0, 1, 0, 1, -1],
            context.integer(-1),
        ),
        (
            1,
            "ordinary-ibp:0:1",
            0,
            [0, 0, 1, 1, 0, -1],
            context.integer(-1),
        ),
        (
            2,
            "ordinary-ibp:0:2",
            2,
            [0, 0, 0, 1, 1, -1],
            context.integer(1),
        ),
        (
            3,
            "ordinary-ibp:1:0",
            3,
            [0, 0, 0, 0, 2, -1],
            context.integer(2),
        ),
        (
            5,
            "ordinary-ibp:1:2",
            4,
            OPPOSITE_DOT_PAIR_TARGET_SHIFT,
            context.integer(-2),
        ),
    ];
    let pivot_guards = opposite.rule().elimination_pivot_guards();
    assert_eq!(pivot_guards.len(), expected_pivot_guards.len());
    for (guard, (source, row, column, shift, coefficient)) in
        pivot_guards.iter().zip(&expected_pivot_guards)
    {
        assert_eq!(guard.source_ordinal(), *source);
        assert_eq!(guard.row_id().stable_string(), *row);
        assert_eq!(guard.pivot_column(), *column);
        assert_eq!(guard.pivot_shift().values(), shift);
        assert_eq!(guard.coefficient(), coefficient);
        assert_eq!(
            guard.nonzero_polynomial(),
            &context
                .numerator_condition_with_limits(coefficient, Default::default())
                .unwrap()
        );
    }

    let singleton_bounds = BASE_CORNER.map(|value| InteriorBounds::new(value, value));
    for cell in [&isolated, &opposite, &adjacent, &triple, &three_distinct] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(cell.application_domain().bounds(), singleton_bounds);
        assert_eq!(cell.fixed_restrictions(), fixed_base_corner());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }
    assert!(isolated.rule().nonzero_guards().is_empty());
    assert!(isolated.guards().is_empty());
    assert!(opposite.rule().nonzero_guards().is_empty());
    assert!(opposite.guards().is_empty());

    assert_eq!(
        isolated
            .assignment_for_target(&IntegralKey::try_new(ISOLATED_DOT).unwrap())
            .unwrap(),
        Some(BASE_CORNER.to_vec())
    );
    assert!(
        isolated
            .assignment_for_target(&IntegralKey::try_new(ADJACENT_DOT_PAIR).unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        opposite
            .assignment_for_target(&IntegralKey::try_new(OPPOSITE_DOT_PAIR).unwrap())
            .unwrap(),
        Some(BASE_CORNER.to_vec())
    );
    assert!(
        opposite
            .assignment_for_target(&IntegralKey::try_new(ADJACENT_DOT_PAIR).unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        adjacent
            .assignment_for_target(&IntegralKey::try_new(ADJACENT_DOT_PAIR).unwrap())
            .unwrap(),
        Some(BASE_CORNER.to_vec())
    );
    assert!(
        adjacent
            .assignment_for_target(&IntegralKey::try_new(OPPOSITE_DOT_PAIR).unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        triple
            .assignment_for_target(&IntegralKey::try_new(TRIPLE_DOT).unwrap())
            .unwrap(),
        Some(BASE_CORNER.to_vec())
    );
    assert!(
        triple
            .assignment_for_target(&IntegralKey::try_new([0, 1, 1, 1, 4, 0]).unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        three_distinct
            .assignment_for_target(&IntegralKey::try_new(THREE_DISTINCT_DOTS).unwrap())
            .unwrap(),
        Some(BASE_CORNER.to_vec())
    );
    assert!(
        three_distinct
            .assignment_for_target(&IntegralKey::try_new([0, 1, 2, 2, 3, 0]).unwrap())
            .unwrap()
            .is_none()
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for cell in [&triple, &three_distinct] {
        let children = cell
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| {
                IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
                    BASE_CORNER[position] + term.shift().values()[position]
                }))
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            children
                .iter()
                .map(|child| child.powers())
                .collect::<Vec<_>>(),
            [&[0, 0, 2, 2, 2, 0][..], &BASE_CORNER]
        );
        assert_eq!(
            canonicalizer
                .canonicalize(&children[0])
                .unwrap()
                .canonical()
                .powers(),
            [0, 0, 2, 0, 2, 2]
        );
        assert_eq!(children[1].powers(), BASE_CORNER);
    }
    for (first, second) in [(1, 2), (1, 4), (2, 3), (3, 4)] {
        let mut powers = FOUR_LINE_SECTOR;
        powers[first] = 2;
        powers[second] = 2;
        let routed = canonicalizer
            .canonicalize(&IntegralKey::try_new(powers).unwrap())
            .unwrap();
        assert_eq!(routed.canonical().powers(), ADJACENT_DOT_PAIR);
        assert_eq!(
            adjacent.assignment_for_target(routed.canonical()).unwrap(),
            Some(BASE_CORNER.to_vec())
        );
    }
    for dotted in 1..5 {
        let mut powers = FOUR_LINE_SECTOR;
        powers[dotted] = 3;
        let routed = canonicalizer
            .canonicalize(&IntegralKey::try_new(powers).unwrap())
            .unwrap();
        assert_eq!(routed.canonical().powers(), TRIPLE_DOT);
        assert_eq!(
            triple.assignment_for_target(routed.canonical()).unwrap(),
            Some(BASE_CORNER.to_vec())
        );
    }
    for undotted in 1..5 {
        let mut powers = FOUR_LINE_SECTOR;
        for dotted in 1..5 {
            if dotted != undotted {
                powers[dotted] = 2;
            }
        }
        let routed = canonicalizer
            .canonicalize(&IntegralKey::try_new(powers).unwrap())
            .unwrap();
        assert_eq!(routed.canonical().powers(), THREE_DISTINCT_DOTS);
        assert_eq!(
            three_distinct
                .assignment_for_target(routed.canonical())
                .unwrap(),
            Some(BASE_CORNER.to_vec())
        );
    }
}

#[test]
fn global_canonicalization_finds_one_dot_orbit_and_two_pair_orbits() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let canonical_dot = IntegralKey::try_new(ISOLATED_DOT).unwrap();
    for slot in 1..5 {
        let mut powers = FOUR_LINE_SECTOR;
        powers[slot] = 2;
        assert_eq!(
            canonicalizer
                .canonicalize(&IntegralKey::try_new(powers).unwrap())
                .unwrap()
                .canonical(),
            &canonical_dot
        );
    }

    let mut orbits = BTreeMap::<[i64; 6], BTreeSet<(usize, usize)>>::new();
    for first in 1..5 {
        for second in (first + 1)..5 {
            let mut powers = FOUR_LINE_SECTOR;
            powers[first] = 2;
            powers[second] = 2;
            let canonical: [i64; 6] = canonicalizer
                .canonicalize(&IntegralKey::try_new(powers).unwrap())
                .unwrap()
                .canonical()
                .powers()
                .try_into()
                .unwrap();
            orbits.entry(canonical).or_default().insert((first, second));
        }
    }
    assert_eq!(
        orbits,
        BTreeMap::from([
            (
                ADJACENT_DOT_PAIR,
                BTreeSet::from([(1, 2), (1, 4), (2, 3), (3, 4)]),
            ),
            (OPPOSITE_DOT_PAIR, BTreeSet::from([(1, 3), (2, 4)]),),
        ])
    );
}

#[test]
fn adjacent_pair_requires_the_complete_depth_two_same_sector_diamond() {
    let depth_zero = derive_adjacent_same_sector_candidate(0);
    let depth_one = derive_adjacent_same_sector_candidate(1);
    assert_eq!(
        depth_zero,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    assert_eq!(
        depth_one,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftNotPivot
        ))
    );
    assert_eq!(ADJACENT_DOT_PAIR_TARGET_SHIFT, [0, 0, 0, 1, 1, 0]);
}

#[test]
fn triple_dot_requires_the_complete_depth_two_same_sector_diamond() {
    let depth_zero = derive_triple_dot_same_sector_candidate(0);
    let depth_one = derive_triple_dot_same_sector_candidate(1);
    assert_eq!(
        depth_zero,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    assert_eq!(
        depth_one,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftNotPivot
        ))
    );
    assert_eq!(triple_dot_search_depth(), 2);
    assert_eq!(TRIPLE_DOT_TARGET_SHIFT, [0, 0, 0, 0, 2, 0]);
}

#[test]
fn three_distinct_dots_require_the_complete_depth_two_same_sector_diamond() {
    let depth_zero = derive_three_distinct_dot_same_sector_candidate(0);
    let depth_one = derive_three_distinct_dot_same_sector_candidate(1);
    for candidate in [depth_zero, depth_one] {
        assert_eq!(
            candidate,
            Err(ArtifactError::ParametricRule(
                ParametricRuleError::TargetShiftAbsent
            ))
        );
    }
    assert_eq!(three_distinct_dot_search_depth(), 2);
    assert_eq!(THREE_DISTINCT_DOT_TARGET_SHIFT, [0, 0, 1, 1, 1, 0]);
}

fn rhs_shifts(cell: &RuleCell) -> Vec<[i64; 6]> {
    cell.rule()
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values().try_into().unwrap())
        .collect()
}

fn source_rows(cell: &RuleCell) -> Vec<(usize, String)> {
    cell.rule()
        .source_combination()
        .iter()
        .map(|source| (source.source_ordinal(), source.row_id().stable_string()))
        .collect()
}
