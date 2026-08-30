use std::collections::{BTreeMap, BTreeSet};

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, SourceViewConstruction};
use crate::foundry::parametric::ParametricRuleError;
use crate::sector::InteriorBounds;

use super::exceptional::{
    ADJACENT_DOT_PAIR_TARGET_SHIFT, OPPOSITE_DOT_PAIR_TARGET_SHIFT,
    derive_adjacent_full_span_candidate, derive_exceptional_four_line_cells, fixed_base_corner,
};
use super::*;

const BASE_CORNER: [i64; 6] = FOUR_LINE_SECTOR;
const ISOLATED_DOT: [i64; 6] = [0, 1, 1, 1, 2, 0];
const ADJACENT_DOT_PAIR: [i64; 6] = [0, 1, 1, 2, 2, 0];
const OPPOSITE_DOT_PAIR: [i64; 6] = [0, 1, 2, 1, 2, 0];

#[test]
fn singleton_projections_replay_the_selected_generated_sources_exactly() {
    let (context, isolated, opposite) = derive_exceptional_four_line_cells().unwrap();
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
}

#[test]
fn exact_singleton_rules_own_the_isolated_dot_and_opposite_pair_only() {
    let (context, isolated, opposite) = derive_exceptional_four_line_cells().unwrap();
    assert_eq!(isolated.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(opposite.rule().anchor().powers(), BASE_CORNER);
    assert_eq!(isolated.rule().pivot().values(), CANONICAL_DOT_TARGET_SHIFT);
    assert_eq!(
        opposite.rule().pivot().values(),
        OPPOSITE_DOT_PAIR_TARGET_SHIFT
    );
    assert_eq!(rhs_shifts(&isolated), vec![[0, 0, 0, 0, 0, 0]]);
    assert_eq!(
        rhs_shifts(&opposite),
        vec![[0, 0, 0, 1, 1, 0], [0, 0, 0, 0, 2, 0], [0, 0, 0, 0, 1, 0],]
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
    for cell in [&isolated, &opposite] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(cell.application_domain().bounds(), singleton_bounds);
        assert_eq!(cell.fixed_restrictions(), fixed_base_corner());
        assert!(cell.rule().nonzero_guards().is_empty());
        assert!(cell.guards().is_empty());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }

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
fn adjacent_pair_full_spans_remain_typed_open_obligations() {
    let zero = derive_adjacent_full_span_candidate([0; 6]);
    let translated = derive_adjacent_full_span_candidate([0, 0, 0, 1, 0, 0]);
    assert_eq!(
        zero,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    assert_eq!(
        translated,
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftNotPivot
        ))
    );
    assert_eq!(ADJACENT_DOT_PAIR_TARGET_SHIFT, [0, 0, 0, 1, 1, 0]);
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
