use std::collections::{BTreeMap, BTreeSet};

use crate::family::IntegralKey;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{ParametricRule, replay_rule_at_concrete_assignment};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::incident_path_dot_numerator_endpoint::{
    INCIDENT_PATH_DOT_PIVOT, INCIDENT_PATH_REPLAY_ANCHOR, INCIDENT_PATH_SOURCE,
    IncidentPathEndpointBuild, derive_incident_path_dot_numerator_endpoint,
    derive_incident_path_endpoint_build, fixed_source, incident_path_search_depth,
};
use super::undotted_path_numerator::{UNDOTTED_PATH_SECTOR, derive_undotted_path_numerator_cells};

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
const COMPLETE_SELECTION: [usize; 2] = [0, 3];
const TARGET: [i64; 6] = [0, 0, 1, -1, 2, 1];
const RHS: [[i64; 6]; 2] = [[0, 0, 0, 0, 0, 0], [0, 0, 0, 1, 0, 0]];

#[test]
fn complete_depth_zero_selection_and_independent_reprojection_are_exact() {
    let IncidentPathEndpointBuild {
        context,
        endpoint,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_incident_path_endpoint_build(true).unwrap();
    let witness = selection_witness.expect("exact test retains complete span");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(UNDOTTED_PATH_SECTOR).unwrap(),
        incident_path_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(incident_path_search_depth(), 0);
    assert_eq!(search.offset_count(), 1);
    assert_eq!(search.offsets()[0].values(), [0; 6]);

    assert_eq!(witness.complete_sources.len(), ORDINARY_ROWS.len());
    for (provenance, row) in witness
        .complete_sources
        .provenance()
        .iter()
        .zip(ORDINARY_ROWS)
    {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(provenance.translated().source_row().stable_string(), row);
        assert!(provenance.symmetry().is_none());
    }
    assert_eq!(
        selected_ordinals(&witness.complete_rule),
        COMPLETE_SELECTION
    );
    assert_eq!(
        selected_complete_source_ordinals.as_ref(),
        COMPLETE_SELECTION
    );
    assert_eq!(endpoint.sources().len(), COMPLETE_SELECTION.len());
    assert_eq!(selected_ordinals(endpoint.rule()), [0, 1]);
    for (provenance, &complete_ordinal) in endpoint
        .sources()
        .provenance()
        .iter()
        .zip(&COMPLETE_SELECTION)
    {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[complete_ordinal]
        );
        assert!(provenance.symmetry().is_none());
    }

    assert_eq!(
        witness
            .complete_rule
            .source_combination()
            .iter()
            .map(|source| (source.row_id(), source.coefficient()))
            .collect::<Vec<_>>(),
        endpoint
            .rule()
            .source_combination()
            .iter()
            .map(|source| (source.row_id(), source.coefficient()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        witness
            .complete_rule
            .right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>(),
        endpoint
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| (term.shift(), term.coefficient()))
            .collect::<Vec<_>>()
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for (sources, original_rows) in [
        (&witness.complete_sources, ORDINARY_ROWS.len()),
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
            panic!("generated endpoint sources must retain residual routes")
        };
        assert_eq!(
            evidence.domain().bounds(),
            INCIDENT_PATH_SOURCE.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_source());
        assert_eq!(evidence.original_relations().len(), original_rows);
        assert_eq!(evidence.term_projections().len(), original_rows);
    }
}

#[test]
fn exact_coefficients_replay_domain_and_rebuild_are_pinned() {
    let build = derive_incident_path_endpoint_build(true).unwrap();
    let complete = &build
        .selection_witness
        .as_ref()
        .expect("exact test retains complete span")
        .complete_rule;
    for (rule, parametric) in [(complete, (2, 14, 30)), (build.endpoint.rule(), (2, 6, 22))] {
        assert_eq!(rule.anchor().powers(), INCIDENT_PATH_REPLAY_ANCHOR);
        assert_eq!(rule.pivot().values(), INCIDENT_PATH_DOT_PIVOT);
        assert_eq!(
            rule.right_hand_side()
                .iter()
                .map(|term| term.shift().values())
                .collect::<Vec<_>>(),
            RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
        );
        assert_eq!(
            (
                rule.replay().source_rows_used(),
                rule.replay().shift_columns_checked(),
                rule.replay().exact_operations(),
            ),
            parametric
        );
        assert_eq!(
            concrete_metrics(rule.concrete_replay()),
            (2, 11, 2, 14, 0, 36, 12)
        );
        assert!(rule.nonzero_guards().is_empty());
    }

    let indexed = |expression| {
        build
            .context
            .lift(&build.context.base().coefficient_fixture(expression))
            .unwrap()
    };
    let minus_half = indexed("-1/2");
    let half = indexed("1/2");
    let pivot = build.context.integer(2);
    let scalar = indexed("(d-1)/2");
    assert_eq!(
        build
            .endpoint
            .rule()
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        [&minus_half, &half]
    );
    assert_eq!(build.endpoint.rule().pivot_guard().coefficient(), &pivot);
    assert_eq!(complete.pivot_guard().coefficient(), &pivot);
    assert_eq!(
        build
            .endpoint
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        [&scalar, &half]
    );
    assert!(build.endpoint.guards().is_empty());
    assert!(build.endpoint.pruned_rhs_ordinals().is_empty());
    assert_eq!(
        build.endpoint.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.endpoint.application_domain().bounds(),
        INCIDENT_PATH_SOURCE.map(|power| InteriorBounds::new(power, power))
    );
    assert_eq!(build.endpoint.fixed_restrictions(), fixed_source());
    assert!(
        build
            .endpoint
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );
    assert_eq!(
        build.endpoint.assignment_for_target(&key(&TARGET)).unwrap(),
        Some(INCIDENT_PATH_SOURCE.to_vec())
    );
    for unowned in [
        [0, 0, 1, -2, 2, 1],
        [0, 0, 1, i64::MIN, 2, 1],
        [0, 0, 1, -1, 3, 1],
        [0, 0, 2, -1, 1, 1],
    ] {
        assert!(
            build
                .endpoint
                .assignment_for_target(&key(&unowned))
                .unwrap()
                .is_none()
        );
    }

    let held_out = replay_rule_at_concrete_assignment(
        &build.context,
        build.endpoint.sources().relations(),
        build.endpoint.rule(),
        &INCIDENT_PATH_SOURCE,
        Default::default(),
    )
    .unwrap();
    assert_eq!(held_out.anchor().powers(), INCIDENT_PATH_SOURCE);
    assert_eq!(
        concrete_metrics(&held_out),
        concrete_metrics(build.endpoint.rule().concrete_replay())
    );

    let (_second_context, second) = derive_incident_path_dot_numerator_endpoint().unwrap();
    assert_eq!(second.rule(), build.endpoint.rule());
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
fn exhaustive_s4_placement_boundary_and_child_routing_are_exact() {
    let (_context, endpoint) = derive_incident_path_dot_numerator_endpoint().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let target = key(&TARGET);
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

    let mut classes = BTreeMap::<Vec<i64>, BTreeSet<(usize, usize)>>::new();
    for numerator in [0, 1, 3] {
        for dot in [2, 4, 5] {
            let mut powers = UNDOTTED_PATH_SECTOR;
            powers[numerator] = -1;
            powers[dot] = 2;
            classes
                .entry(canonical(&canonicalizer, powers))
                .or_default()
                .insert((numerator, dot));
        }
    }
    assert_eq!(
        classes,
        [
            (vec![-1, 0, 1, 0, 1, 2], [(0, 5)].into_iter().collect()),
            (
                vec![-1, 0, 1, 0, 2, 1],
                [(0, 2), (0, 4)].into_iter().collect(),
            ),
            (
                vec![0, 0, 1, -1, 1, 2],
                [(1, 5), (3, 5)].into_iter().collect(),
            ),
            (TARGET.to_vec(), [(1, 2), (3, 4)].into_iter().collect(),),
            (
                vec![0, 0, 2, -1, 1, 1],
                [(1, 4), (3, 2)].into_iter().collect(),
            ),
        ]
        .into_iter()
        .collect()
    );
    for (representative, orbit_size, multiplicity) in [
        ([-1, 0, 1, 0, 1, 2], 12, 2),
        ([-1, 0, 1, 0, 2, 1], 24, 1),
        ([0, 0, 1, -1, 1, 2], 24, 1),
        (TARGET, 24, 1),
        ([0, 0, 2, -1, 1, 1], 24, 1),
    ] {
        let alternate = key(&representative);
        let alternate_orbit = canonicalizer.orbit(&alternate).unwrap();
        assert_eq!(alternate_orbit.orbit_size(), orbit_size);
        assert!(
            alternate_orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == multiplicity)
        );
        assert_eq!(alternate_orbit.canonical().integral(), &alternate);
        assert_eq!(
            endpoint
                .assignment_for_target(&alternate)
                .unwrap()
                .is_some(),
            representative == TARGET
        );
    }

    let children = canonical_children(&canonicalizer, &endpoint, &INCIDENT_PATH_SOURCE);
    assert_eq!(
        children,
        [INCIDENT_PATH_SOURCE.to_vec(), UNDOTTED_PATH_SECTOR.to_vec()]
    );
    let (_context, installed_endpoint, _installed_bulk) =
        derive_undotted_path_numerator_cells().unwrap();
    assert_eq!(
        installed_endpoint
            .assignment_for_target(&key(&INCIDENT_PATH_SOURCE))
            .unwrap(),
        Some(UNDOTTED_PATH_SECTOR.to_vec())
    );
    assert!(matches!(
        terminals.classify(&key(&UNDOTTED_PATH_SECTOR)),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));
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
