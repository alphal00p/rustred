use std::collections::BTreeSet;

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
use super::FACTORIZED_FACE_SECTOR;
use super::inactive_numerator_endpoint::{
    FACTORIZED_FACE_NUMERATOR_PIVOT, FactorizedFaceNumeratorEndpointBuild,
    derive_factorized_face_numerator_candidate, derive_factorized_face_numerator_endpoint,
    derive_factorized_face_numerator_endpoint_build, factorized_face_numerator_search_depth,
    fixed_endpoint,
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
const COMPLETE_SELECTION: [usize; 8] = [6, 7, 8, 18, 19, 20, 24, 26];
const TARGET: [i64; 6] = [0, -1, 1, 1, 1, 1];
const RHS: [[i64; 6]; 3] = [[0, 0, 1, -1, 0, 0], [0, 0, 0, 0, 0, 0], [0, 0, 0, -1, 0, 0]];

#[test]
fn complete_depth_one_selection_and_compact_reprojection_are_exact() {
    assert_eq!(
        derive_factorized_face_numerator_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    let FactorizedFaceNumeratorEndpointBuild {
        context,
        endpoint,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_factorized_face_numerator_endpoint_build(true).unwrap();
    let witness = selection_witness.expect("exact test retains the complete span");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FACTORIZED_FACE_SECTOR).unwrap(),
        factorized_face_numerator_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(factorized_face_numerator_search_depth(), 1);
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
        (0..8).collect::<Vec<_>>()
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
            panic!("generated endpoint sources must retain residual routes")
        };
        assert_eq!(
            evidence.domain().bounds(),
            FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power))
        );
        assert_eq!(evidence.fixed_restrictions(), fixed_endpoint());
        assert_eq!(evidence.original_relations().len(), original_rows);
        assert_eq!(evidence.term_projections().len(), original_rows);
        assert_eq!(evidence.stabilizer_group_elements().len(), 2);
    }
}

#[test]
fn exact_coefficients_guard_replay_domain_and_rebuild_are_pinned() {
    let build = derive_factorized_face_numerator_endpoint_build(true).unwrap();
    let witness = build
        .selection_witness
        .as_ref()
        .expect("exact test retains the complete span");
    assert_rule_metrics(&witness.complete_rule, (8, 51, 123));
    assert_rule_metrics(build.endpoint.rule(), (8, 18, 90));
    assert_eq!(
        witness.complete_rule.pivot_guard().coefficient(),
        build.endpoint.rule().pivot_guard().coefficient()
    );
    assert_eq!(
        witness.complete_rule.pivot_guard().nonzero_polynomial(),
        build.endpoint.rule().pivot_guard().nonzero_polynomial()
    );
    assert_eq!(
        witness
            .complete_rule
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        build
            .endpoint
            .rule()
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        witness
            .complete_rule
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        build
            .endpoint
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        witness
            .complete_rule
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>(),
        build
            .endpoint
            .rule()
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>()
    );

    let reciprocal = build
        .context
        .lift(&build.context.base().coefficient_fixture("1/(d-1)"))
        .unwrap();
    let twice_reciprocal = build
        .context
        .lift(&build.context.base().coefficient_fixture("2/(d-1)"))
        .unwrap();
    let minus_twice_reciprocal = build
        .context
        .lift(&build.context.base().coefficient_fixture("-2/(d-1)"))
        .unwrap();
    assert_eq!(
        build
            .endpoint
            .rule()
            .source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        [
            &reciprocal,
            &reciprocal,
            &reciprocal,
            &minus_twice_reciprocal,
            &minus_twice_reciprocal,
            &minus_twice_reciprocal,
            &twice_reciprocal,
            &reciprocal,
        ]
    );
    assert_eq!(
        build
            .endpoint
            .rule()
            .right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        [&twice_reciprocal, &build.context.one(), &reciprocal]
    );
    let guard = build
        .context
        .denominator_condition_with_limits(&reciprocal, Default::default())
        .unwrap();
    let dimension_minus_one = build
        .context
        .lift(&build.context.base().coefficient_fixture("d-1"))
        .unwrap();
    assert_eq!(
        build.endpoint.rule().pivot_guard().coefficient(),
        &dimension_minus_one
    );
    assert_eq!(
        build.endpoint.rule().pivot_guard().nonzero_polynomial(),
        &guard
    );
    assert_eq!(build.endpoint.rule().nonzero_guards().len(), 1);
    assert_eq!(
        build.endpoint.rule().nonzero_guards()[0].polynomial(),
        &guard
    );
    assert_eq!(build.endpoint.guards().len(), 1);
    assert_eq!(build.endpoint.guards()[0].polynomial(), &guard);

    assert_eq!(
        build.endpoint.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.endpoint.application_domain().bounds(),
        FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power))
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
        Some(FACTORIZED_FACE_SECTOR.to_vec())
    );
    for unowned in [
        [0, -2, 1, 1, 1, 1],
        [0, i64::MIN, 1, 1, 1, 1],
        [0, -1, 1, 1, 2, 1],
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
        build.endpoint.rule(),
        &FACTORIZED_FACE_SECTOR,
        Default::default(),
    )
    .unwrap();
    assert_eq!(held_out, build.endpoint.rule().concrete_replay().clone());

    let (_second_context, second) = derive_factorized_face_numerator_endpoint().unwrap();
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
fn exhaustive_inactive_placement_orbit_and_children_are_exact() {
    let (_context, endpoint) = derive_factorized_face_numerator_endpoint().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let target = IntegralKey::try_new(TARGET).unwrap();
    let target_orbit = canonicalizer.orbit(&target).unwrap();
    assert_eq!(target_orbit.group_order(), 24);
    assert_eq!(target_orbit.orbit_size(), 24);
    assert!(
        target_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 1)
    );
    assert_eq!(target_orbit.canonical().integral(), &target);

    let base = IntegralKey::try_new(FACTORIZED_FACE_SECTOR).unwrap();
    let base_orbit = canonicalizer.orbit(&base).unwrap();
    assert_eq!(base_orbit.orbit_size(), 12);
    assert!(
        base_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 2)
    );
    let mut exhaustive_placements = BTreeSet::new();
    for image in base_orbit.images() {
        for inactive in image
            .integral()
            .powers()
            .iter()
            .enumerate()
            .filter_map(|(position, &power)| (power == 0).then_some(position))
        {
            let mut powers: [i64; 6] = image.integral().powers().try_into().unwrap();
            powers[inactive] = -1;
            let decorated = IntegralKey::try_new(powers).unwrap();
            assert_eq!(
                canonicalizer.canonicalize(&decorated).unwrap().canonical(),
                &target
            );
            exhaustive_placements.insert(decorated);
        }
    }
    assert_eq!(exhaustive_placements.len(), 24);
    assert_eq!(
        exhaustive_placements,
        target_orbit
            .images()
            .iter()
            .map(|image| image.integral().clone())
            .collect()
    );

    let children = canonical_children(&canonicalizer, &endpoint, &FACTORIZED_FACE_SECTOR);
    assert_eq!(
        children,
        [
            vec![0, 0, 1, 0, 2, 1],
            vec![0, 0, 1, 1, 1, 1],
            vec![0, 0, 1, 0, 1, 1],
        ]
    );
    for (child, owner) in children.iter().zip([2, 0, 2]) {
        assert!(matches!(
            terminals.classify(&key(child)),
            Some(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == owner
        ));
    }
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

fn assert_rule_metrics(rule: &ParametricRule, parametric: (usize, usize, usize)) {
    assert_eq!(rule.anchor().powers(), FACTORIZED_FACE_SECTOR);
    assert_eq!(rule.pivot().values(), FACTORIZED_FACE_NUMERATOR_PIVOT);
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
    assert_eq!(replay.source_contributions_checked(), 8);
    assert_eq!(replay.source_terms_checked(), 45);
    assert_eq!(replay.right_hand_side_terms_checked(), 3);
    assert_eq!(replay.integral_keys_checked(), 49);
    assert_eq!(replay.nonzero_guards_checked(), 1);
    assert_eq!(replay.exact_operations(), 133);
    assert_eq!(replay.peak_retained_coefficient_terms(), 30);
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
