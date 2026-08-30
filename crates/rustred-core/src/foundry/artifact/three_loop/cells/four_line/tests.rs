use std::collections::BTreeSet;

use crate::algebra::{IndexedAlgebraLimits, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::cell::{ResidualTermDisposition, RuleCellDomainProof, SourceViewConstruction};
use crate::foundry::parametric::ParametricRuleError;

use super::*;

#[test]
fn projected_source_spans_replay_all_nine_rows_on_the_exact_four_line_face() {
    let (context, canonical_dot, mixed) = derive_four_line_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    assert_eq!(zero_sectors.len(), 26);

    let expected_projection_counts = [
        [1, 0, 7],
        [4, 0, 7],
        [4, 0, 7],
        [3, 0, 8],
        [3, 0, 5],
        [3, 0, 8],
        [3, 0, 8],
        [3, 0, 8],
        [3, 0, 5],
    ];

    for (cell, translation) in [
        (&canonical_dot, CANONICAL_TARGET_SOURCE_SHIFT),
        (&mixed, ZERO_SOURCE_SHIFT),
    ] {
        let sources = cell.sources();
        assert_eq!(sources.len(), 9);
        assert_eq!(
            sources
                .provenance()
                .iter()
                .map(|source| source.translated().offset().values())
                .collect::<Vec<_>>(),
            vec![translation.as_slice(); 9]
        );
        assert_eq!(
            sources
                .provenance()
                .iter()
                .map(|source| source.translated().source_row().stable_string())
                .collect::<Vec<_>>(),
            [
                "ordinary-ibp:0:0",
                "ordinary-ibp:0:1",
                "ordinary-ibp:0:2",
                "ordinary-ibp:1:0",
                "ordinary-ibp:1:1",
                "ordinary-ibp:1:2",
                "ordinary-ibp:2:0",
                "ordinary-ibp:2:1",
                "ordinary-ibp:2:2",
            ]
        );
        assert!(
            sources
                .provenance()
                .iter()
                .all(|source| source.symmetry().is_none())
        );
        let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
            panic!("four-line sources must retain residual-projection evidence")
        };
        assert_eq!(
            evidence.domain().sector().active_bits(),
            [false, true, true, true, true, false]
        );
        assert_eq!(
            evidence.domain().bounds(),
            [
                InteriorBounds::new(0, 0),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(0, 0),
            ]
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_inactive_indices());
        // Only the identity fixes every point of this symbolic face. The
        // larger setwise four-line stabilizer is checked independently below.
        assert_eq!(evidence.stabilizer_group_elements(), [0]);
        assert_eq!(
            evidence
                .term_projections()
                .iter()
                .map(|terms| {
                    terms.iter().fold([0_usize; 3], |mut counts, term| {
                        let slot = match term.disposition() {
                            ResidualTermDisposition::CoefficientZero => 0,
                            ResidualTermDisposition::ProvedZero { .. } => 1,
                            ResidualTermDisposition::Routed { .. } => 2,
                        };
                        counts[slot] += 1;
                        counts
                    })
                })
                .collect::<Vec<_>>(),
            expected_projection_counts
        );
        assert!(
            sources
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
fn exact_rules_target_the_canonical_dot_subcell_and_mixed_boundary() {
    let (_context, canonical_dot, mixed) = derive_four_line_cells().unwrap();
    assert_eq!(canonical_dot.rule().anchor().powers(), ANCHOR);
    assert_eq!(mixed.rule().anchor().powers(), ANCHOR);
    assert_eq!(
        canonical_dot.rule().pivot().values(),
        CANONICAL_DOT_TARGET_SHIFT
    );
    assert_eq!(
        mixed.rule().pivot().values(),
        MIXED_NUMERATOR_DOT_TARGET_SHIFT
    );
    assert_eq!(
        canonical_dot
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, -1, 1, 0, 1, 0][..],
            &[0, -1, 0, 1, 1, 0],
            &[0, -1, 0, 0, 2, 0],
            &[0, -1, 0, 0, 1, 0],
        ]
    );
    assert_eq!(
        mixed
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, 0, 0, 1, 0, 0][..],
            &[0, 0, 0, 0, 1, 0],
            &[0, 0, 0, 0, 0, 0],
            &[0, 0, 0, -1, 1, 0],
        ]
    );
    assert_eq!(
        source_rows(&canonical_dot),
        [
            (0, "ordinary-ibp:0:0".to_owned()),
            (4, "ordinary-ibp:1:1".to_owned()),
            (8, "ordinary-ibp:2:2".to_owned()),
        ]
    );
    assert_eq!(
        source_rows(&mixed),
        [
            (0, "ordinary-ibp:0:0".to_owned()),
            (6, "ordinary-ibp:2:0".to_owned()),
        ]
    );
    assert_eq!(canonical_dot.rule().replay().source_rows_used(), 3);
    assert_eq!(mixed.rule().replay().source_rows_used(), 2);
    for cell in [&canonical_dot, &mixed] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(cell.terms().len(), cell.rule().right_hand_side().len());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let canonical_dotted_corner = IntegralKey::try_new([0, 1, 1, 1, 2, 0]).unwrap();
    for slot in 1..5 {
        let mut dotted = FOUR_LINE_SECTOR;
        dotted[slot] = 2;
        assert_eq!(
            canonicalizer
                .canonicalize(&IntegralKey::try_new(dotted).unwrap())
                .unwrap()
                .canonical(),
            &canonical_dotted_corner
        );
    }
    let canonical_mixed_corner = IntegralKey::try_new([0, 1, 1, 1, 2, -1]).unwrap();
    for inactive_slot in [0, 5] {
        for active_slot in 1..5 {
            let mut mixed_target = FOUR_LINE_SECTOR;
            mixed_target[inactive_slot] = -1;
            mixed_target[active_slot] = 2;
            assert_eq!(
                canonicalizer
                    .canonicalize(&IntegralKey::try_new(mixed_target).unwrap())
                    .unwrap()
                    .canonical(),
                &canonical_mixed_corner
            );
        }
    }
    let canonical_anchor_target = IntegralKey::try_new([0, 2, 2, 2, 3, 0]).unwrap();
    assert_eq!(
        canonicalizer
            .canonicalize(&canonical_anchor_target)
            .unwrap()
            .canonical(),
        &canonical_anchor_target
    );
    let canonical_mixed_anchor_target = IntegralKey::try_new([0, 2, 2, 2, 3, -1]).unwrap();
    assert_eq!(
        canonicalizer
            .canonicalize(&canonical_mixed_anchor_target)
            .unwrap()
            .canonical(),
        &canonical_mixed_anchor_target
    );
    let setwise_stabilizer = canonicalizer
        .group_elements()
        .filter(|mapping| {
            [mapping[0], mapping[5]]
                .into_iter()
                .collect::<BTreeSet<_>>()
                == BTreeSet::from([0, 5])
        })
        .collect::<Vec<_>>();
    assert_eq!(setwise_stabilizer.len(), 8);
    assert_eq!(
        setwise_stabilizer
            .iter()
            .map(|mapping| mapping[1])
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3, 4])
    );
}

#[test]
fn guards_and_application_boxes_are_exact_on_both_four_line_cells() {
    let (context, canonical_dot, mixed) = derive_four_line_cells().unwrap();
    assert_affine_guards(
        &context,
        &canonical_dot,
        &[(0, -1, 3), (1, 1, 4), (2, -2, 1), (-1, 1, 1), (-2, 2, 1)],
    );
    assert_affine_guards(&context, &mixed, &[(0, -1, 3), (0, -1, 4), (0, 1, 4)]);
    assert_eq!(
        canonical_dot.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(2, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 2),
            InteriorBounds::new(0, 0),
        ]
    );
    assert_eq!(
        mixed.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(0, 0),
        ]
    );
    assert_eq!(canonical_dot.fixed_restrictions(), fixed_inactive_indices());
    assert_eq!(mixed.fixed_restrictions(), fixed_inactive_indices());

    let canonical_target = IntegralKey::try_new([0, 2, 2, 2, 3, 0]).unwrap();
    assert_eq!(
        canonical_dot
            .assignment_for_target(&canonical_target)
            .unwrap(),
        Some(ANCHOR.to_vec())
    );
    // The target-aligned source has exact (n1 - 1) guards. Its largest
    // guard-safe positive box starts at n1 = 2, so this cell deliberately
    // does not claim the isolated one-dot scalar corner.
    assert!(
        canonical_dot
            .assignment_for_target(&IntegralKey::try_new([0, 1, 1, 1, 2, 0]).unwrap())
            .unwrap()
            .is_none()
    );
    let mixed_target = IntegralKey::try_new([0, 2, 2, 2, 3, -1]).unwrap();
    assert_eq!(
        mixed.assignment_for_target(&mixed_target).unwrap(),
        Some(ANCHOR.to_vec())
    );
    assert_eq!(
        mixed
            .assignment_for_target(&IntegralKey::try_new([0, 1, 1, 1, 2, -1]).unwrap())
            .unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    for (cell, assignment) in [(&canonical_dot, ANCHOR), (&mixed, FOUR_LINE_SECTOR)] {
        assert!(cell.guards().iter().all(|guard| {
            !context
                .specialize_polynomial(
                    guard.polynomial(),
                    &assignment,
                    IndexedAlgebraLimits::default(),
                )
                .unwrap()
                .is_zero()
        }));
    }
    assert!(
        canonical_dot
            .assignment_for_target(&IntegralKey::try_new([0, 2, i64::MAX, 2, 3, 0]).unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        mixed
            .assignment_for_target(&IntegralKey::try_new([0, 1, 1, i64::MAX, 2, -1]).unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn zero_translation_does_not_mislabel_a_free_canonical_dot_as_a_rule_pivot() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let (completed, source_count) = complete_ordinary_sources(&generator).unwrap();
    let sources = projected_sources(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
        ZERO_SOURCE_SHIFT,
    )
    .unwrap();
    assert_eq!(
        derive_sector_monotone_rule_for_target(
            generator.context(),
            sources.relations(),
            &ANCHOR,
            &CANONICAL_DOT_TARGET_SHIFT,
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::TargetShiftNotPivot)
    );
}

fn source_rows(cell: &RuleCell) -> Vec<(usize, String)> {
    cell.rule()
        .source_combination()
        .iter()
        .map(|source| (source.source_ordinal(), source.row_id().stable_string()))
        .collect()
}

fn assert_affine_guards(
    context: &IndexedCoefficientContext,
    cell: &RuleCell,
    expected: &[(i64, i64, usize)],
) {
    assert_eq!(cell.rule().nonzero_guards().len(), expected.len());
    assert_eq!(cell.guards().len(), expected.len());
    for (guard, &(constant, multiplier, index)) in cell.rule().nonzero_guards().iter().zip(expected)
    {
        let variable = context.index(index).unwrap();
        let scaled = context
            .mul(&context.integer(multiplier), &variable)
            .unwrap();
        let affine = context.add(&context.integer(constant), &scaled).unwrap();
        assert_eq!(guard.polynomial().raw(), &affine.raw().numerator);
    }
}
