use std::collections::{BTreeMap, BTreeSet};

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleError, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::super::three_line::derive_three_line_cells;
use super::FACTORIZED_FACE_SECTOR;
use super::triangle_dot_numerator::{
    BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT, FactorizedTriangleDotNumeratorBuild,
    OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT, OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT,
    RAY_FREE_POSITION, derive_complete_endpoint_candidate, derive_complete_ray_candidate,
    derive_factorized_triangle_dot_numerator_build, derive_factorized_triangle_dot_numerator_cells,
    endpoint_search_depth, fixed_endpoint_source, fixed_ray_source, ray_search_depth,
    repeated_endpoint_search_depth,
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
    [0, -1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 1, 0],
    [0, 0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0, 0],
];
const RAY_SELECTION: [usize; 10] = [0, 3, 6, 7, 8, 18, 19, 20, 24, 26];
const ENDPOINT_SELECTION: [usize; 10] = [3, 4, 6, 7, 8, 18, 19, 20, 24, 26];
const REPEATED_SELECTION: [usize; 34] = [
    18, 21, 22, 24, 25, 26, 27, 30, 31, 32, 33, 34, 36, 37, 39, 40, 42, 43, 44, 45, 46, 47, 49, 51,
    52, 54, 58, 117, 123, 125, 130, 133, 144, 145,
];
const RAY_RHS: [[i64; 6]; 8] = [
    [0, -1, 0, 0, -1, 1],
    [0, -1, 0, -1, 0, 1],
    [0, 0, 1, -1, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -1, 1],
    [0, 0, 0, -1, 0, 1],
    [0, 0, 0, 0, -1, 0],
    [0, 0, 0, -1, 0, 0],
];
const REPEATED_RHS: [[i64; 6]; 8] = [
    [0, -1, 0, 0, -1, 1],
    [0, -1, 0, -1, 0, 1],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -1, 1],
    [0, 0, 0, -1, 1, 0],
    [0, 0, 0, -1, 0, 1],
    [0, 0, 0, 0, -1, 0],
    [0, 0, 0, -1, 0, 0],
];

#[test]
fn complete_search_minimality_selection_and_reprojection_are_exact() {
    assert_eq!(
        derive_complete_ray_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    assert_eq!(
        derive_complete_endpoint_candidate(0, &OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    for depth in [0, 1] {
        assert_eq!(
            derive_complete_endpoint_candidate(depth, &OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT,),
            Err(ArtifactError::ParametricRule(
                ParametricRuleError::TargetShiftAbsent
            ))
        );
    }

    let FactorizedTriangleDotNumeratorBuild {
        context,
        bridge_opposite_ray,
        opposite_edge_endpoint,
        opposite_edge_repeated_endpoint,
        ray_selected_complete_source_ordinals,
        endpoint_selected_complete_source_ordinals,
        repeated_endpoint_selected_complete_source_ordinals,
        selection_witness,
    } = derive_factorized_triangle_dot_numerator_build(true).unwrap();
    let witness = selection_witness.expect("exact tests retain complete spans");
    let depth_one = search(ray_search_depth());
    assert_eq!(ray_search_depth(), 1);
    assert_eq!(endpoint_search_depth(), 1);
    assert_eq!(search_offsets(&depth_one), DEPTH_ONE_OFFSETS);
    let depth_two = search(repeated_endpoint_search_depth());
    assert_eq!(repeated_endpoint_search_depth(), 2);
    assert_eq!(depth_two.offset_count(), 28);

    for (sources, search) in [
        (&witness.complete_ray_sources, &depth_one),
        (&witness.complete_endpoint_sources, &depth_one),
        (&witness.complete_repeated_endpoint_sources, &depth_two),
    ] {
        assert_complete_provenance(sources, search);
    }
    assert_eq!(witness.complete_ray_sources.len(), 63);
    assert_eq!(witness.complete_endpoint_sources.len(), 63);
    assert_eq!(witness.complete_repeated_endpoint_sources.len(), 252);
    assert_eq!(selected_ordinals(&witness.complete_ray_rule), RAY_SELECTION);
    assert_eq!(
        ray_selected_complete_source_ordinals.as_ref(),
        RAY_SELECTION
    );
    assert_eq!(
        selected_ordinals(&witness.complete_endpoint_rule),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        endpoint_selected_complete_source_ordinals.as_ref(),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        selected_ordinals(&witness.complete_repeated_endpoint_rule),
        REPEATED_SELECTION
    );
    assert_eq!(
        repeated_endpoint_selected_complete_source_ordinals.as_ref(),
        REPEATED_SELECTION
    );
    assert_selected_provenance(bridge_opposite_ray.sources(), &depth_one, &RAY_SELECTION);
    assert_selected_provenance(
        opposite_edge_endpoint.sources(),
        &depth_one,
        &ENDPOINT_SELECTION,
    );
    assert_selected_provenance(
        opposite_edge_repeated_endpoint.sources(),
        &depth_two,
        &REPEATED_SELECTION,
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for sources in [
        &witness.complete_ray_sources,
        bridge_opposite_ray.sources(),
        &witness.complete_endpoint_sources,
        opposite_edge_endpoint.sources(),
        &witness.complete_repeated_endpoint_sources,
        opposite_edge_repeated_endpoint.sources(),
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
    }
    assert_projection(
        &witness.complete_ray_sources,
        ray_source_bounds(),
        &fixed_ray_source(),
        63,
    );
    assert_projection(
        bridge_opposite_ray.sources(),
        ray_source_bounds(),
        &fixed_ray_source(),
        RAY_SELECTION.len(),
    );
    for (sources, rows) in [
        (&witness.complete_endpoint_sources, 63),
        (opposite_edge_endpoint.sources(), ENDPOINT_SELECTION.len()),
        (&witness.complete_repeated_endpoint_sources, 252),
        (
            opposite_edge_repeated_endpoint.sources(),
            REPEATED_SELECTION.len(),
        ),
    ] {
        assert_projection(
            sources,
            FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power)),
            &fixed_endpoint_source(),
            rows,
        );
    }
}

#[test]
fn exact_coefficients_guards_replay_domains_and_machine_boundary_are_pinned() {
    let build = derive_factorized_triangle_dot_numerator_build(true).unwrap();
    let witness = build
        .selection_witness
        .as_ref()
        .expect("exact tests retain complete spans");
    assert_rule(
        &witness.complete_ray_rule,
        &BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT,
        &RAY_RHS,
        (10, 51, 151),
        (10, 61, 8, 70, 10, 203, 48),
    );
    assert_rule(
        build.bridge_opposite_ray.rule(),
        &BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT,
        &RAY_RHS,
        (10, 22, 122),
        (10, 61, 8, 70, 9, 202, 48),
    );
    assert_rule(
        &witness.complete_endpoint_rule,
        &OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT,
        &RAY_RHS,
        (10, 51, 149),
        (10, 60, 8, 69, 3, 194, 49),
    );
    assert_rule(
        build.opposite_edge_endpoint.rule(),
        &OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT,
        &RAY_RHS,
        (10, 22, 120),
        (10, 60, 8, 69, 3, 194, 49),
    );
    assert_rule(
        &witness.complete_repeated_endpoint_rule,
        &OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT,
        &REPEATED_RHS,
        (34, 157, 572),
        (34, 234, 8, 243, 5, 701, 129),
    );
    assert_rule(
        build.opposite_edge_repeated_endpoint.rule(),
        &OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT,
        &REPEATED_RHS,
        (34, 53, 468),
        (34, 234, 8, 243, 4, 700, 129),
    );
    for (complete, compact) in [
        (&witness.complete_ray_rule, build.bridge_opposite_ray.rule()),
        (
            &witness.complete_endpoint_rule,
            build.opposite_edge_endpoint.rule(),
        ),
        (
            &witness.complete_repeated_endpoint_rule,
            build.opposite_edge_repeated_endpoint.rule(),
        ),
    ] {
        assert_complete_compact_equivalence(complete, compact);
    }

    assert_eq!(
        coefficient_expression(build.bridge_opposite_ray.rule().pivot_guard().coefficient()),
        "-1+d"
    );
    assert_eq!(
        guard_expressions(build.bridge_opposite_ray.rule()),
        [
            "n4",
            "2*n4",
            "-n4",
            "-1+d",
            "d*n4-n4",
            "3*n4",
            "6*n4",
            "2*d*n4-2*n4",
            "6*d*n4-6*n4",
        ]
    );
    assert_eq!(
        rhs_coefficient_expressions(build.bridge_opposite_ray.rule()),
        [
            "-1/2/n4",
            "1/2/n4",
            "(-1+d-2*n4)/(d*n4-n4)",
            "1/3*(d-3*n4)/n4",
            "-1/6/n4",
            "1/6/n4",
            "1/2/n4",
            "(-1+d-2*n4)/(2*d*n4-2*n4)",
        ]
    );
    assert_eq!(
        cell_guard_expressions(&build.bridge_opposite_ray),
        [
            "n4",
            "2*n4",
            "-n4",
            "-1+d",
            "d*n4-n4",
            "3*n4",
            "6*n4",
            "2*d*n4-2*n4",
            "6*d*n4-6*n4",
        ]
    );

    assert_eq!(
        coefficient_expression(
            build
                .opposite_edge_endpoint
                .rule()
                .pivot_guard()
                .coefficient()
        ),
        "-1+d"
    );
    assert_eq!(
        guard_expressions(build.opposite_edge_endpoint.rule()),
        ["-1+d", "-2+2*d", "-6+6*d"]
    );
    assert_eq!(
        rhs_coefficient_expressions(build.opposite_edge_endpoint.rule()),
        [
            "1/2",
            "-1/2",
            "(-2+d)/(-1+d)",
            "1/3*(-3+d)",
            "-1/6",
            "1/6",
            "-1/2",
            "(-3+2*d)/(-2+2*d)",
        ]
    );
    assert_eq!(
        cell_guard_expressions(&build.opposite_edge_endpoint),
        ["-1+d", "-2+2*d", "-6+6*d"]
    );

    assert_eq!(
        coefficient_expression(
            build
                .opposite_edge_repeated_endpoint
                .rule()
                .pivot_guard()
                .coefficient()
        ),
        "2"
    );
    assert_eq!(
        guard_expressions(build.opposite_edge_repeated_endpoint.rule()),
        ["-1+d", "-12+12*d", "-72+72*d", "-8+8*d"]
    );
    assert_eq!(
        rhs_coefficient_expressions(build.opposite_edge_repeated_endpoint.rule()),
        [
            "1/8*(-4+d)",
            "1/8*(4-d)",
            "1/18*(24-11*d+d^2)",
            "1/72*(4+d)",
            "(-40+30*d-5*d^2)/(-12+12*d)",
            "(-236+177*d-31*d^2)/(-72+72*d)",
            "1/8*(4-d)",
            "(-60+81*d-36*d^2+5*d^3)/(-8+8*d)",
        ]
    );
    assert_eq!(
        cell_guard_expressions(&build.opposite_edge_repeated_endpoint),
        ["-1+d", "-12+12*d", "-72+72*d", "-8+8*d"]
    );

    assert_eq!(
        build.bridge_opposite_ray.application_domain().bounds(),
        ray_application_bounds()
    );
    assert_eq!(
        build.bridge_opposite_ray.fixed_restrictions(),
        fixed_ray_source()
    );
    for cell in [
        &build.opposite_edge_endpoint,
        &build.opposite_edge_repeated_endpoint,
    ] {
        assert_eq!(
            cell.application_domain().bounds(),
            FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(cell.fixed_restrictions(), fixed_endpoint_source());
    }
    for cell in [
        &build.bridge_opposite_ray,
        &build.opposite_edge_endpoint,
        &build.opposite_edge_repeated_endpoint,
    ] {
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert!(cell.pruned_rhs_ordinals().is_empty());
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }

    for free in [1, 7, i64::MAX - 1] {
        assert_replay_at(
            &build.context,
            &build.bridge_opposite_ray,
            [0, 0, 1, 1, free, 1],
            (10, 61, 8, 70, 9, 202, 48),
        );
    }
    assert_replay_at(
        &build.context,
        &build.opposite_edge_endpoint,
        FACTORIZED_FACE_SECTOR,
        (10, 60, 8, 69, 3, 194, 49),
    );
    assert_replay_at(
        &build.context,
        &build.opposite_edge_repeated_endpoint,
        FACTORIZED_FACE_SECTOR,
        (34, 234, 8, 243, 4, 700, 129),
    );

    assert_eq!(
        build
            .bridge_opposite_ray
            .assignment_for_target(&key(&[0, -1, 1, 1, i64::MAX, 1]))
            .unwrap(),
        Some(vec![0, 0, 1, 1, i64::MAX - 1, 1])
    );
    assert!(
        build
            .bridge_opposite_ray
            .assignment_for_target(&key(&[0, -1, 1, 1, 1, 1]))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        build
            .opposite_edge_endpoint
            .assignment_for_target(&key(&[0, -1, 1, 2, 1, 1]))
            .unwrap(),
        Some(FACTORIZED_FACE_SECTOR.to_vec())
    );
    assert_eq!(
        build
            .opposite_edge_repeated_endpoint
            .assignment_for_target(&key(&[0, -1, 1, 3, 1, 1]))
            .unwrap(),
        Some(FACTORIZED_FACE_SECTOR.to_vec())
    );

    let (_context, second_ray, second_endpoint, second_repeated) =
        derive_factorized_triangle_dot_numerator_cells().unwrap();
    for (first, second) in [
        (&build.bridge_opposite_ray, &second_ray),
        (&build.opposite_edge_endpoint, &second_endpoint),
        (&build.opposite_edge_repeated_endpoint, &second_repeated),
    ] {
        assert_eq!(first.rule(), second.rule());
        assert_eq!(first.application_domain(), second.application_domain());
        assert_eq!(first.fixed_restrictions(), second.fixed_restrictions());
        assert_eq!(first.guards(), second.guards());
    }
}

#[test]
fn exhaustive_s4_placement_nonownership_and_boundary_children_are_exact() {
    let (_context, ray, endpoint, repeated) =
        derive_factorized_triangle_dot_numerator_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for dot_power in [2, 3, 7] {
        let mut classes = BTreeMap::<Vec<i64>, BTreeSet<(usize, usize)>>::new();
        for numerator in [0, 1] {
            for dot in [2, 3, 4, 5] {
                let mut powers = FACTORIZED_FACE_SECTOR;
                powers[numerator] = -1;
                powers[dot] = dot_power;
                classes
                    .entry(canonical(&canonicalizer, powers))
                    .or_default()
                    .insert((numerator, dot));
            }
        }
        let expected = BTreeSet::from([
            vec![0, -1, dot_power, 1, 1, 1],
            vec![0, -1, 1, 1, dot_power, 1],
            vec![0, -1, 1, dot_power, 1, 1],
            vec![0, -1, 1, 1, 1, dot_power],
        ]);
        assert_eq!(classes.keys().cloned().collect::<BTreeSet<_>>(), expected);
        for representative in classes.keys() {
            let representative = key(representative);
            let orbit = canonicalizer.orbit(&representative).unwrap();
            assert_eq!(orbit.orbit_size(), 24);
            assert!(
                orbit
                    .images()
                    .iter()
                    .all(|image| image.routing_multiplicity() == 1)
            );
            assert_eq!(orbit.canonical().integral(), &representative);
            assert_eq!(
                ray.assignment_for_target(&representative)
                    .unwrap()
                    .is_some(),
                representative.powers() == [0, -1, 1, 1, dot_power, 1]
            );
            assert_eq!(
                endpoint
                    .assignment_for_target(&representative)
                    .unwrap()
                    .is_some(),
                representative.powers() == [0, -1, 1, 2, 1, 1]
            );
            assert_eq!(
                repeated
                    .assignment_for_target(&representative)
                    .unwrap()
                    .is_some(),
                representative.powers() == [0, -1, 1, 3, 1, 1]
            );
        }
    }
    for unowned in [
        [0, -2, 1, 1, 2, 1],
        [0, -1, 1, 1, 2, 2],
        [0, -1, 1, 2, 2, 1],
        [0, -1, 1, 4, 1, 1],
    ] {
        let target = key(&unowned);
        assert!(ray.assignment_for_target(&target).unwrap().is_none());
        assert!(endpoint.assignment_for_target(&target).unwrap().is_none());
        assert!(repeated.assignment_for_target(&target).unwrap().is_none());
    }

    let expected_children = BTreeSet::from([
        vec![0, 0, 1, 1, -1, 2],
        vec![0, 0, 1, -1, 1, 2],
        vec![0, 0, 1, 0, 2, 1],
        vec![0, 0, 1, 1, 1, 1],
        vec![0, 0, 1, 1, 0, 2],
        vec![0, 0, 1, 0, 1, 2],
        vec![0, 0, 1, 1, 0, 1],
        vec![0, 0, 1, 0, 1, 1],
    ]);
    for cell in [&ray, &endpoint, &repeated] {
        assert_eq!(
            canonical_children(&canonicalizer, cell, &FACTORIZED_FACE_SECTOR)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            expected_children
        );
    }

    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let three_line = derive_three_line_cells().unwrap();
    for child in expected_children {
        let child_key = key(&child);
        match child.as_slice() {
            [0, 0, 1, -1, 1, 2] => assert_eq!(
                three_line
                    .factorized_path_middle_dot_numerator_ray
                    .assignment_for_target(&child_key)
                    .unwrap(),
                Some(vec![0, 0, 1, 0, 1, 1])
            ),
            [0, 0, 1, 1, -1, 2] => assert_eq!(
                three_line
                    .factorized_star_spoke_dot_numerator_ray
                    .assignment_for_target(&child_key)
                    .unwrap(),
                Some(vec![0, 0, 1, 1, 0, 1])
            ),
            _ => {
                let expected_owner = match child.as_slice() {
                    [0, 0, 1, 1, 1, 1] => 0,
                    [0, 0, 1, 1, 0, 2] | [0, 0, 1, 1, 0, 1] => 1,
                    _ => 2,
                };
                assert!(matches!(
                    terminals.classify(&child_key),
                    Some(terminal)
                        if terminal.kind() == ReachabilityTerminalKind::Factorization
                            && terminal.owner_ordinal() == expected_owner
                ));
            }
        }
    }
}

fn search(depth: usize) -> SectorSearchDiamond {
    SectorSearchDiamond::try_new(
        IntegralKey::try_new(FACTORIZED_FACE_SECTOR).unwrap(),
        depth,
        SectorSearchLimits::default(),
    )
    .unwrap()
}

fn search_offsets(search: &SectorSearchDiamond) -> Vec<[i64; 6]> {
    search
        .offsets()
        .iter()
        .map(|offset| offset.values().try_into().unwrap())
        .collect()
}

fn assert_complete_provenance(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
) {
    assert_eq!(sources.len(), search.offset_count() * ORDINARY_ROWS.len());
    for (ordinal, provenance) in sources.provenance().iter().enumerate() {
        let offset = ordinal / ORDINARY_ROWS.len();
        let row = ordinal % ORDINARY_ROWS.len();
        assert_eq!(
            provenance.translated().offset().values(),
            search.offsets()[offset].values()
        );
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[row]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_selected_provenance(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
    ordinals: &[usize],
) {
    assert_eq!(sources.len(), ordinals.len());
    for (provenance, &ordinal) in sources.provenance().iter().zip(ordinals) {
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

fn assert_projection(
    sources: &crate::foundry::cell::SourceViewBatch,
    bounds: [InteriorBounds; 6],
    fixed: &[crate::foundry::cell::FixedIndexRestriction],
    rows: usize,
) {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        panic!("generated sources must retain residual projection evidence")
    };
    assert_eq!(evidence.domain().bounds(), bounds);
    assert_eq!(evidence.fixed_restrictions(), fixed);
    assert_eq!(evidence.original_relations().len(), rows);
    assert_eq!(evidence.term_projections().len(), rows);
}

fn assert_rule(
    rule: &ParametricRule,
    pivot: &[i64; 6],
    rhs: &[[i64; 6]],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), FACTORIZED_FACE_SECTOR);
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

fn assert_replay_at(
    context: &crate::algebra::IndexedCoefficientContext,
    cell: &RuleCell,
    assignment: [i64; 6],
    metrics: (usize, usize, usize, usize, usize, usize, usize),
) {
    let replay = replay_rule_at_concrete_assignment(
        context,
        cell.sources().relations(),
        cell.rule(),
        &assignment,
        Default::default(),
    )
    .unwrap();
    assert_eq!(replay.anchor().powers(), assignment);
    assert_eq!(
        concrete_metrics(&replay),
        metrics,
        "held-out assignment {assignment:?}"
    );
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|source| source.source_ordinal())
        .collect()
}

fn guard_expressions(rule: &ParametricRule) -> Vec<String> {
    rule.nonzero_guards()
        .iter()
        .map(|guard| guard.polynomial().to_expression().to_string())
        .collect()
}

fn cell_guard_expressions(cell: &RuleCell) -> Vec<String> {
    cell.guards()
        .iter()
        .map(|guard| guard.polynomial().to_expression().to_string())
        .collect()
}

fn rhs_coefficient_expressions(rule: &ParametricRule) -> Vec<String> {
    rule.right_hand_side()
        .iter()
        .map(|term| coefficient_expression(term.coefficient()))
        .collect()
}

fn coefficient_expression(coefficient: &crate::algebra::IndexedCoefficient) -> String {
    coefficient.to_expression().to_string()
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

fn ray_source_bounds() -> [InteriorBounds; 6] {
    let mut bounds = FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power));
    bounds[RAY_FREE_POSITION] = InteriorBounds::new(1, i64::MAX);
    bounds
}

fn ray_application_bounds() -> [InteriorBounds; 6] {
    let mut bounds = FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power));
    bounds[RAY_FREE_POSITION] = InteriorBounds::new(1, i64::MAX - 1);
    bounds
}

fn canonical(canonicalizer: &crate::sector::symmetry::Canonicalizer, powers: [i64; 6]) -> Vec<i64> {
    canonicalizer
        .canonicalize(&IntegralKey::try_new(powers).unwrap())
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
