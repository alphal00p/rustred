use crate::family::IntegralKey;
use crate::foundry::cell::{
    ResidualTermDisposition, RuleCell, RuleCellDomainProof, SourceViewConstruction,
};
use crate::foundry::parametric::{
    ParametricRuleLimits, SectorMonotoneDependencyKind, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;
use symbolica::prelude::Integer;

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::FOUR_LINE_SECTOR;
use super::repeated_dot::{
    REPEATED_DOT_TARGET_SHIFT, derive_repeated_dot_ray_cell, fixed_ray_indices,
    repeated_dot_free_position, repeated_dot_search_depth,
};

#[test]
fn repeated_dot_ray_comes_from_the_complete_generated_depth_two_span() {
    let (context, cell) = derive_repeated_dot_ray_cell().unwrap();
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        repeated_dot_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(search.offset_count(), 28);
    assert_eq!(cell.sources().len(), 28 * 9);
    assert_eq!(cell.rule().source_combination().len(), 50);

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
        panic!("repeated-dot sources must retain residual-projection evidence")
    };
    assert_eq!(
        evidence.domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(0, 0),
        ]
    );
    assert_eq!(evidence.fixed_restrictions(), fixed_ray_indices());
    assert_eq!(evidence.original_relations().len(), 28 * 9);
    assert_eq!(evidence.term_projections().len(), 28 * 9);
    assert_eq!(evidence.stabilizer_group_elements(), [0, 22]);
    assert!(evidence.term_projections().iter().flatten().any(|term| {
        matches!(
            term.disposition(),
            ResidualTermDisposition::Routed {
                group_element: 22,
                ..
            }
        )
    }));
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    assert!(
        cell.sources()
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                Default::default(),
            )
            .unwrap()
    );
    assert_eq!(repeated_dot_free_position(), 4);
    assert_eq!(cell.rule().anchor().powers(), FOUR_LINE_SECTOR);
    assert_eq!(cell.rule().pivot().values(), REPEATED_DOT_TARGET_SHIFT);
    assert_eq!(cell.rule().right_hand_side().len(), 8);
    assert_eq!(cell.rule().nonzero_guards().len(), 32);
    assert_eq!(cell.guards().len(), 32);
    assert_guards_are_generically_nonzero_on_positive_ray(&context, &cell);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert!(cell.terms().iter().all(|term| term.descent().verify()));

    let replay = cell.rule().concrete_replay();
    assert_eq!(replay.source_contributions_checked(), 50);
    assert_eq!(replay.source_terms_checked(), 358);
    assert_eq!(replay.right_hand_side_terms_checked(), 8);
    assert_eq!(replay.integral_keys_checked(), 367);
    assert_eq!(replay.nonzero_guards_checked(), 32);
    assert_eq!(replay.exact_operations(), 1078);

    for (free_power, exact_operations) in [(2, 1080), (8, 1080)] {
        let mut assignment = FOUR_LINE_SECTOR;
        assignment[4] = free_power;
        let held_out = replay_rule_at_concrete_assignment(
            &context,
            cell.sources().relations(),
            cell.rule(),
            &assignment,
            ParametricRuleLimits::default(),
        )
        .unwrap();
        assert_eq!(held_out.source_contributions_checked(), 50);
        assert_eq!(held_out.source_terms_checked(), 358);
        assert_eq!(held_out.right_hand_side_terms_checked(), 8);
        assert_eq!(held_out.integral_keys_checked(), 367);
        assert_eq!(held_out.nonzero_guards_checked(), 32);
        assert_eq!(held_out.exact_operations(), exact_operations);
    }

    let (_second_context, second) = derive_repeated_dot_ray_cell().unwrap();
    assert_eq!(second.rule(), cell.rule());
    assert_eq!(second.application_domain(), cell.application_domain());
}

#[test]
fn repeated_dot_ray_has_the_exact_structural_domain_and_lower_sectors() {
    let (context, cell) = derive_repeated_dot_ray_cell().unwrap();
    assert_eq!(
        cell.rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, -1, 1, 1, 1, 0][..],
            &[0, 0, -1, 1, 1, 0],
            &[0, -1, 0, 1, 1, 0],
            &[0, 0, 0, 1, -1, 0],
            &[0, 0, 0, 0, 0, 0],
            &[0, 0, -1, 1, 0, 0],
            &[0, -1, 1, 0, 0, 0],
            &[0, -1, 0, 0, 1, 0],
        ]
    );
    assert_eq!(
        cell.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, i64::MAX - 2),
            InteriorBounds::new(0, 0),
        ]
    );
    for target_power in [3, 4, 9, i64::MAX] {
        let target = IntegralKey::try_new([0, 1, 1, 1, target_power, 0]).unwrap();
        let assignment = cell
            .assignment_for_target(&target)
            .unwrap()
            .expect("the selected repeated-dot ray must be owned");
        assert_eq!(assignment, [0, 1, 1, 1, target_power - 2, 0]);
        assert!(cell.guards().iter().all(|guard| {
            !context
                .specialize_polynomial(guard.polynomial(), &assignment, Default::default())
                .unwrap()
                .is_zero()
        }));
    }
    for outside in [[0, 1, 1, 1, 2, 0], [0, 1, 1, 2, 3, 0], [0, 1, 1, 1, 3, -1]] {
        assert!(
            cell.assignment_for_target(&IntegralKey::try_new(outside).unwrap())
                .unwrap()
                .is_none()
        );
    }

    let admission = cell
        .rule()
        .sector_monotone_admission()
        .expect("target-directed ray retains universal pinch dispositions");
    assert!(admission.verify());
    assert_eq!(admission.dependencies().len(), 8);
    for (free_power, expected_kinds, expected_pinches) in [
        (
            1,
            [
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::SameSector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
            ],
            [
                vec![1],
                vec![2],
                vec![1],
                vec![4],
                vec![],
                vec![2],
                vec![1],
                vec![1],
            ],
        ),
        (
            8,
            [
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::SameSector,
                SectorMonotoneDependencyKind::SameSector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
                SectorMonotoneDependencyKind::ProperSubsector,
            ],
            [
                vec![1],
                vec![2],
                vec![1],
                vec![],
                vec![],
                vec![2],
                vec![1],
                vec![1],
            ],
        ),
    ] {
        let mut assignment = FOUR_LINE_SECTOR;
        assignment[4] = free_power;
        let classified = admission.classify(&assignment).unwrap();
        assert_eq!(
            classified
                .iter()
                .map(|dependency| dependency.kind())
                .collect::<Vec<_>>(),
            expected_kinds
        );
        assert_eq!(
            classified
                .iter()
                .map(|dependency| dependency.pinched_positions().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected_pinches
        );
        assert!(classified.iter().all(|dependency| dependency.verify()));
    }

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for repeated_power in [3, 7] {
        let canonical = IntegralKey::try_new([0, 1, 1, 1, repeated_power, 0]).unwrap();
        for active_slot in 1..5 {
            let mut placement = FOUR_LINE_SECTOR;
            placement[active_slot] = repeated_power;
            assert_eq!(
                canonicalizer
                    .canonicalize(&IntegralKey::try_new(placement).unwrap())
                    .unwrap()
                    .canonical(),
                &canonical
            );
        }
    }
}

/// Every fixed-coordinate guard is a polynomial only in `d` and the free
/// ray index. Its leading-in-`d` coefficient has nonzero, uniform-sign
/// integer coefficients, so it cannot become the zero polynomial in `d` at
/// any positive free index. Concrete exceptional dimensions remain encoded
/// by the guards themselves.
fn assert_guards_are_generically_nonzero_on_positive_ray(
    context: &crate::algebra::IndexedCoefficientContext,
    cell: &RuleCell,
) {
    let base_variables = context.base().parameter_names().len();
    assert_eq!(base_variables, 1);
    let free_variable = base_variables + repeated_dot_free_position();
    for guard in cell.guards() {
        let polynomial = guard.polynomial().raw();
        let max_dimension_degree = polynomial
            .exponents_iter()
            .map(|exponents| exponents[0])
            .max()
            .unwrap();
        let mut leading_sign = None;
        for (coefficient, exponents) in polynomial
            .coefficients
            .iter()
            .zip(polynomial.exponents_iter())
        {
            assert!(
                exponents
                    .iter()
                    .enumerate()
                    .all(|(variable, &power)| variable == 0
                        || variable == free_variable
                        || power == 0)
            );
            if exponents[0] != max_dimension_degree {
                continue;
            }
            assert_ne!(coefficient, &Integer::from(0));
            let negative = coefficient.is_negative();
            assert!(leading_sign.is_none_or(|expected| expected == negative));
            leading_sign = Some(negative);
        }
        assert!(leading_sign.is_some());
    }
}
