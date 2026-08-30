use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::foundry::cell::{
    FixedIndexRestriction, ResidualTermDisposition, RuleCell, RuleCellDomainProof, RuleCellLimits,
    SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::parametric::{ParametricRule, replay_rule_at_concrete_assignment};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::factorized_bridge_dot_numerator::{
    BRIDGE_DOT_NUMERATOR_PIVOT, BULK_REPLAY_ANCHOR, FACTORIZED_FOUR_LINE_SECTOR, FREE_POSITION,
    FactorizedBridgeDotNumeratorBuild, derive_factorized_bridge_dot_numerator_build,
    derive_factorized_bridge_dot_numerator_cells, factorized_bridge_dot_search_depth,
    fixed_endpoint, fixed_free_face,
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

const ENDPOINT_SELECTION: [usize; 5] = [0, 1, 2, 6, 8];
const MACHINE_SAFE_SELECTION: [usize; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
const BULK_SELECTION: [usize; 6] = [3, 4, 5, 6, 7, 8];
const ENDPOINT_RHS: [[i64; 6]; 2] = [[0, 0, 1, -1, 0, 0], [0, 0, 0, 0, 0, 0]];
const MACHINE_SAFE_RHS: [[i64; 6]; 5] = [
    [0, 0, 1, 0, 0, -1],
    [0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, -1],
    [0, 1, -1, 0, 0, 0],
];
const BULK_RHS: [[i64; 6]; 4] = [
    [0, 0, 1, 0, 0, -1],
    [0, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, -1],
];

#[test]
fn complete_depth_zero_rows_machine_safety_and_reprojection_are_exact() {
    let FactorizedBridgeDotNumeratorBuild {
        context,
        endpoint,
        bulk,
        endpoint_selected_complete_source_ordinals,
        machine_safe_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        selection_witness,
    } = derive_factorized_bridge_dot_numerator_build(true).unwrap();
    let selection = selection_witness.expect("exact test retains selection evidence");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FACTORIZED_FOUR_LINE_SECTOR).unwrap(),
        factorized_bridge_dot_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(factorized_bridge_dot_search_depth(), 0);
    assert_eq!(search.offset_count(), 1);
    assert_eq!(search.offsets()[0].values(), [0; 6]);

    assert_eq!(
        selection.complete_endpoint_sources.len(),
        ORDINARY_ROWS.len()
    );
    assert_complete_provenance(&selection.complete_endpoint_sources);
    assert_eq!(
        selected_ordinals(&selection.complete_endpoint_rule),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        endpoint_selected_complete_source_ordinals.as_ref(),
        ENDPOINT_SELECTION
    );
    assert_selected_provenance(endpoint.sources(), &ENDPOINT_SELECTION);
    assert_eq!(
        selected_ordinals(endpoint.rule()),
        (0..5).collect::<Vec<_>>()
    );

    assert_eq!(selection.complete_free_sources.len(), ORDINARY_ROWS.len());
    assert_complete_provenance(&selection.complete_free_sources);
    let SourceViewConstruction::ResidualProjection(complete_free_evidence) =
        selection.complete_free_sources.construction()
    else {
        panic!("complete free rows must retain authenticated residual routes")
    };
    let intended_bounds = free_face_bounds(i64::MIN + 1, 0);
    let independently_safe = complete_free_evidence
        .term_projections()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, projections)| {
            projections
                .iter()
                .all(|projection| match projection.disposition() {
                    ResidualTermDisposition::Routed {
                        projected_shift, ..
                    } => intended_bounds.iter().zip(projected_shift.iter()).all(
                        |(bounds, &shift)| {
                            bounds.lower().checked_add(shift).is_some()
                                && bounds.upper().checked_add(shift).is_some()
                        },
                    ),
                    ResidualTermDisposition::CoefficientZero
                    | ResidualTermDisposition::ProvedZero { .. } => true,
                })
                .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(independently_safe, MACHINE_SAFE_SELECTION);
    assert_eq!(
        machine_safe_complete_source_ordinals.as_ref(),
        MACHINE_SAFE_SELECTION
    );
    assert_eq!(selection.machine_safe_sources.len(), ORDINARY_ROWS.len());
    assert_selected_provenance(&selection.machine_safe_sources, &MACHINE_SAFE_SELECTION);
    assert_eq!(
        selected_ordinals(&selection.machine_safe_rule),
        BULK_SELECTION
    );
    assert_eq!(
        bulk_selected_complete_source_ordinals.as_ref(),
        BULK_SELECTION
    );
    assert_selected_provenance(bulk.sources(), &BULK_SELECTION);
    assert_eq!(selected_ordinals(bulk.rule()), (0..6).collect::<Vec<_>>());

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for sources in [
        &selection.complete_endpoint_sources,
        endpoint.sources(),
        &selection.complete_free_sources,
        &selection.machine_safe_sources,
        bulk.sources(),
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

    let endpoint_stabilizers = canonicalizer
        .group_elements()
        .enumerate()
        .filter_map(|(ordinal, mapping)| {
            mapping
                .iter()
                .enumerate()
                .all(|(target, &source)| {
                    FACTORIZED_FOUR_LINE_SECTOR[source] == FACTORIZED_FOUR_LINE_SECTOR[target]
                })
                .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(endpoint_stabilizers.len(), 2);
    assert_projection(
        &selection.complete_endpoint_sources,
        FACTORIZED_FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power)),
        &fixed_endpoint(),
        &endpoint_stabilizers,
        9,
    );
    assert_projection(
        endpoint.sources(),
        FACTORIZED_FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power)),
        &fixed_endpoint(),
        &endpoint_stabilizers,
        5,
    );
    assert_projection(
        &selection.complete_free_sources,
        free_face_bounds(i64::MIN + 2, 0),
        &fixed_free_face(),
        &[0],
        9,
    );
    assert_projection(
        &selection.machine_safe_sources,
        free_face_bounds(i64::MIN + 1, 0),
        &fixed_free_face(),
        &[0],
        9,
    );
    assert_projection(
        bulk.sources(),
        free_face_bounds(i64::MIN + 1, -1),
        &fixed_free_face(),
        &[0],
        6,
    );
}

#[test]
fn rules_guards_replay_descent_and_machine_endpoints_are_pinned() {
    let build = derive_factorized_bridge_dot_numerator_build(true).unwrap();
    let selection = build
        .selection_witness
        .as_ref()
        .expect("exact test retains selection evidence");
    assert_rule_metrics(
        &selection.complete_endpoint_rule,
        FACTORIZED_FOUR_LINE_SECTOR,
        &ENDPOINT_RHS,
        (5, 9, 46),
        (5, 23, 2, 26, 0, 70, 13),
    );
    assert_rule_metrics(
        build.endpoint.rule(),
        FACTORIZED_FOUR_LINE_SECTOR,
        &ENDPOINT_RHS,
        (5, 9, 46),
        (5, 23, 2, 26, 0, 70, 13),
    );
    assert_rule_metrics(
        &selection.machine_safe_rule,
        BULK_REPLAY_ANCHOR,
        &MACHINE_SAFE_RHS,
        (6, 24, 96),
        (6, 45, 5, 51, 1, 135, 23),
    );
    assert_rule_metrics(
        build.bulk.rule(),
        BULK_REPLAY_ANCHOR,
        &BULK_RHS,
        (6, 17, 88),
        (6, 44, 4, 49, 0, 129, 23),
    );
    assert!(build.endpoint.guards().is_empty());
    assert!(build.bulk.guards().is_empty());
    assert_eq!(
        build.endpoint.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.bulk.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.endpoint.application_domain().bounds(),
        free_face_bounds(0, 0)
    );
    assert_eq!(build.endpoint.fixed_restrictions(), fixed_endpoint());
    assert_eq!(
        build.bulk.application_domain().bounds(),
        free_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(build.bulk.fixed_restrictions(), fixed_free_face());
    assert!(
        build
            .endpoint
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );
    assert!(
        build
            .bulk
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );

    assert_replay_at(
        &build.context,
        &build.endpoint,
        FACTORIZED_FOUR_LINE_SECTOR,
        (5, 23, 2, 26, 0, 70, 13),
    );
    for free in [-1, -7, i64::MIN + 1] {
        assert_replay_at(
            &build.context,
            &build.bulk,
            [0, free, 1, 1, 1, 1],
            (6, 44, 4, 49, 0, 129, 23),
        );
    }

    let target = |numerator| IntegralKey::try_new([0, numerator, 2, 1, 1, 1]).unwrap();
    assert_eq!(
        build.bulk.assignment_for_target(&target(i64::MIN)).unwrap(),
        Some(vec![0, i64::MIN + 1, 1, 1, 1, 1])
    );
    assert_eq!(
        build.bulk.assignment_for_target(&target(-2)).unwrap(),
        Some(vec![0, -1, 1, 1, 1, 1])
    );
    assert!(
        build
            .bulk
            .assignment_for_target(&target(-1))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        build.endpoint.assignment_for_target(&target(-1)).unwrap(),
        Some(FACTORIZED_FOUR_LINE_SECTOR.to_vec())
    );
    assert!(
        build
            .endpoint
            .assignment_for_target(&target(-2))
            .unwrap()
            .is_none()
    );
    assert!(
        build
            .endpoint
            .assignment_for_target(&target(0))
            .unwrap()
            .is_none()
    );
}

#[test]
fn factorization_and_canonical_descendants_keep_the_frontier_honest() {
    let (_context, endpoint, bulk) = derive_factorized_bridge_dot_numerator_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();

    let endpoint_children =
        canonical_children(&canonicalizer, &endpoint, &FACTORIZED_FOUR_LINE_SECTOR);
    assert_eq!(
        endpoint_children,
        [vec![0, 0, 1, 0, 2, 1], vec![0, 0, 1, 1, 1, 1]]
    );
    for (child, owner) in endpoint_children.iter().zip([2, 0]) {
        assert!(matches!(
            terminals.classify(&key(child)),
            Some(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == owner
        ));
    }

    for free in [-1, -7, i64::MIN + 1] {
        let children = canonical_children(&canonicalizer, &bulk, &[0, free, 1, 1, 1, 1]);
        assert_eq!(
            children,
            [
                vec![free, 0, 1, 0, 2, 1],
                vec![0, free, 1, 1, 1, 1],
                vec![0, free + 1, 1, 1, 1, 1],
                vec![free + 1, 0, 1, 0, 1, 1],
            ]
        );
        assert!(terminals.classify(&key(&children[0])).is_none());
        assert!(terminals.classify(&key(&children[1])).is_none());
        if free == -1 {
            for (child, owner) in children[2..].iter().zip([0, 2]) {
                assert!(matches!(
                    terminals.classify(&key(child)),
                    Some(terminal)
                        if terminal.kind() == ReachabilityTerminalKind::Factorization
                            && terminal.owner_ordinal() == owner
                ));
            }
        } else {
            assert!(terminals.classify(&key(&children[2])).is_none());
            assert!(terminals.classify(&key(&children[3])).is_none());
        }
    }
}

#[test]
fn exact_s4_boundary_owns_only_the_bridge_dot_orbit() {
    let (_context, endpoint, bulk) = derive_factorized_bridge_dot_numerator_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    assert_eq!(
        canonicalizer
            .orbit(&IntegralKey::try_new(FACTORIZED_FOUR_LINE_SECTOR).unwrap())
            .unwrap()
            .orbit_size(),
        12
    );

    for free in [-1, -7] {
        let owned = IntegralKey::try_new([0, free, 2, 1, 1, 1]).unwrap();
        let equivalent = IntegralKey::try_new([free, 0, 2, 1, 1, 1]).unwrap();
        let owned_canonical = canonicalizer.canonicalize(&owned).unwrap();
        assert_eq!(owned_canonical.canonical(), &owned);
        assert_eq!(
            canonicalizer.canonicalize(&equivalent).unwrap().canonical(),
            &owned
        );
        assert_eq!(canonicalizer.orbit(&owned).unwrap().orbit_size(), 24);

        let expected_representatives = BTreeSet::from([
            vec![0, free, 2, 1, 1, 1],
            vec![0, free, 1, 1, 2, 1],
            vec![0, free, 1, 2, 1, 1],
            vec![0, free, 1, 1, 1, 2],
        ]);
        let mut observed_representatives = BTreeSet::new();
        for numerator_slot in [0, 1] {
            for dot_slot in [2, 3, 4, 5] {
                let mut powers = FACTORIZED_FOUR_LINE_SECTOR;
                powers[numerator_slot] = free;
                powers[dot_slot] = 2;
                observed_representatives.insert(
                    canonicalizer
                        .canonicalize(&IntegralKey::try_new(powers).unwrap())
                        .unwrap()
                        .canonical()
                        .powers()
                        .to_vec(),
                );
            }
        }
        assert_eq!(observed_representatives, expected_representatives);

        for alternate in expected_representatives
            .iter()
            .filter(|representative| representative.as_slice() != owned.powers())
        {
            let alternate = key(alternate);
            let orbit = canonicalizer.orbit(&alternate).unwrap();
            assert_eq!(orbit.orbit_size(), 24);
            assert_eq!(orbit.canonical().integral(), &alternate);
            assert!(
                endpoint
                    .assignment_for_target(&alternate)
                    .unwrap()
                    .is_none()
            );
            assert!(bulk.assignment_for_target(&alternate).unwrap().is_none());
        }
    }
}

fn assert_complete_provenance(sources: &SourceViewBatch) {
    assert_eq!(sources.len(), ORDINARY_ROWS.len());
    for (provenance, row) in sources.provenance().iter().zip(ORDINARY_ROWS) {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(provenance.translated().source_row().stable_string(), row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_selected_provenance(sources: &SourceViewBatch, complete_ordinals: &[usize]) {
    assert_eq!(sources.len(), complete_ordinals.len());
    for (provenance, &ordinal) in sources.provenance().iter().zip(complete_ordinals) {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_projection(
    sources: &SourceViewBatch,
    bounds: [InteriorBounds; 6],
    fixed: &[FixedIndexRestriction],
    stabilizers: &[usize],
    original_rows: usize,
) {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        panic!("generated sources must retain residual projection evidence")
    };
    assert_eq!(evidence.domain().bounds(), bounds);
    assert_eq!(evidence.fixed_restrictions(), fixed);
    assert_eq!(evidence.stabilizer_group_elements(), stabilizers);
    assert_eq!(evidence.original_relations().len(), original_rows);
    assert_eq!(evidence.term_projections().len(), original_rows);
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn assert_rule_metrics(
    rule: &ParametricRule,
    anchor: [i64; 6],
    rhs: &[[i64; 6]],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), anchor);
    assert_eq!(rule.pivot().values(), BRIDGE_DOT_NUMERATOR_PIVOT);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        rhs.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
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
    assert_eq!(replay.source_contributions_checked(), metrics.0);
    assert_eq!(replay.source_terms_checked(), metrics.1);
    assert_eq!(replay.right_hand_side_terms_checked(), metrics.2);
    assert_eq!(replay.integral_keys_checked(), metrics.3);
    assert_eq!(replay.nonzero_guards_checked(), metrics.4);
    assert_eq!(replay.exact_operations(), metrics.5);
    assert_eq!(replay.peak_retained_coefficient_terms(), metrics.6);
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

fn free_face_bounds(lower: i64, upper: i64) -> [InteriorBounds; 6] {
    std::array::from_fn(|position| {
        if position == FREE_POSITION {
            InteriorBounds::new(lower, upper)
        } else {
            InteriorBounds::new(
                FACTORIZED_FOUR_LINE_SECTOR[position],
                FACTORIZED_FOUR_LINE_SECTOR[position],
            )
        }
    })
}
