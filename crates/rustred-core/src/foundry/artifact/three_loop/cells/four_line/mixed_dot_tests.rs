use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::ParametricRuleError;
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::FOUR_LINE_SECTOR;
use super::corner::fixed_base_corner;
use super::mixed_dot::{
    ADJACENT_MIXED_DOT_TARGET_SHIFT, MixedDotFourLineCells, OPPOSITE_MIXED_DOT_TARGET_SHIFT,
    derive_adjacent_mixed_dot_candidate, derive_mixed_dot_four_line_cells,
    derive_opposite_mixed_dot_candidate, mixed_dot_search_depth,
};

const ADJACENT_MIXED_DOT: [i64; 6] = [0, 1, 1, 2, 3, 0];
const OPPOSITE_MIXED_DOT: [i64; 6] = [0, 1, 2, 1, 3, 0];
const EXPECTED_RHS: [[i64; 6]; 2] = [[0, -1, 1, 1, 1, 0], [0, 0, 0, 0, 0, 0]];

#[test]
fn mixed_dot_singletons_own_independent_complete_generated_projections() {
    let MixedDotFourLineCells {
        context,
        adjacent,
        opposite,
    } = derive_mixed_dot_four_line_cells().unwrap();
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        mixed_dot_search_depth(),
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
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();

    for (label, cell, expected_contributions, expected_source_terms, expected_keys, expected_ops) in [
        ("adjacent", &adjacent, 17, 105, 108, 317),
        ("opposite", &opposite, 18, 113, 116, 342),
    ] {
        assert_eq!(cell.sources().len(), 28 * 9);
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
        let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction()
        else {
            panic!("{label} mixed-dot sources must retain projection evidence")
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

        assert_eq!(
            cell.rule().source_combination().len(),
            expected_contributions
        );
        for contribution in cell.rule().source_combination() {
            let provenance = &cell.sources().provenance()[contribution.source_ordinal()];
            assert_eq!(contribution.row_id(), provenance.translated().source_row());
        }
        let replay = cell.rule().concrete_replay();
        assert_eq!(
            replay.source_contributions_checked(),
            expected_contributions
        );
        assert_eq!(replay.source_terms_checked(), expected_source_terms);
        assert_eq!(replay.right_hand_side_terms_checked(), 2);
        assert_eq!(replay.integral_keys_checked(), expected_keys);
        assert_eq!(replay.nonzero_guards_checked(), 9);
        assert_eq!(replay.exact_operations(), expected_ops);
    }

    // Re-run both complete derivations rather than sharing either projected
    // batch or an authored relation table.
    assert_eq!(
        derive_adjacent_mixed_dot_candidate(2).unwrap(),
        *adjacent.rule()
    );
    assert_eq!(
        derive_opposite_mixed_dot_candidate(2).unwrap(),
        *opposite.rule()
    );
}

#[test]
fn mixed_dot_singletons_have_exact_descent_domains_and_s4_orbits() {
    let MixedDotFourLineCells {
        context: _,
        adjacent,
        opposite,
    } = derive_mixed_dot_four_line_cells().unwrap();
    assert_cell_shape(
        &adjacent,
        &ADJACENT_MIXED_DOT_TARGET_SHIFT,
        ADJACENT_MIXED_DOT,
        OPPOSITE_MIXED_DOT,
    );
    assert_cell_shape(
        &opposite,
        &OPPOSITE_MIXED_DOT_TARGET_SHIFT,
        OPPOSITE_MIXED_DOT,
        ADJACENT_MIXED_DOT,
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for power_three_slot in 1..5 {
        for power_two_slot in 1..5 {
            if power_three_slot == power_two_slot {
                continue;
            }
            let mut placement = FOUR_LINE_SECTOR;
            placement[power_three_slot] = 3;
            placement[power_two_slot] = 2;
            let canonical = canonicalizer
                .canonicalize(&IntegralKey::try_new(placement).unwrap())
                .unwrap();
            let pair = if power_three_slot < power_two_slot {
                (power_three_slot, power_two_slot)
            } else {
                (power_two_slot, power_three_slot)
            };
            let (expected_target, expected_cell) =
                if matches!(pair, (1, 2) | (1, 4) | (2, 3) | (3, 4)) {
                    (ADJACENT_MIXED_DOT, &adjacent)
                } else {
                    (OPPOSITE_MIXED_DOT, &opposite)
                };
            assert_eq!(canonical.canonical().powers(), expected_target);
            assert_eq!(
                expected_cell
                    .assignment_for_target(canonical.canonical())
                    .unwrap(),
                Some(FOUR_LINE_SECTOR.to_vec())
            );
        }
    }

    let second = derive_mixed_dot_four_line_cells().unwrap();
    assert_eq!(second.adjacent.rule(), adjacent.rule());
    assert_eq!(second.opposite.rule(), opposite.rule());
    assert_eq!(
        second.adjacent.application_domain(),
        adjacent.application_domain()
    );
    assert_eq!(
        second.opposite.application_domain(),
        opposite.application_domain()
    );
}

#[test]
fn mixed_dot_singletons_require_the_complete_depth_two_span() {
    for derive in [
        derive_adjacent_mixed_dot_candidate as fn(usize) -> Result<_, _>,
        derive_opposite_mixed_dot_candidate,
    ] {
        for depth in [0, 1] {
            assert_eq!(
                derive(depth),
                Err(ArtifactError::ParametricRule(
                    ParametricRuleError::TargetShiftAbsent
                ))
            );
        }
    }
    assert_eq!(mixed_dot_search_depth(), 2);
}

fn assert_cell_shape(
    cell: &RuleCell,
    expected_shift: &[i64; 6],
    target: [i64; 6],
    other_orbit: [i64; 6],
) {
    assert_eq!(cell.rule().anchor().powers(), FOUR_LINE_SECTOR);
    assert_eq!(cell.rule().pivot().values(), expected_shift);
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
    assert_eq!(cell.rule().nonzero_guards().len(), 9);
    assert_eq!(cell.guards().len(), 9);
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
    assert_eq!(
        cell.assignment_for_target(&IntegralKey::try_new(target).unwrap())
            .unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    assert!(
        cell.assignment_for_target(&IntegralKey::try_new(other_orbit).unwrap())
            .unwrap()
            .is_none()
    );

    let children = cell
        .rule()
        .right_hand_side()
        .iter()
        .map(|term| {
            IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
                FOUR_LINE_SECTOR[position] + term.shift().values()[position]
            }))
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(children[0].powers(), [0, 0, 2, 2, 2, 0]);
    assert_eq!(children[1].powers(), FOUR_LINE_SECTOR);
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    assert_eq!(
        canonicalizer
            .canonicalize(&children[0])
            .unwrap()
            .canonical()
            .powers(),
        [0, 0, 2, 0, 2, 2]
    );
}
