use std::collections::BTreeMap;

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{
    ParametricGuardOrigin, ParametricRule, ParametricRuleError, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{
    ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::super::three_line::derive_three_line_cells;
use super::super::FOUR_LINE_SECTOR;
use super::opposite_inactive_numerator_pair::{
    OPPOSITE_PAIR_DOT_PIVOT, OPPOSITE_PAIR_PIVOT, OPPOSITE_PAIR_REPLAY_ANCHOR,
    OPPOSITE_PAIR_SOURCE, OppositePairEndpointBuild,
    derive_opposite_inactive_numerator_pair_endpoints, derive_opposite_pair_candidate,
    derive_opposite_pair_endpoint_build, dotted_search_depth, fixed_source, undotted_search_depth,
};
use super::scalar::derive_inactive_numerator_cells;

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
const DEPTH_ONE_OFFSETS: [[i64; 6]; 7] = [
    [-1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, -1],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0],
    [0, 0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0, 0],
    [0, 1, 0, 0, 0, 0],
];
const UNDOTTED_SELECTION: [usize; 4] = [0, 4, 18, 22];
const DOTTED_SELECTION: [usize; 2] = [0, 4];
const UNDOTTED_TARGET: [i64; 6] = [-1, 1, 1, 1, 1, -1];
const DOTTED_TARGET: [i64; 6] = [-1, 1, 1, 1, 2, -1];
const UNDOTTED_REDUCER_PIVOT_SHIFT: [i64; 6] = [0, 0, 0, 0, 0, -1];
const UNDOTTED_RHS: [[i64; 6]; 4] = [
    [-1, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0],
    [0, -1, 0, 0, 1, 0],
    [0, 0, 0, 0, 0, 1],
];
const DOTTED_RHS: [[i64; 6]; 3] = [[0, 0, 0, 0, 0, 0], [0, -1, 0, 0, 1, 0], [0, 0, 0, 0, 0, 1]];

#[test]
fn complete_search_minimality_selection_and_independent_reprojection_are_exact() {
    assert_eq!(
        derive_opposite_pair_candidate(0, &OPPOSITE_PAIR_PIVOT),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    let OppositePairEndpointBuild {
        context,
        undotted_endpoint,
        dotted_endpoint,
        undotted_selected_complete_source_ordinals,
        dotted_selected_complete_source_ordinals,
        selection_witness,
    } = derive_opposite_pair_endpoint_build(true).unwrap();
    let witness = selection_witness.expect("exact tests retain complete spans");

    let undotted_search = search(undotted_search_depth());
    assert_eq!(undotted_search_depth(), 1);
    assert_eq!(undotted_search.offset_count(), DEPTH_ONE_OFFSETS.len());
    assert_eq!(
        undotted_search
            .offsets()
            .iter()
            .map(|offset| offset.values())
            .collect::<Vec<_>>(),
        DEPTH_ONE_OFFSETS
            .iter()
            .map(<[i64; 6]>::as_slice)
            .collect::<Vec<_>>()
    );
    assert_eq!(witness.complete_undotted_sources.len(), 63);
    assert_complete_provenance(&witness.complete_undotted_sources, &undotted_search);
    assert_eq!(
        selected_ordinals(&witness.complete_undotted_rule),
        UNDOTTED_SELECTION
    );
    assert_eq!(
        undotted_selected_complete_source_ordinals.as_ref(),
        UNDOTTED_SELECTION
    );
    assert_selected_provenance(
        undotted_endpoint.sources(),
        &undotted_search,
        &UNDOTTED_SELECTION,
    );
    assert_eq!(selected_ordinals(undotted_endpoint.rule()), [0, 1, 2, 3]);

    let dotted_search = search(dotted_search_depth());
    assert_eq!(dotted_search_depth(), 0);
    assert_eq!(dotted_search.offset_count(), 1);
    assert_eq!(dotted_search.offsets()[0].values(), [0; 6]);
    assert_eq!(witness.complete_dotted_sources.len(), 9);
    assert_complete_provenance(&witness.complete_dotted_sources, &dotted_search);
    assert_eq!(
        selected_ordinals(&witness.complete_dotted_rule),
        DOTTED_SELECTION
    );
    assert_eq!(
        dotted_selected_complete_source_ordinals.as_ref(),
        DOTTED_SELECTION
    );
    assert_selected_provenance(dotted_endpoint.sources(), &dotted_search, &DOTTED_SELECTION);
    assert_eq!(selected_ordinals(dotted_endpoint.rule()), [0, 1]);

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for (sources, original_rows) in [
        (&witness.complete_undotted_sources, 63),
        (undotted_endpoint.sources(), 4),
        (&witness.complete_dotted_sources, 9),
        (dotted_endpoint.sources(), 2),
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
            panic!("generated endpoints must retain residual projection routes")
        };
        assert_eq!(
            evidence.domain().bounds(),
            OPPOSITE_PAIR_SOURCE.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_source());
        assert_eq!(evidence.original_relations().len(), original_rows);
        assert_eq!(evidence.term_projections().len(), original_rows);
    }
}

#[test]
fn exact_coefficients_guards_replay_domains_and_rebuild_are_pinned() {
    let build = derive_opposite_pair_endpoint_build(true).unwrap();
    let witness = build
        .selection_witness
        .as_ref()
        .expect("exact tests retain complete spans");
    assert_rule(
        &witness.complete_undotted_rule,
        &OPPOSITE_PAIR_PIVOT,
        &UNDOTTED_RHS,
        (4, 38, 67),
        (4, 19, 4, 24, 4, 69, 18),
    );
    assert_rule(
        build.undotted_endpoint.rule(),
        &OPPOSITE_PAIR_PIVOT,
        &UNDOTTED_RHS,
        (4, 9, 38),
        (4, 19, 4, 24, 1, 66, 18),
    );
    assert_rule(
        &witness.complete_dotted_rule,
        &OPPOSITE_PAIR_DOT_PIVOT,
        &DOTTED_RHS,
        (2, 7, 20),
        (2, 9, 3, 13, 0, 34, 11),
    );
    assert_rule(
        build.dotted_endpoint.rule(),
        &OPPOSITE_PAIR_DOT_PIVOT,
        &DOTTED_RHS,
        (2, 5, 18),
        (2, 9, 3, 13, 0, 34, 11),
    );

    assert_complete_compact_equivalence(
        &witness.complete_undotted_rule,
        build.undotted_endpoint.rule(),
    );
    assert_complete_compact_equivalence(
        &witness.complete_dotted_rule,
        build.dotted_endpoint.rule(),
    );

    let indexed = |expression| {
        build
            .context
            .lift(&build.context.base().coefficient_fixture(expression))
            .unwrap()
    };
    let reciprocal = indexed("1/(3*d-4)");
    let twice_reciprocal = indexed("2/(3*d-4)");
    let minus_three_reciprocal = indexed("-3/(3*d-4)");
    let pivot = indexed("(3*d-4)/2");
    let complete_pivot = indexed("(4-3*d)/6");
    let minus_twice_reciprocal = indexed("-2/(3*d-4)");
    let scalar = indexed("(d-4)/(3*d-4)");
    let eight_reciprocal = indexed("8/(3*d-4)");
    assert_eq!(
        coefficients(build.undotted_endpoint.rule()),
        [
            &reciprocal,
            &twice_reciprocal,
            &minus_three_reciprocal,
            &twice_reciprocal,
        ]
    );
    assert_eq!(
        build.undotted_endpoint.rule().pivot_guard().coefficient(),
        &pivot
    );
    assert_eq!(
        rhs_coefficients(build.undotted_endpoint.rule()),
        [
            &minus_twice_reciprocal,
            &scalar,
            &eight_reciprocal,
            &minus_twice_reciprocal,
        ]
    );
    let guard = build
        .context
        .denominator_condition_with_limits(&reciprocal, Default::default())
        .unwrap();
    let complete_guard = build
        .context
        .numerator_condition_with_limits(&complete_pivot, Default::default())
        .unwrap();
    for (
        rule,
        reducer_source,
        pivot_column,
        reducer_pivot_shift,
        pivot_coefficient,
        pivot_polynomial,
    ) in [
        (
            &witness.complete_undotted_rule,
            22,
            26,
            &UNDOTTED_REDUCER_PIVOT_SHIFT,
            &complete_pivot,
            &complete_guard,
        ),
        (
            build.undotted_endpoint.rule(),
            3,
            3,
            &OPPOSITE_PAIR_PIVOT,
            &pivot,
            &guard,
        ),
    ] {
        assert_eq!(rule.pivot_guard().source_ordinal(), reducer_source);
        assert_eq!(
            rule.pivot_guard().row_id().stable_string(),
            "ordinary-ibp:1:1"
        );
        assert_eq!(rule.pivot_guard().pivot_column(), pivot_column);
        assert_eq!(
            rule.pivot_guard().pivot_shift().values(),
            reducer_pivot_shift
        );
        assert_eq!(rule.pivot_guard().coefficient(), pivot_coefficient);
        assert_eq!(rule.pivot_guard().nonzero_polynomial(), pivot_polynomial);
        let reducer_guard = rule
            .nonzero_guards()
            .iter()
            .find(|candidate| candidate.polynomial() == pivot_polynomial)
            .expect("pivot guard must survive guard deduplication");
        assert!(reducer_guard.origins().iter().any(|origin| matches!(
            origin,
            ParametricGuardOrigin::ReducerPivotNumerator {
                source_ordinal,
                row_id,
                pivot_column: origin_pivot_column,
                pivot_shift,
            } if *source_ordinal == reducer_source
                && row_id.stable_string() == "ordinary-ibp:1:1"
                && *origin_pivot_column == pivot_column
                        && pivot_shift.values() == reducer_pivot_shift
        )));
    }
    assert_eq!(witness.complete_undotted_rule.nonzero_guards().len(), 4);
    assert_eq!(build.undotted_endpoint.rule().nonzero_guards().len(), 1);
    assert_eq!(
        build.undotted_endpoint.rule().nonzero_guards()[0].polynomial(),
        &guard
    );
    assert_eq!(build.undotted_endpoint.guards().len(), 1);
    assert_eq!(build.undotted_endpoint.guards()[0].polynomial(), &guard);

    let minus_three_eighths = indexed("-3/8");
    let one_quarter = indexed("1/4");
    let minus_four = build.context.integer(-4);
    let dotted_scalar = indexed("(d-2)/8");
    let minus_one_quarter = indexed("-1/4");
    assert_eq!(
        coefficients(build.dotted_endpoint.rule()),
        [&minus_three_eighths, &one_quarter]
    );
    assert_eq!(
        build.dotted_endpoint.rule().pivot_guard().coefficient(),
        &minus_four
    );
    assert_eq!(
        witness.complete_dotted_rule.pivot_guard().coefficient(),
        &minus_four
    );
    assert_eq!(
        rhs_coefficients(build.dotted_endpoint.rule()),
        [&dotted_scalar, &build.context.one(), &minus_one_quarter]
    );
    assert!(build.dotted_endpoint.rule().nonzero_guards().is_empty());
    assert!(witness.complete_dotted_rule.nonzero_guards().is_empty());
    assert!(build.dotted_endpoint.guards().is_empty());

    for cell in [&build.undotted_endpoint, &build.dotted_endpoint] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(
            cell.application_domain().bounds(),
            OPPOSITE_PAIR_SOURCE.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(cell.fixed_restrictions(), fixed_source());
        assert!(cell.pruned_rhs_ordinals().is_empty());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
        let replay = replay_rule_at_concrete_assignment(
            &build.context,
            cell.sources().relations(),
            cell.rule(),
            &OPPOSITE_PAIR_SOURCE,
            Default::default(),
        )
        .unwrap();
        assert_eq!(replay.anchor().powers(), OPPOSITE_PAIR_SOURCE);
        assert_eq!(
            concrete_metrics(&replay),
            concrete_metrics(cell.rule().concrete_replay())
        );
    }

    assert_eq!(
        build
            .undotted_endpoint
            .assignment_for_target(&key(&UNDOTTED_TARGET))
            .unwrap(),
        Some(OPPOSITE_PAIR_SOURCE.to_vec())
    );
    assert_eq!(
        build
            .dotted_endpoint
            .assignment_for_target(&key(&DOTTED_TARGET))
            .unwrap(),
        Some(OPPOSITE_PAIR_SOURCE.to_vec())
    );
    for unowned in [
        [-1, -1, 1, 1, 1, 1],
        [-2, 1, 1, 1, 1, -1],
        [i64::MIN, 1, 1, 1, 1, -1],
        [-1, 1, 1, 1, 1, i64::MIN],
        [-1, 1, 1, 1, 2, -1],
    ] {
        assert!(
            build
                .undotted_endpoint
                .assignment_for_target(&key(&unowned))
                .unwrap()
                .is_none()
        );
    }
    for unowned in [
        [-1, -1, 1, 1, 1, 2],
        [-1, -1, 1, 1, 2, 1],
        [-1, -1, 2, 1, 1, 1],
        [-1, 1, 1, 1, 3, -1],
        [-2, 1, 1, 1, 2, -1],
        [i64::MIN, 1, 1, 1, 2, -1],
        [-1, 1, 1, 1, 2, i64::MIN],
        UNDOTTED_TARGET,
    ] {
        assert!(
            build
                .dotted_endpoint
                .assignment_for_target(&key(&unowned))
                .unwrap()
                .is_none()
        );
    }

    let (_second_context, second_undotted, second_dotted) =
        derive_opposite_inactive_numerator_pair_endpoints().unwrap();
    for (second, first) in [
        (&second_undotted, &build.undotted_endpoint),
        (&second_dotted, &build.dotted_endpoint),
    ] {
        assert_eq!(second.rule(), first.rule());
        assert_eq!(second.application_domain(), first.application_domain());
        assert_eq!(second.fixed_restrictions(), first.fixed_restrictions());
        assert_eq!(second.guards(), first.guards());
    }
}

#[test]
fn exhaustive_s4_placement_boundaries_are_exact() {
    let (_context, undotted, dotted) = derive_opposite_inactive_numerator_pair_endpoints().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();

    let mut pair_classes = BTreeMap::<Vec<i64>, usize>::new();
    for first in 0..6 {
        for second in (first + 1)..6 {
            let mut powers = [1; 6];
            powers[first] = -1;
            powers[second] = -1;
            *pair_classes
                .entry(canonical(&canonicalizer, powers))
                .or_default() += 1;
        }
    }
    assert_eq!(
        pair_classes,
        [
            (vec![-1, -1, 1, 1, 1, 1], 12),
            (UNDOTTED_TARGET.to_vec(), 3),
        ]
        .into_iter()
        .collect()
    );
    for (representative, orbit_size, multiplicity) in
        [([-1, -1, 1, 1, 1, 1], 12, 2), (UNDOTTED_TARGET, 3, 8)]
    {
        let orbit = canonicalizer.orbit(&key(&representative)).unwrap();
        assert_eq!(orbit.orbit_size(), orbit_size);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == multiplicity)
        );
        assert_eq!(orbit.canonical().integral().powers(), representative);
    }
    assert!(
        undotted
            .assignment_for_target(&key(&[-1, -1, 1, 1, 1, 1]))
            .unwrap()
            .is_none()
    );

    let mut dotted_classes = BTreeMap::<Vec<i64>, usize>::new();
    for first in 0..6 {
        for second in (first + 1)..6 {
            for dot in 0..6 {
                if dot == first || dot == second {
                    continue;
                }
                let mut powers = [1; 6];
                powers[first] = -1;
                powers[second] = -1;
                powers[dot] = 2;
                *dotted_classes
                    .entry(canonical(&canonicalizer, powers))
                    .or_default() += 1;
            }
        }
    }
    assert_eq!(
        dotted_classes,
        [
            (vec![-1, -1, 1, 1, 1, 2], 24),
            (vec![-1, -1, 1, 1, 2, 1], 12),
            (vec![-1, -1, 2, 1, 1, 1], 12),
            (DOTTED_TARGET.to_vec(), 12),
        ]
        .into_iter()
        .collect()
    );
    for (representative, orbit_size, multiplicity) in [
        ([-1, -1, 1, 1, 1, 2], 24, 1),
        ([-1, -1, 1, 1, 2, 1], 12, 2),
        ([-1, -1, 2, 1, 1, 1], 12, 2),
        (DOTTED_TARGET, 12, 2),
    ] {
        let orbit = canonicalizer.orbit(&key(&representative)).unwrap();
        assert_eq!(orbit.orbit_size(), orbit_size);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == multiplicity)
        );
        assert_eq!(orbit.canonical().integral().powers(), representative);
        assert_eq!(
            dotted
                .assignment_for_target(&key(&representative))
                .unwrap()
                .is_some(),
            representative == DOTTED_TARGET
        );
    }
}

#[test]
fn canonical_children_route_to_existing_owners_or_the_preexisting_corner() {
    let (_context, undotted, dotted) = derive_opposite_inactive_numerator_pair_endpoints().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let undotted_children = canonical_children(&canonicalizer, &undotted, &OPPOSITE_PAIR_SOURCE);
    assert_eq!(
        undotted_children,
        [
            OPPOSITE_PAIR_SOURCE.to_vec(),
            OPPOSITE_PAIR_SOURCE.to_vec(),
            vec![0, 0, 1, -1, 2, 1],
            FOUR_LINE_SECTOR.to_vec(),
        ]
    );
    let dotted_children = canonical_children(&canonicalizer, &dotted, &OPPOSITE_PAIR_SOURCE);
    assert_eq!(
        dotted_children,
        [
            OPPOSITE_PAIR_SOURCE.to_vec(),
            vec![0, 0, 1, -1, 2, 1],
            FOUR_LINE_SECTOR.to_vec(),
        ]
    );

    let (_context, scalar_endpoint, _scalar_bulk) = derive_inactive_numerator_cells().unwrap();
    assert_eq!(
        scalar_endpoint
            .assignment_for_target(&key(&OPPOSITE_PAIR_SOURCE))
            .unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    let three_line = derive_three_line_cells().unwrap();
    assert_eq!(
        three_line
            .incident_path_dot_numerator_endpoint
            .assignment_for_target(&key(&[0, 0, 1, -1, 2, 1]))
            .unwrap(),
        Some(vec![0, 0, 1, -1, 1, 1])
    );
    assert!(terminals.classify(&key(&FOUR_LINE_SECTOR)).is_none());
}

fn search(depth: usize) -> SectorSearchDiamond {
    SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        depth,
        SectorSearchLimits::default(),
    )
    .unwrap()
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn assert_complete_provenance(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
) {
    assert_eq!(sources.len(), search.offset_count() * ORDINARY_ROWS.len());
    for (ordinal, provenance) in sources.provenance().iter().enumerate() {
        assert_eq!(
            provenance.translated().offset().values(),
            search.offsets()[ordinal / ORDINARY_ROWS.len()].values()
        );
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal % ORDINARY_ROWS.len()]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_selected_provenance(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
    selected: &[usize],
) {
    assert_eq!(sources.len(), selected.len());
    for (provenance, &ordinal) in sources.provenance().iter().zip(selected) {
        assert_eq!(
            provenance.translated().offset().values(),
            search.offsets()[ordinal / ORDINARY_ROWS.len()].values()
        );
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal % ORDINARY_ROWS.len()]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_rule(
    rule: &ParametricRule,
    pivot: &[i64; 6],
    rhs: &[[i64; 6]],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), OPPOSITE_PAIR_REPLAY_ANCHOR);
    assert_eq!(rule.pivot().values(), pivot);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        rhs.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
    );
    assert_eq!(
        (
            rule.replay().source_rows_used(),
            rule.replay().shift_columns_checked(),
            rule.replay().exact_operations(),
        ),
        parametric
    );
    assert_eq!(concrete_metrics(rule.concrete_replay()), concrete);
}

fn assert_complete_compact_equivalence(complete: &ParametricRule, compact: &ParametricRule) {
    assert_eq!(
        complete
            .source_combination()
            .iter()
            .map(|source| (source.row_id(), source.coefficient()))
            .collect::<Vec<_>>(),
        compact
            .source_combination()
            .iter()
            .map(|source| (source.row_id(), source.coefficient()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        complete
            .right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>(),
        compact
            .right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>()
    );
}

fn coefficients(rule: &ParametricRule) -> Vec<&crate::algebra::IndexedCoefficient> {
    rule.source_combination()
        .iter()
        .map(|source| source.coefficient())
        .collect()
}

fn rhs_coefficients(rule: &ParametricRule) -> Vec<&crate::algebra::IndexedCoefficient> {
    rule.right_hand_side()
        .iter()
        .map(|term| term.coefficient())
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

fn canonical(canonicalizer: &crate::sector::symmetry::Canonicalizer, powers: [i64; 6]) -> Vec<i64> {
    canonicalizer
        .canonicalize(&key(&powers))
        .unwrap()
        .canonical()
        .powers()
        .to_vec()
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
            let raw = std::array::from_fn::<_, 6, _>(|position| {
                assignment[position]
                    .checked_add(term.shift().values()[position])
                    .unwrap()
            });
            canonical(canonicalizer, raw)
        })
        .collect()
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}
