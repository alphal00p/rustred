use std::collections::{BTreeMap, BTreeSet};

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{
    ParametricGuardOrigin, ParametricRule, ParametricRuleError, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::FOUR_LINE_SECTOR;
use super::super::exceptional::derive_exceptional_four_line_cells;
use super::incident_two_dot_endpoint::{
    INCIDENT_TWO_DOT_NUMERATOR_PIVOT, IncidentTwoDotEndpointBuild,
    derive_incident_two_dot_candidate, derive_incident_two_dot_endpoint_build,
    derive_incident_two_dot_numerator_endpoint, fixed_endpoint, incident_two_dot_search_depth,
};

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
const COMPLETE_SELECTION: [usize; 5] = [18, 21, 27, 28, 30];
const TARGET: [i64; 6] = [0, 1, 2, 2, 1, -1];
const RHS: [[i64; 6]; 4] = [
    [0, 0, 0, 1, 1, 0],
    [0, 0, 0, 0, 2, 0],
    [0, -1, 0, 1, 1, 0],
    [0, 0, 0, 0, 0, 0],
];

#[test]
fn complete_depth_one_selection_and_compact_reprojection_are_exact() {
    assert_eq!(
        derive_incident_two_dot_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    let IncidentTwoDotEndpointBuild {
        context,
        endpoint,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_incident_two_dot_endpoint_build(true).unwrap();
    let witness = selection_witness.expect("exact test retains the complete source span");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        incident_two_dot_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(incident_two_dot_search_depth(), 1);
    assert_eq!(search.offset_count(), DEPTH_ONE_OFFSETS.len());
    assert_eq!(
        search
            .offsets()
            .iter()
            .map(|offset| offset.values())
            .collect::<Vec<_>>(),
        DEPTH_ONE_OFFSETS
            .iter()
            .map(<[i64; 6]>::as_slice)
            .collect::<Vec<_>>()
    );

    assert_eq!(witness.complete_sources.len(), 7 * ORDINARY_ROWS.len());
    assert_eq!(
        selected_ordinals(&witness.complete_rule),
        COMPLETE_SELECTION
    );
    assert_eq!(
        selected_complete_source_ordinals.as_ref(),
        COMPLETE_SELECTION
    );
    assert_complete_provenance(&witness.complete_sources, &search);

    assert_eq!(endpoint.sources().len(), COMPLETE_SELECTION.len());
    assert_eq!(
        selected_ordinals(endpoint.rule()),
        (0..5).collect::<Vec<_>>()
    );
    for (provenance, &complete_ordinal) in endpoint
        .sources()
        .provenance()
        .iter()
        .zip(&COMPLETE_SELECTION)
    {
        assert_eq!(
            provenance.translated().offset().values(),
            DEPTH_ONE_OFFSETS[complete_ordinal / ORDINARY_ROWS.len()]
        );
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[complete_ordinal % ORDINARY_ROWS.len()]
        );
        assert!(provenance.symmetry().is_none());
    }

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for (sources, original_rows) in [
        (&witness.complete_sources, 63),
        (endpoint.sources(), COMPLETE_SELECTION.len()),
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
            panic!("generated endpoint sources must retain residual projection routes")
        };
        assert_eq!(
            evidence.domain().bounds(),
            FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_endpoint());
        assert_eq!(
            evidence.stabilizer_group_elements(),
            [0, 1, 2, 3, 20, 21, 22, 23]
        );
        assert_eq!(evidence.original_relations().len(), original_rows);
        assert_eq!(evidence.term_projections().len(), original_rows);
    }
}

#[test]
fn exact_coefficients_guard_replay_domain_and_rebuild_are_pinned() {
    let build = derive_incident_two_dot_endpoint_build(true).unwrap();
    let complete = &build
        .selection_witness
        .as_ref()
        .expect("exact test retains the complete source span")
        .complete_rule;
    let compact = build.endpoint.rule();
    assert_rule_metrics(complete, (5, 19, 64), (5, 28, 4, 33, 1, 92, 20));
    assert_rule_metrics(compact, (5, 11, 56), (5, 28, 4, 33, 0, 91, 20));

    assert_eq!(
        complete
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        compact
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        complete
            .source_combination()
            .iter()
            .map(|source| source.row_id())
            .collect::<Vec<_>>(),
        compact
            .source_combination()
            .iter()
            .map(|source| source.row_id())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        complete
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        compact
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>()
    );

    let indexed = |expression| {
        build
            .context
            .lift(&build.context.base().coefficient_fixture(expression))
            .unwrap()
    };
    let first_source = indexed("(21-6*d)/8");
    let second_source = indexed("(2*d-7)/4");
    let minus_two = build.context.integer(-2);
    let minus_one = build.context.integer(-1);
    let one = build.context.one();
    assert_eq!(
        compact
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        [&first_source, &second_source, &minus_two, &minus_one, &one]
    );

    let minus_three = build.context.integer(-3);
    let minus_four = build.context.integer(-4);
    let scalar_corner = indexed("(6*d^2-37*d+56)/8");
    assert_eq!(
        compact
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        [&minus_three, &minus_four, &one, &scalar_corner]
    );

    let pivot_polynomial = build
        .context
        .numerator_condition_with_limits(&minus_two, Default::default())
        .unwrap();
    for rule in [complete, compact] {
        assert_eq!(rule.pivot_guard().coefficient(), &minus_two);
        assert_eq!(rule.pivot_guard().nonzero_polynomial(), &pivot_polynomial);
    }
    let complete_guard_coefficient = indexed("6-3*d");
    let complete_guard = build
        .context
        .numerator_condition_with_limits(&complete_guard_coefficient, Default::default())
        .unwrap();
    assert_eq!(complete.nonzero_guards().len(), 1);
    assert_eq!(complete.nonzero_guards()[0].polynomial(), &complete_guard);
    assert_eq!(complete.nonzero_guards()[0].origins().len(), 1);
    let ParametricGuardOrigin::ReducerPivotNumerator {
        source_ordinal,
        row_id,
        pivot_column,
        pivot_shift,
    } = &complete.nonzero_guards()[0].origins()[0]
    else {
        panic!("complete guard must retain its reducer-pivot provenance")
    };
    assert_eq!(*source_ordinal, 21);
    assert_eq!(row_id.stable_string(), "ordinary-ibp:1:0");
    assert_eq!(*pivot_column, 14);
    assert_eq!(pivot_shift.values(), [0, 0, 0, 0, 0, -1]);
    assert!(compact.nonzero_guards().is_empty());
    assert!(build.endpoint.guards().is_empty());

    assert_eq!(
        build.endpoint.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.endpoint.application_domain().bounds(),
        FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power))
    );
    assert_eq!(build.endpoint.fixed_restrictions(), fixed_endpoint());
    assert!(build.endpoint.pruned_rhs_ordinals().is_empty());
    assert!(
        build
            .endpoint
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );

    let target = IntegralKey::try_new(TARGET).unwrap();
    assert_eq!(
        build.endpoint.assignment_for_target(&target).unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    for unowned in [
        [0, 1, 2, 2, 1, -2],
        [0, 1, 2, 2, 1, i64::MIN],
        [0, 1, 2, 3, 1, -1],
        [0, 1, 1, 2, 1, -1],
        [0, 1, 2, 2, 1, 0],
    ] {
        assert!(
            build
                .endpoint
                .assignment_for_target(&IntegralKey::try_new(unowned).unwrap())
                .unwrap()
                .is_none()
        );
    }

    let held_out = replay_rule_at_concrete_assignment(
        &build.context,
        build.endpoint.sources().relations(),
        compact,
        &FOUR_LINE_SECTOR,
        Default::default(),
    )
    .unwrap();
    assert_eq!(held_out, compact.concrete_replay().clone());

    let (_second_context, second) = derive_incident_two_dot_numerator_endpoint().unwrap();
    assert_eq!(second.rule(), compact);
    assert_eq!(
        second.sources().relations(),
        build.endpoint.sources().relations()
    );
    assert_eq!(
        second.application_domain(),
        build.endpoint.application_domain()
    );
    assert_eq!(
        second.fixed_restrictions(),
        build.endpoint.fixed_restrictions()
    );
    assert_eq!(second.guards(), build.endpoint.guards());
}

#[test]
fn exact_s4_partition_nonownership_and_child_routing_are_pinned() {
    let (_context, endpoint) = derive_incident_two_dot_numerator_endpoint().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let target = IntegralKey::try_new(TARGET).unwrap();
    let target_orbit = canonicalizer.orbit(&target).unwrap();
    assert_eq!(target_orbit.group_order(), 24);
    assert_eq!(target_orbit.orbit_size(), 12);
    assert!(
        target_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 2)
    );
    assert_eq!(target_orbit.canonical().integral(), &target);

    let mut placement_classes = BTreeMap::<[i64; 6], BTreeSet<(usize, usize, usize)>>::new();
    for inactive in [0, 5] {
        for first_dot in 1..5 {
            for second_dot in (first_dot + 1)..5 {
                let mut powers = FOUR_LINE_SECTOR;
                powers[inactive] = -1;
                powers[first_dot] = 2;
                powers[second_dot] = 2;
                let canonical: [i64; 6] = canonicalizer
                    .canonicalize(&IntegralKey::try_new(powers).unwrap())
                    .unwrap()
                    .canonical()
                    .powers()
                    .try_into()
                    .unwrap();
                placement_classes
                    .entry(canonical)
                    .or_default()
                    .insert((inactive, first_dot, second_dot));
            }
        }
    }
    let expected = BTreeMap::from([
        (
            [0, 1, 1, 2, 2, -1],
            BTreeSet::from([(0, 1, 4), (0, 2, 3), (5, 1, 2), (5, 3, 4)]),
        ),
        (
            [0, 1, 2, 1, 2, -1],
            BTreeSet::from([(0, 1, 3), (0, 2, 4), (5, 1, 3), (5, 2, 4)]),
        ),
        (
            TARGET,
            BTreeSet::from([(0, 1, 2), (0, 3, 4), (5, 1, 4), (5, 2, 3)]),
        ),
    ]);
    assert_eq!(placement_classes, expected);
    for representative in placement_classes.keys() {
        let representative = IntegralKey::try_new(*representative).unwrap();
        let orbit = canonicalizer.orbit(&representative).unwrap();
        assert_eq!(orbit.group_order(), 24);
        assert_eq!(orbit.orbit_size(), 12);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == 2)
        );
        assert_eq!(orbit.canonical().integral(), &representative);
        if representative == target {
            assert!(
                endpoint
                    .assignment_for_target(&representative)
                    .unwrap()
                    .is_some()
            );
        } else {
            assert!(
                endpoint
                    .assignment_for_target(&representative)
                    .unwrap()
                    .is_none()
            );
        }
    }

    for outside in [
        [0, 1, 2, 2, 1, -2],
        [0, 1, 2, 2, 1, i64::MIN],
        [0, 1, 2, 3, 1, -1],
        [0, 1, 1, 2, 1, -1],
    ] {
        let outside = canonicalizer
            .canonicalize(&IntegralKey::try_new(outside).unwrap())
            .unwrap();
        assert!(
            endpoint
                .assignment_for_target(outside.canonical())
                .unwrap()
                .is_none()
        );
    }

    let children = canonical_children(&canonicalizer, &endpoint, &FOUR_LINE_SECTOR);
    assert_eq!(
        children,
        [
            vec![0, 1, 1, 2, 2, 0],
            vec![0, 1, 1, 1, 3, 0],
            vec![0, 0, 1, 0, 2, 2],
            vec![0, 1, 1, 1, 1, 0],
        ]
    );
    let exceptional = derive_exceptional_four_line_cells().unwrap();
    assert!(
        exceptional
            .adjacent
            .assignment_for_target(&key(&children[0]))
            .unwrap()
            .is_some()
    );
    assert!(
        exceptional
            .triple
            .assignment_for_target(&key(&children[1]))
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        terminals.classify(&key(&children[2])),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));
    assert!(matches!(
        terminals.classify(&key(&children[3])),
        Some(terminal) if terminal.kind() == ReachabilityTerminalKind::Master
    ));
    assert!(
        endpoint
            .assignment_for_target(&key(&children[3]))
            .unwrap()
            .is_none()
    );
}

fn assert_complete_provenance(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
) {
    assert_eq!(sources.len(), search.offset_count() * ORDINARY_ROWS.len());
    for (ordinal, provenance) in sources.provenance().iter().enumerate() {
        let offset_ordinal = ordinal / ORDINARY_ROWS.len();
        let row_ordinal = ordinal % ORDINARY_ROWS.len();
        assert_eq!(
            provenance.translated().offset().values(),
            search.offsets()[offset_ordinal].values()
        );
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[row_ordinal]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_rule_metrics(
    rule: &ParametricRule,
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), FOUR_LINE_SECTOR);
    assert_eq!(rule.pivot().values(), INCIDENT_TWO_DOT_NUMERATOR_PIVOT);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
    );
    assert_eq!(rule.replay().source_rows_used(), parametric.0);
    assert_eq!(rule.replay().shift_columns_checked(), parametric.1);
    assert_eq!(rule.replay().exact_operations(), parametric.2);
    let replay = rule.concrete_replay();
    assert_eq!(replay.source_contributions_checked(), concrete.0);
    assert_eq!(replay.source_terms_checked(), concrete.1);
    assert_eq!(replay.right_hand_side_terms_checked(), concrete.2);
    assert_eq!(replay.integral_keys_checked(), concrete.3);
    assert_eq!(replay.nonzero_guards_checked(), concrete.4);
    assert_eq!(replay.exact_operations(), concrete.5);
    assert_eq!(replay.peak_retained_coefficient_terms(), concrete.6);
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
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
