use std::collections::BTreeMap;

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{ParametricRuleError, replay_rule_at_concrete_assignment};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;
use symbolica::prelude::Integer;

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::FOUR_LINE_SECTOR;
use super::corner::fixed_base_corner;
use super::mixed_dot_ray::{
    MIXED_DOT_RAY_TARGET_SHIFT, MixedDotRayBuild, derive_mixed_dot_ray_build,
    derive_mixed_dot_ray_cell, fixed_ray_indices, mixed_dot_ray_free_position,
    mixed_dot_ray_search_depth,
};

const EXPECTED_RHS: [[i64; 6]; 5] = [
    [0, 0, 0, 1, 3, 0],
    [0, 0, 0, 0, 3, 0],
    [0, 0, -1, 1, 3, 0],
    [0, -1, 0, 1, 3, 0],
    [0, 0, 0, 0, 2, 0],
];

#[test]
fn selected_source_mixed_dot_ray_retains_complete_exact_evidence() {
    let MixedDotRayBuild {
        context,
        cell,
        selection_witness,
        selected_complete_source_ordinals,
        full_span_diagnosis,
    } = derive_mixed_dot_ray_build(true).unwrap();

    // The exact corner elimination, not an authored ordinal table, selected
    // these rows from the complete 84 x 9 depth-three source span.
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        mixed_dot_ray_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(search.offset_count(), 84);
    let selection_witness = selection_witness.expect("exact tests retain the selection witness");
    assert_eq!(selection_witness.sources().len(), 84 * 9);
    assert_eq!(selected_complete_source_ordinals.len(), 46);
    assert!(
        selected_complete_source_ordinals
            .windows(2)
            .all(|window| window[0] < window[1])
    );
    assert!(
        selected_complete_source_ordinals
            .iter()
            .all(|&ordinal| ordinal < 84 * 9)
    );
    assert_eq!(cell.sources().len(), 46);
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
    for (offset, provenance) in search.offsets().iter().zip(
        selection_witness
            .sources()
            .provenance()
            .chunks(ordinary_rows.len()),
    ) {
        assert_eq!(provenance.len(), ordinary_rows.len());
        for (source, row) in provenance.iter().zip(ordinary_rows) {
            assert_eq!(source.translated().offset(), offset);
            assert_eq!(source.translated().source_row().stable_string(), row);
            assert!(source.symmetry().is_none());
        }
    }
    for (source, &ordinal) in cell
        .sources()
        .provenance()
        .iter()
        .zip(selected_complete_source_ordinals.iter())
    {
        assert_eq!(
            source.translated().offset(),
            &search.offsets()[ordinal / ordinary_rows.len()]
        );
        assert_eq!(
            source.translated().source_ordinal(),
            ordinal % ordinary_rows.len()
        );
        assert_eq!(
            source.translated().source_row().stable_string(),
            ordinary_rows[ordinal % ordinary_rows.len()]
        );
        assert!(source.symmetry().is_none());
    }

    let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction() else {
        panic!("the mixed-dot ray must retain selected residual-projection evidence")
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
    assert_eq!(evidence.stabilizer_group_elements(), [0, 22]);
    assert_eq!(evidence.original_relations().len(), 46);
    assert_eq!(evidence.term_projections().len(), 46);
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let SourceViewConstruction::ResidualProjection(selection_evidence) =
        selection_witness.sources().construction()
    else {
        panic!("the complete source-selection search must retain projection evidence")
    };
    assert_eq!(
        selection_evidence.domain().bounds(),
        FOUR_LINE_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(selection_evidence.fixed_restrictions(), fixed_base_corner());
    assert_eq!(
        selection_evidence.stabilizer_group_elements(),
        [0, 1, 2, 3, 20, 21, 22, 23]
    );
    assert_eq!(selection_evidence.original_relations().len(), 84 * 9);
    assert_eq!(selection_evidence.term_projections().len(), 84 * 9);
    assert!(
        selection_witness
            .sources()
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );
    assert_eq!(
        selection_witness
            .rule()
            .source_combination()
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        selected_complete_source_ordinals.as_ref()
    );
    let selection_replay = selection_witness.rule().concrete_replay();
    assert_eq!(selection_replay.source_contributions_checked(), 46);
    assert_eq!(selection_replay.source_terms_checked(), 310);
    assert_eq!(selection_replay.right_hand_side_terms_checked(), 3);
    assert_eq!(selection_replay.integral_keys_checked(), 314);
    assert_eq!(selection_replay.nonzero_guards_checked(), 22);
    assert_eq!(selection_replay.exact_operations(), 934);
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

    // Feeding all 756 ray-projected rows directly to the deterministic
    // eliminator chooses a candidate with an exceptional anchor guard.  The
    // generated 46-row selection is therefore a necessary search refinement,
    // not evidence that the target shift was absent from the complete span.
    assert_eq!(
        full_span_diagnosis,
        Some(Err(ArtifactError::ParametricRule(
            ParametricRuleError::GuardVanishedAtAnchor { guard_ordinal: 28 }
        )))
    );

    assert_eq!(cell.rule().anchor().powers(), FOUR_LINE_SECTOR);
    assert_eq!(cell.rule().pivot().values(), MIXED_DOT_RAY_TARGET_SHIFT);
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
    assert_eq!(cell.rule().source_combination().len(), 13);
    assert_eq!(cell.rule().replay().source_rows_used(), 13);
    assert_eq!(cell.rule().replay().exact_operations(), 245);
    for contribution in cell.rule().source_combination() {
        let provenance = &cell.sources().provenance()[contribution.source_ordinal()];
        assert_eq!(contribution.row_id(), provenance.translated().source_row());
    }
    let replay = cell.rule().concrete_replay();
    assert_eq!(replay.source_contributions_checked(), 13);
    assert_eq!(replay.source_terms_checked(), 90);
    assert_eq!(replay.right_hand_side_terms_checked(), 5);
    assert_eq!(replay.integral_keys_checked(), 96);
    assert_eq!(replay.nonzero_guards_checked(), 7);
    assert_eq!(replay.exact_operations(), 275);
    assert_eq!(cell.rule().nonzero_guards().len(), 7);
    assert_eq!(cell.guards().len(), 7);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        cell.application_domain().bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, i64::MAX - 3),
            InteriorBounds::new(0, 0),
        ]
    );
    assert_eq!(cell.fixed_restrictions(), fixed_ray_indices());
    assert!(cell.terms().iter().all(|term| term.descent().verify()));
    assert_guards_are_generically_nonzero_on_positive_ray(&context, &cell);

    // The anchor assignment (free power 1) and held-out assignments replay
    // the exact generated recurrence.  The only new same-sector frontier is
    // J(0,1,1,2,n+3,0); all other children lie on the existing repeated-dot
    // ray or exact factorization sectors.
    for free_power in [1, 2, 8] {
        let mut assignment = FOUR_LINE_SECTOR;
        assignment[mixed_dot_ray_free_position()] = free_power;
        let target = IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
            assignment[position] + MIXED_DOT_RAY_TARGET_SHIFT[position]
        }))
        .unwrap();
        assert_eq!(
            cell.assignment_for_target(&target).unwrap(),
            Some(assignment.to_vec())
        );
        assert!(cell.guards().iter().all(|guard| {
            !context
                .specialize_polynomial(guard.polynomial(), &assignment, Default::default())
                .unwrap()
                .is_zero()
        }));
        let checked_replay = replay_rule_at_concrete_assignment(
            &context,
            cell.sources().relations(),
            cell.rule(),
            &assignment,
            Default::default(),
        )
        .unwrap();
        assert_eq!(checked_replay.source_contributions_checked(), 13);
        assert_eq!(checked_replay.source_terms_checked(), 90);
        assert_eq!(checked_replay.right_hand_side_terms_checked(), 5);
        assert_eq!(checked_replay.integral_keys_checked(), 96);
        assert_eq!(checked_replay.nonzero_guards_checked(), 7);
        assert_eq!(checked_replay.exact_operations(), 275);
        assert_eq!(
            canonical_children(&canonicalizer, &cell, &assignment),
            [
                vec![0, 1, 1, 2, free_power + 3, 0],
                vec![0, 1, 1, 1, free_power + 3, 0],
                vec![0, 0, 1, 0, 2, free_power + 3],
                vec![0, 0, 1, 0, free_power + 3, 2],
                vec![0, 1, 1, 1, free_power + 2, 0],
            ]
        );
    }

    // S4 splits {1,2,2,N} into two inequivalent orbits.  This cell owns only
    // the 16-permutation canonical orientation used in its proof; the
    // complementary eight-permutation orientation remains explicit work.
    for target_power in [3, 4, 10] {
        let orbit = canonical_permutation_counts(&canonicalizer, [1, 2, 2, target_power]);
        let owned = vec![0, 1, 2, 2, target_power, 0];
        let complementary = vec![0, 1, 2, target_power, 2, 0];
        assert_eq!(
            orbit,
            BTreeMap::from([(owned.clone(), 16), (complementary.clone(), 8)])
        );
        assert!(
            cell.assignment_for_target(&IntegralKey::try_new(owned).unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            cell.assignment_for_target(&IntegralKey::try_new(complementary).unwrap())
                .unwrap()
                .is_none()
        );
    }
    for outside in [[0, 1, 2, 3, 3, 0], [0, 1, 3, 2, 3, 0]] {
        assert!(
            cell.assignment_for_target(&IntegralKey::try_new(outside).unwrap())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn mixed_dot_ray_owns_its_maximal_representable_target() {
    let (_context, cell) = derive_mixed_dot_ray_cell().unwrap();
    let maximal_owned = IntegralKey::try_new([0, 1, 2, 2, i64::MAX - 1, 0]).unwrap();
    assert_eq!(
        cell.assignment_for_target(&maximal_owned).unwrap(),
        Some(vec![0, 1, 1, 1, i64::MAX - 3, 0])
    );

    let unrepresentable_rhs = IntegralKey::try_new([0, 1, 2, 2, i64::MAX, 0]).unwrap();
    assert!(
        cell.assignment_for_target(&unrepresentable_rhs)
            .unwrap()
            .is_none()
    );
}

fn canonical_children(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    cell: &RuleCell,
    assignment: &[i64; 6],
) -> Vec<Vec<i64>> {
    cell.rule()
        .right_hand_side()
        .iter()
        .map(|term| {
            let raw = IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
                assignment[position] + term.shift().values()[position]
            }))
            .unwrap();
            canonicalizer
                .canonicalize(&raw)
                .unwrap()
                .canonical()
                .powers()
                .to_vec()
        })
        .collect()
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

/// Every specialized guard is a polynomial only in `d` and the positive free
/// index.  Its leading-in-`d` coefficient has nonzero, uniform-sign integer
/// coefficients, hence cannot vanish identically for any positive index.
fn assert_guards_are_generically_nonzero_on_positive_ray(
    context: &crate::algebra::IndexedCoefficientContext,
    cell: &RuleCell,
) {
    let base_variables = context.base().parameter_names().len();
    assert_eq!(base_variables, 1);
    let free_variable = base_variables + mixed_dot_ray_free_position();
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
