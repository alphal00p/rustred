use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::foundry::cell::{
    RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::parametric::{ParametricRule, replay_rule_at_concrete_assignment};
use crate::foundry::search::{
    ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::three_line::derive_three_line_cells;
use super::FOUR_LINE_SECTOR;
use super::dotted_negative_numerator_bulk::{
    BULK_REPLAY_ANCHOR, DOTTED_NEGATIVE_NUMERATOR_PIVOT, DottedNegativeNumeratorBulkBuild,
    FREE_POSITION, derive_dotted_negative_numerator_bulk,
    derive_dotted_negative_numerator_bulk_build, dotted_negative_numerator_search_depth,
    fixed_scalar_source_face,
};
use super::inactive_numerator::derive_inactive_numerator_cells;

const ORDINARY_ROWS: [&str; 9] = [
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
const COMPLETE_SELECTION: [usize; 3] = [0, 3, 4];
const RHS: [[i64; 6]; 3] = [[0, 0, 0, 0, 0, 0], [0, -1, 1, 0, 0, 0], [0, 0, 0, 0, 0, 1]];

#[test]
fn complete_depth_zero_selection_and_independent_full_projection_are_exact() {
    let DottedNegativeNumeratorBulkBuild {
        context,
        bulk,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_dotted_negative_numerator_bulk_build(true).unwrap();
    let selection = selection_witness.expect("exact test retains the complete source span");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        dotted_negative_numerator_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(dotted_negative_numerator_search_depth(), 0);
    assert_eq!(search.offset_count(), 1);
    assert_eq!(search.offsets()[0].values(), [0; 6]);

    assert_eq!(selection.complete_sources.len(), ORDINARY_ROWS.len());
    assert_complete_provenance(&selection.complete_sources);
    assert_eq!(
        selected_ordinals(&selection.complete_rule),
        COMPLETE_SELECTION
    );
    assert_eq!(
        selected_complete_source_ordinals.as_ref(),
        COMPLETE_SELECTION
    );
    assert_selected_provenance(bulk.sources(), &COMPLETE_SELECTION);
    assert_eq!(selected_ordinals(bulk.rule()), [0, 1, 2]);

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for (sources, lower, original_rows) in [
        (
            &selection.complete_sources,
            i64::MIN + 2,
            ORDINARY_ROWS.len(),
        ),
        (bulk.sources(), i64::MIN + 1, COMPLETE_SELECTION.len()),
    ] {
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
        let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
            panic!("generated bulk sources must retain residual projection evidence")
        };
        assert_eq!(evidence.domain().bounds(), source_bounds(lower));
        assert_eq!(evidence.fixed_restrictions(), fixed_scalar_source_face());
        assert_eq!(evidence.stabilizer_group_elements(), [0, 1, 2, 3]);
        assert_eq!(evidence.original_relations().len(), original_rows);
        assert_eq!(evidence.term_projections().len(), original_rows);
    }
}

#[test]
fn exact_rule_guards_replay_machine_bounds_descent_and_rebuild_are_pinned() {
    let build = derive_dotted_negative_numerator_bulk_build(true).unwrap();
    let selection = build
        .selection_witness
        .as_ref()
        .expect("exact test retains the complete source span");
    for rule in [&selection.complete_rule, build.bulk.rule()] {
        assert_rule(&build.context, rule);
    }
    assert_eq!(
        selection.complete_rule.pivot_guard().coefficient(),
        build.bulk.rule().pivot_guard().coefficient()
    );
    assert_eq!(
        selection.complete_rule.pivot_guard().nonzero_polynomial(),
        build.bulk.rule().pivot_guard().nonzero_polynomial()
    );
    assert_eq!(
        selection
            .complete_rule
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        build
            .bulk
            .rule()
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        selection
            .complete_rule
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        build
            .bulk
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>()
    );
    assert!(build.bulk.guards().is_empty());
    assert!(build.bulk.pruned_rhs_ordinals().is_empty());
    assert_eq!(
        build.bulk.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.bulk.application_domain().bounds(),
        source_bounds(i64::MIN + 1)
    );
    assert_eq!(build.bulk.fixed_restrictions(), fixed_scalar_source_face());
    assert!(
        build
            .bulk
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );

    for free in [-1, -7, i64::MIN + 1] {
        let replay = replay_rule_at_concrete_assignment(
            &build.context,
            build.bulk.sources().relations(),
            build.bulk.rule(),
            &[0, 1, 1, 1, 1, free],
            Default::default(),
        )
        .unwrap();
        assert_eq!(concrete_metrics(&replay), (3, 15, 3, 19, 0, 51, 13));
    }

    let target = |numerator| IntegralKey::try_new([0, 1, 1, 1, 2, numerator]).unwrap();
    assert_eq!(
        build.bulk.assignment_for_target(&target(i64::MIN)).unwrap(),
        Some(vec![0, 1, 1, 1, 1, i64::MIN + 1])
    );
    assert_eq!(
        build.bulk.assignment_for_target(&target(-2)).unwrap(),
        Some(vec![0, 1, 1, 1, 1, -1])
    );
    assert!(
        build
            .bulk
            .assignment_for_target(&target(-1))
            .unwrap()
            .is_none()
    );
    assert!(
        build
            .bulk
            .assignment_for_target(&target(0))
            .unwrap()
            .is_none()
    );

    let (_second_context, second) = derive_dotted_negative_numerator_bulk().unwrap();
    assert_eq!(second.rule(), build.bulk.rule());
    assert_eq!(
        second.sources().relations(),
        build.bulk.sources().relations()
    );
    assert_eq!(second.application_domain(), build.bulk.application_domain());
    assert_eq!(second.fixed_restrictions(), build.bulk.fixed_restrictions());
}

#[test]
fn exact_s4_boundary_is_one_dotted_inactive_numerator_orbit() {
    let (_context, bulk) = derive_dotted_negative_numerator_bulk().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();

    for free in [-2, -7, i64::MIN] {
        let target = IntegralKey::try_new([0, 1, 1, 1, 2, free]).unwrap();
        let orbit = canonicalizer.orbit(&target).unwrap();
        assert_eq!(orbit.group_order(), 24);
        assert_eq!(orbit.orbit_size(), 24);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == 1)
        );
        assert_eq!(orbit.canonical().integral(), &target);

        let mut placement_representatives = BTreeSet::new();
        for inactive in [0, 5] {
            for active in 1..5 {
                let mut powers = FOUR_LINE_SECTOR;
                powers[inactive] = free;
                powers[active] = 2;
                placement_representatives.insert(
                    canonicalizer
                        .canonicalize(&IntegralKey::try_new(powers).unwrap())
                        .unwrap()
                        .canonical()
                        .powers()
                        .to_vec(),
                );
            }
        }
        assert_eq!(
            placement_representatives,
            BTreeSet::from([target.powers().to_vec()])
        );
        assert!(bulk.assignment_for_target(&target).unwrap().is_some());

        for outside in [
            [0, 1, 1, 1, 1, free],
            [0, 1, 1, 1, 3, free],
            [0, 1, 1, 2, 2, free],
            [free, 1, 1, 1, 2, -1],
        ] {
            let outside = canonicalizer
                .canonicalize(&IntegralKey::try_new(outside).unwrap())
                .unwrap();
            assert!(
                bulk.assignment_for_target(outside.canonical())
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert!(
        bulk.assignment_for_target(&IntegralKey::try_new([0, 1, 1, 1, 2, -1]).unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn every_child_routes_to_installed_cells_or_the_existing_scalar_corner() {
    let (_context, bulk) = derive_dotted_negative_numerator_bulk().unwrap();
    let (_context, scalar_endpoint, scalar_bulk) = derive_inactive_numerator_cells().unwrap();
    let three_line = derive_three_line_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();

    for free in [-1, -7, i64::MIN + 1] {
        let children = canonical_children(&canonicalizer, &bulk, &[0, 1, 1, 1, 1, free]);
        assert_eq!(
            children,
            [
                vec![0, 1, 1, 1, 1, free],
                vec![0, 0, 2, free, 1, 1],
                vec![0, 1, 1, 1, 1, free + 1],
            ]
        );

        if free == -1 {
            assert!(
                scalar_endpoint
                    .assignment_for_target(&key(&children[0]))
                    .unwrap()
                    .is_some()
            );
            assert!(
                three_line
                    .decorated_path_numerator_endpoint
                    .assignment_for_target(&key(&children[1]))
                    .unwrap()
                    .is_some()
            );
            assert!(
                scalar_endpoint
                    .assignment_for_target(&key(&children[2]))
                    .unwrap()
                    .is_none()
            );
            assert!(
                scalar_bulk
                    .assignment_for_target(&key(&children[2]))
                    .unwrap()
                    .is_none()
            );
            assert!(terminals.classify(&key(&children[2])).is_none());
        } else {
            assert!(
                scalar_bulk
                    .assignment_for_target(&key(&children[0]))
                    .unwrap()
                    .is_some()
            );
            assert!(
                three_line
                    .decorated_path_numerator_bulk
                    .assignment_for_target(&key(&children[1]))
                    .unwrap()
                    .is_some()
            );
            assert!(
                scalar_bulk
                    .assignment_for_target(&key(&children[2]))
                    .unwrap()
                    .is_some()
            );
        }
    }
}

fn assert_complete_provenance(sources: &SourceViewBatch) {
    for (source, row) in sources.provenance().iter().zip(ORDINARY_ROWS) {
        assert_eq!(source.translated().offset().values(), [0; 6]);
        assert_eq!(source.translated().source_row().stable_string(), row);
        assert!(source.symmetry().is_none());
    }
}

fn assert_selected_provenance(sources: &SourceViewBatch, complete_ordinals: &[usize]) {
    assert_eq!(sources.len(), complete_ordinals.len());
    for (source, &ordinal) in sources.provenance().iter().zip(complete_ordinals) {
        assert_eq!(source.translated().offset().values(), [0; 6]);
        assert_eq!(
            source.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal]
        );
        assert!(source.symmetry().is_none());
    }
}

fn assert_rule(context: &crate::algebra::IndexedCoefficientContext, rule: &ParametricRule) {
    assert_eq!(rule.anchor().powers(), BULK_REPLAY_ANCHOR);
    assert_eq!(rule.pivot().values(), DOTTED_NEGATIVE_NUMERATOR_PIVOT);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
    );
    assert!(rule.nonzero_guards().is_empty());
    let five_eighths = context
        .lift(&context.base().coefficient_fixture("5/8"))
        .unwrap();
    let minus_one = context.integer(-1);
    let minus_three_quarters = context
        .lift(&context.base().coefficient_fixture("-3/4"))
        .unwrap();
    assert_eq!(rule.pivot_guard().coefficient(), &context.integer(-4));
    assert_eq!(
        rule.source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        [&five_eighths, &minus_one, &minus_three_quarters]
    );

    let dimension = context
        .lift(&context.base().coefficient_fixture("d"))
        .unwrap();
    let six_n5 = context
        .mul(&context.integer(6), &context.index(5).unwrap())
        .unwrap();
    let dimension_minus_six_n5 = context.sub(&dimension, &six_n5).unwrap();
    let first_rhs = context
        .div(&dimension_minus_six_n5, &context.integer(8))
        .unwrap();
    let third_rhs = context
        .mul(&minus_three_quarters, &context.index(5).unwrap())
        .unwrap();
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        [&first_rhs, &context.one(), &third_rhs]
    );
    assert_eq!(rule.replay().source_rows_used(), 3);
    assert_eq!(rule.replay().shift_columns_checked(), 7);
    assert_eq!(rule.replay().exact_operations(), 30);
    assert_eq!(
        concrete_metrics(rule.concrete_replay()),
        (3, 15, 3, 19, 0, 51, 13)
    );
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn concrete_metrics(
    replay: &crate::foundry::parametric::ConcreteSpecializationReplayWitness,
) -> (usize, usize, usize, usize, usize, usize, usize) {
    (
        replay.source_contributions_checked(),
        replay.source_terms_checked(),
        replay.right_hand_side_terms_checked(),
        replay.integral_keys_checked(),
        replay.nonzero_guards_checked(),
        replay.exact_operations(),
        replay.peak_retained_coefficient_terms(),
    )
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
                assignment[position]
                    .checked_add(term.shift().values()[position])
                    .unwrap()
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

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn source_bounds(lower: i64) -> [InteriorBounds; 6] {
    std::array::from_fn(|position| {
        if position == FREE_POSITION {
            InteriorBounds::new(lower, -1)
        } else {
            InteriorBounds::new(FOUR_LINE_SECTOR[position], FOUR_LINE_SECTOR[position])
        }
    })
}
