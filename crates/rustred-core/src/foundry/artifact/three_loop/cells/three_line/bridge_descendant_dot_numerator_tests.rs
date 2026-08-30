use std::collections::BTreeSet;

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
use super::bridge_descendant_dot_numerator::{
    BRIDGE_DESCENDANT_SECTOR, BRIDGE_DESCENDANT_TARGET_SHIFT, BridgeDescendantEndpointBuild,
    bridge_descendant_search_depth, derive_bridge_descendant_dot_numerator_endpoint,
    derive_bridge_descendant_endpoint_build, fixed_endpoint,
};
use super::decorated_path_numerator::derive_decorated_path_numerator_cells;

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
const TARGET: [i64; 6] = [-1, 0, 1, 0, 2, 1];
const RHS: [[i64; 6]; 2] = [[0, 0, 1, -1, 0, 0], [0, 0, 0, 0, 0, 0]];

#[test]
fn complete_depth_zero_selection_and_compact_reprojection_are_exact() {
    let BridgeDescendantEndpointBuild {
        context,
        endpoint,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_bridge_descendant_endpoint_build(true).unwrap();
    let witness = selection_witness.expect("exact test retains the complete span");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(BRIDGE_DESCENDANT_SECTOR).unwrap(),
        bridge_descendant_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(bridge_descendant_search_depth(), 0);
    assert_eq!(search.offset_count(), 1);
    assert_eq!(search.offsets()[0].values(), [0; 6]);

    assert_eq!(witness.complete_sources.len(), ORDINARY_ROWS.len());
    assert_eq!(
        selected_ordinals(&witness.complete_rule),
        COMPLETE_SELECTION
    );
    assert_eq!(
        selected_complete_source_ordinals.as_ref(),
        COMPLETE_SELECTION
    );
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

    assert_eq!(endpoint.sources().len(), COMPLETE_SELECTION.len());
    assert_eq!(selected_ordinals(endpoint.rule()), [0, 1]);
    assert_eq!(
        endpoint.rule().right_hand_side().len(),
        witness.complete_rule.right_hand_side().len()
    );
    for (compact, complete) in endpoint
        .rule()
        .right_hand_side()
        .iter()
        .zip(witness.complete_rule.right_hand_side())
    {
        assert_eq!(compact.shift(), complete.shift());
        assert_eq!(compact.coefficient(), complete.coefficient());
    }
    for ((compact, complete), &complete_ordinal) in endpoint
        .rule()
        .source_combination()
        .iter()
        .zip(witness.complete_rule.source_combination())
        .zip(&COMPLETE_SELECTION)
    {
        assert_eq!(complete.source_ordinal(), complete_ordinal);
        assert_eq!(compact.row_id(), complete.row_id());
        assert_eq!(compact.coefficient(), complete.coefficient());
    }
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

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for sources in [&witness.complete_sources, endpoint.sources()] {
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
            BRIDGE_DESCENDANT_SECTOR.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_endpoint());
        assert_eq!(evidence.original_relations().len(), sources.len());
        assert_eq!(evidence.term_projections().len(), sources.len());
        assert_eq!(evidence.stabilizer_group_elements().len(), 2);
    }
}

#[test]
fn endpoint_rule_replay_descent_domain_and_rebuild_are_exact() {
    let build = derive_bridge_descendant_endpoint_build(true).unwrap();
    let witness = build
        .selection_witness
        .as_ref()
        .expect("exact test retains the complete span");
    for (rule, shift_columns, exact_operations) in [
        (&witness.complete_rule, 7, 19),
        (build.endpoint.rule(), 4, 16),
    ] {
        assert_eq!(rule.anchor().powers(), BRIDGE_DESCENDANT_SECTOR);
        assert_eq!(rule.pivot().values(), BRIDGE_DESCENDANT_TARGET_SHIFT);
        assert_eq!(
            rule.right_hand_side()
                .iter()
                .map(|term| term.shift().values())
                .collect::<Vec<_>>(),
            RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
        );
        assert!(rule.nonzero_guards().is_empty());
        assert_eq!(rule.replay().source_rows_used(), 2);
        assert_eq!(rule.replay().shift_columns_checked(), shift_columns);
        assert_eq!(rule.replay().exact_operations(), exact_operations);
        assert_concrete_metrics(rule, (2, 8, 2, 11, 0, 29, 9));
    }

    assert!(build.endpoint.guards().is_empty());
    assert!(build.endpoint.pruned_rhs_ordinals().is_empty());
    assert_eq!(
        build.endpoint.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.endpoint.application_domain().bounds(),
        BRIDGE_DESCENDANT_SECTOR.map(|power| InteriorBounds::new(power, power))
    );
    assert_eq!(build.endpoint.fixed_restrictions(), fixed_endpoint());
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
        Some(BRIDGE_DESCENDANT_SECTOR.to_vec())
    );

    let replay = replay_rule_at_concrete_assignment(
        &build.context,
        build.endpoint.sources().relations(),
        build.endpoint.rule(),
        &BRIDGE_DESCENDANT_SECTOR,
        Default::default(),
    )
    .unwrap();
    assert_eq!(concrete_metrics(&replay), (2, 8, 2, 11, 0, 29, 9));

    // This is a singleton endpoint, not an overclaimed negative-power or dot
    // ray; extreme machine powers are therefore structurally inapplicable.
    for unowned in [
        [-2, 0, 1, 0, 2, 1],
        [i64::MIN, 0, 1, 0, 2, 1],
        [-1, 0, 1, 0, 3, 1],
    ] {
        assert!(
            build
                .endpoint
                .assignment_for_target(&IntegralKey::try_new(unowned).unwrap())
                .unwrap()
                .is_none()
        );
    }

    let (_second_context, second) = derive_bridge_descendant_dot_numerator_endpoint().unwrap();
    assert_eq!(second.rule(), build.endpoint.rule());
    assert_eq!(
        second.application_domain(),
        build.endpoint.application_domain()
    );
    assert_eq!(
        second.fixed_restrictions(),
        build.endpoint.fixed_restrictions()
    );
}

#[test]
fn exact_s4_orbit_and_owned_children_reduce_the_frontier_by_one() {
    let (_context, endpoint) = derive_bridge_descendant_dot_numerator_endpoint().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let target = IntegralKey::try_new(TARGET).unwrap();
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

    let mut placement_orbits = BTreeSet::new();
    for (numerator, &numerator_power) in BRIDGE_DESCENDANT_SECTOR.iter().enumerate() {
        if numerator_power != 0 {
            continue;
        }
        for (dot, &dot_power) in BRIDGE_DESCENDANT_SECTOR.iter().enumerate() {
            if dot_power != 1 {
                continue;
            }
            let mut powers = BRIDGE_DESCENDANT_SECTOR;
            powers[numerator] = -1;
            powers[dot] = 2;
            placement_orbits.insert(
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
        placement_orbits,
        [
            TARGET.to_vec(),
            vec![0, 0, 2, -1, 1, 1],
            vec![0, 0, 1, -1, 2, 1],
            vec![-1, 0, 1, 0, 1, 2],
            vec![0, 0, 1, -1, 1, 2],
        ]
        .into_iter()
        .collect()
    );

    let children = canonical_children(&canonicalizer, &endpoint, &BRIDGE_DESCENDANT_SECTOR);
    assert_eq!(children, [vec![0, 0, 2, -1, 1, 1], vec![0, 0, 1, 0, 1, 1]]);
    let (_context, installed_endpoint, _installed_bulk) =
        derive_decorated_path_numerator_cells().unwrap();
    assert_eq!(
        installed_endpoint
            .assignment_for_target(&key(&children[0]))
            .unwrap(),
        Some(BRIDGE_DESCENDANT_SECTOR.to_vec())
    );
    assert!(matches!(
        terminals.classify(&key(&children[1])),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));

    // A dot and inactive numerator on this three-line sector have five
    // inequivalent canonical S4 representatives. This endpoint owns only the
    // first; the already-installed decorated orbit and three other obligations
    // remain disjoint.
    for (alternate, orbit_size) in [
        ([0, 0, 2, -1, 1, 1], 24),
        ([0, 0, 1, -1, 2, 1], 24),
        ([-1, 0, 1, 0, 1, 2], 12),
        ([0, 0, 1, -1, 1, 2], 24),
    ] {
        let alternate = IntegralKey::try_new(alternate).unwrap();
        let alternate_orbit = canonicalizer.orbit(&alternate).unwrap();
        assert_eq!(alternate_orbit.orbit_size(), orbit_size);
        assert_eq!(alternate_orbit.canonical().integral(), &alternate);
        assert_ne!(alternate_orbit.canonical().integral(), &target);
        assert!(
            endpoint
                .assignment_for_target(&alternate)
                .unwrap()
                .is_none()
        );
    }
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn assert_concrete_metrics(
    rule: &ParametricRule,
    expected: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(concrete_metrics(rule.concrete_replay()), expected);
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
