use std::collections::BTreeSet;

use crate::algebra::IndexedAlgebraLimits;
use crate::foundry::cell::{ResidualTermDisposition, RuleCellDomainProof, SourceViewConstruction};
use symbolica::prelude::Integer;

use super::*;

#[test]
fn projected_source_span_replays_all_nine_rows_on_the_exact_residual_face() {
    let (context, adjacent, opposite) = derive_five_line_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    assert_eq!(zero_sectors.len(), 26);

    let expected_provenance = [
        "translated-source-v1:0:ordinary-ibp:0:0:[0,0,0,0,0,0]",
        "translated-source-v1:1:ordinary-ibp:0:1:[0,0,0,0,0,0]",
        "translated-source-v1:2:ordinary-ibp:0:2:[0,0,0,0,0,0]",
        "translated-source-v1:3:ordinary-ibp:1:0:[0,0,0,0,0,0]",
        "translated-source-v1:4:ordinary-ibp:1:1:[0,0,0,0,0,0]",
        "translated-source-v1:5:ordinary-ibp:1:2:[0,0,0,0,0,0]",
        "translated-source-v1:6:ordinary-ibp:2:0:[0,0,0,0,0,0]",
        "translated-source-v1:7:ordinary-ibp:2:1:[0,0,0,0,0,0]",
        "translated-source-v1:8:ordinary-ibp:2:2:[0,0,0,0,0,0]",
    ];
    let expected_projection_counts = [
        [1, 0, 7],
        [0, 0, 11],
        [0, 0, 11],
        [3, 0, 8],
        [0, 0, 8],
        [0, 0, 11],
        [3, 0, 8],
        [0, 0, 11],
        [0, 0, 8],
    ];

    for cell in [&adjacent, &opposite] {
        let sources = cell.sources();
        assert_eq!(sources.len(), 9);
        assert_eq!(
            sources
                .provenance()
                .iter()
                .map(|source| source.translated().stable_string())
                .collect::<Vec<_>>(),
            expected_provenance
        );
        assert!(
            sources
                .provenance()
                .iter()
                .all(|source| source.symmetry().is_none())
        );
        let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
            panic!("five-line sources must retain residual-projection evidence")
        };
        assert_eq!(
            evidence.domain().sector().active_bits(),
            [false, true, true, true, true, true]
        );
        assert_eq!(
            evidence.domain().bounds(),
            [
                InteriorBounds::new(0, 0),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
            ]
        );
        assert_eq!(
            evidence.fixed_restrictions(),
            [FixedIndexRestriction::new(0, 0)]
        );
        // Only the identity fixes every point of this symbolic face. The
        // larger setwise missing-edge stabilizer is tested separately below.
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
fn exact_rules_target_the_two_canonical_dotted_edge_representatives() {
    let (_context, adjacent, opposite) = derive_five_line_cells().unwrap();
    assert_eq!(adjacent.rule().anchor().powers(), ANCHOR);
    assert_eq!(opposite.rule().anchor().powers(), ANCHOR);
    assert_eq!(adjacent.rule().pivot().values(), ADJACENT_EDGE_TARGET_SHIFT);
    assert_eq!(opposite.rule().pivot().values(), OPPOSITE_EDGE_TARGET_SHIFT);
    assert_eq!(
        adjacent
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, 0, 0, 1, 0, -1][..],
            &[0, 0, 0, 1, -1, 0],
            &[0, 0, 0, 0, 1, -1],
            &[0, 0, 0, 0, 0, 0],
            &[0, 0, 0, -1, 1, 0],
        ]
    );
    assert_eq!(
        opposite
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, 1, 0, 0, 0, -1][..],
            &[0, 1, -1, 0, 0, 0],
            &[0, 0, 1, 0, 0, -1],
            &[0, 0, 0, 1, 0, -1],
            &[0, 0, 0, 1, -1, 0],
            &[0, 0, 0, 0, 1, -1],
            &[0, 0, 0, 0, 0, 0],
            &[0, 0, 0, -1, 1, 0],
            &[0, -1, 1, 0, 0, 0],
        ]
    );
    assert_eq!(
        adjacent
            .rule()
            .source_combination()
            .iter()
            .map(|source| (source.source_ordinal(), source.row_id().stable_string()))
            .collect::<Vec<_>>(),
        [
            (0, "ordinary-ibp:0:0".to_owned()),
            (3, "ordinary-ibp:1:0".to_owned()),
            (6, "ordinary-ibp:2:0".to_owned()),
        ]
    );
    assert_eq!(
        opposite
            .rule()
            .source_combination()
            .iter()
            .map(|source| (source.source_ordinal(), source.row_id().stable_string()))
            .collect::<Vec<_>>(),
        [
            (0, "ordinary-ibp:0:0".to_owned()),
            (4, "ordinary-ibp:1:1".to_owned()),
            (5, "ordinary-ibp:1:2".to_owned()),
            (7, "ordinary-ibp:2:1".to_owned()),
            (8, "ordinary-ibp:2:2".to_owned()),
        ]
    );
    assert_eq!(adjacent.rule().replay().source_rows_used(), 3);
    assert_eq!(opposite.rule().replay().source_rows_used(), 5);
    for cell in [&adjacent, &opposite] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(cell.terms().len(), cell.rule().right_hand_side().len());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let canonical_adjacent = IntegralKey::try_new([0, 1, 1, 1, 2, 1]).unwrap();
    for slot in 1..5 {
        let mut dotted = FIVE_LINE_SECTOR;
        dotted[slot] = 2;
        assert_eq!(
            canonicalizer
                .canonicalize(&IntegralKey::try_new(dotted).unwrap())
                .unwrap()
                .canonical(),
            &canonical_adjacent
        );
    }
    let canonical_opposite = IntegralKey::try_new([0, 1, 1, 1, 1, 2]).unwrap();
    assert_eq!(
        canonicalizer
            .canonicalize(&canonical_opposite)
            .unwrap()
            .canonical(),
        &canonical_opposite
    );
    let missing_edge_stabilizer = canonicalizer
        .group_elements()
        .filter(|mapping| mapping[0] == 0)
        .collect::<Vec<_>>();
    assert_eq!(missing_edge_stabilizer.len(), 4);
    assert_eq!(
        missing_edge_stabilizer
            .iter()
            .map(|mapping| mapping[3])
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 3, 4])
    );
    assert_eq!(
        missing_edge_stabilizer
            .iter()
            .map(|mapping| mapping[5])
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([5])
    );
}

#[test]
fn guards_and_application_boxes_are_exact_on_each_canonical_cell() {
    let (context, adjacent, opposite) = derive_five_line_cells().unwrap();
    assert_monomial_guards(&context, &adjacent, &[(-1, 3), (1, 3), (-3, 4), (3, 4)]);
    assert_monomial_guards(
        &context,
        &opposite,
        &[(-1, 3), (1, 4), (-2, 1), (-1, 2), (-6, 5), (3, 5), (6, 5)],
    );
    assert_eq!(
        adjacent.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX),
        ]
    );
    assert_eq!(
        opposite.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ]
    );
    for cell in [&adjacent, &opposite] {
        assert_eq!(
            cell.fixed_restrictions(),
            [FixedIndexRestriction::new(0, 0)]
        );
    }

    let adjacent_target = IntegralKey::try_new([0, 1, 1, 1, 2, 1]).unwrap();
    assert_eq!(
        adjacent.assignment_for_target(&adjacent_target).unwrap(),
        Some(vec![0, 1, 1, 1, 1, 1])
    );
    let opposite_target = IntegralKey::try_new([0, 1, 1, 1, 1, 2]).unwrap();
    assert_eq!(
        opposite.assignment_for_target(&opposite_target).unwrap(),
        Some(vec![0, 1, 1, 1, 1, 1])
    );
    let corner = IntegralKey::try_new(FIVE_LINE_SECTOR).unwrap();
    assert!(adjacent.assignment_for_target(&corner).unwrap().is_none());
    assert!(opposite.assignment_for_target(&corner).unwrap().is_none());
    assert!(
        adjacent
            .assignment_for_target(&IntegralKey::try_new([1, 1, 1, 1, 2, 1]).unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        opposite
            .assignment_for_target(&IntegralKey::try_new([0, i64::MAX, 1, 1, 1, 2]).unwrap())
            .unwrap()
            .is_none()
    );
}

fn assert_monomial_guards(
    context: &IndexedCoefficientContext,
    cell: &RuleCell,
    expected: &[(i64, usize)],
) {
    assert_eq!(cell.rule().nonzero_guards().len(), expected.len());
    assert_eq!(cell.guards().len(), expected.len());
    let base_variables = context.base().parameter_names().len();
    for (guard, &(expected_multiplier, expected_index)) in
        cell.rule().nonzero_guards().iter().zip(expected)
    {
        let polynomial = guard.polynomial().raw();
        assert_eq!(polynomial.nterms(), 1);
        assert_eq!(
            polynomial.coefficients,
            [Integer::from(expected_multiplier)]
        );
        let exponents = polynomial.exponents_iter().next().unwrap();
        assert!(exponents[..base_variables].iter().all(|&power| power == 0));
        assert!(
            exponents[base_variables..]
                .iter()
                .enumerate()
                .all(|(index, &power)| power == u16::from(index == expected_index))
        );
    }

    let assignment = vec![0, 1, 1, 1, 1, 1];
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
