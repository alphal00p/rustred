use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleError, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::sector::InteriorBounds;

use super::super::super::super::{canonical_family, canonical_s4};
use super::super::FOUR_LINE_SECTOR;
use super::opposite_inactive_numerator_pair_bulk::{
    FREE_POSITION, OPPOSITE_PAIR_BULK_PIVOT, OPPOSITE_PAIR_BULK_REPLAY_ANCHOR,
    OppositePairBulkBuild, derive_opposite_inactive_numerator_pair_bulk,
    derive_opposite_pair_bulk_build, derive_opposite_pair_bulk_candidate, fixed_source_face,
    opposite_pair_bulk_search_depth,
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
const COMPLETE_SELECTION: [usize; 5] = [9, 13, 18, 21, 22];
const MACHINE_SAFE_SELECTION: [usize; 59] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
    30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 59, 60, 61, 62,
];
const RHS: [[i64; 6]; 5] = [
    [0, 0, 0, 0, 0, 0],
    [0, -1, 1, 0, 0, 0],
    [1, 0, 0, 0, 0, -1],
    [0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0],
];

#[test]
fn complete_depth_one_search_safe_selection_and_compact_reprojection_are_exact() {
    assert_eq!(
        derive_opposite_pair_bulk_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    );
    let OppositePairBulkBuild {
        context: _,
        bulk,
        machine_safe_complete_source_ordinals,
        selected_complete_source_ordinals,
        selection_witness,
    } = derive_opposite_pair_bulk_build(true).unwrap();
    let witness = selection_witness.expect("test retains complete generated evidence");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        opposite_pair_bulk_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
    assert_eq!(opposite_pair_bulk_search_depth(), 1);
    assert_eq!(search.offset_count(), 7);
    assert_eq!(witness.complete_sources.len(), 63);
    assert_complete_provenance(&witness.complete_sources, &search);
    assert_eq!(
        selected_ordinals(&witness.complete_rule),
        COMPLETE_SELECTION
    );
    assert_eq!(
        selected_complete_source_ordinals.as_ref(),
        COMPLETE_SELECTION
    );
    assert_eq!(
        machine_safe_complete_source_ordinals.as_ref(),
        MACHINE_SAFE_SELECTION
    );
    assert!(
        COMPLETE_SELECTION
            .iter()
            .all(|ordinal| machine_safe_complete_source_ordinals.contains(ordinal))
    );
    assert_eq!(
        witness.machine_safe_sources.len(),
        machine_safe_complete_source_ordinals.len()
    );
    assert_selected_provenance(
        &witness.machine_safe_sources,
        &machine_safe_complete_source_ordinals,
        &search,
    );
    assert_eq!(
        witness
            .machine_safe_rule
            .source_combination()
            .iter()
            .map(|source| { machine_safe_complete_source_ordinals[source.source_ordinal()] })
            .collect::<Vec<_>>(),
        COMPLETE_SELECTION,
    );
    assert_selected_provenance(bulk.sources(), &COMPLETE_SELECTION, &search);
    assert_eq!(selected_ordinals(bulk.rule()), [0, 1, 2, 3, 4]);
}

#[test]
fn coefficients_guards_replay_domain_descent_and_machine_endpoints_are_exact() {
    let build = derive_opposite_pair_bulk_build(true).unwrap();
    let witness = build.selection_witness.as_ref().unwrap();
    assert_rule(&build.context, build.bulk.rule());
    for rule in [&witness.complete_rule, &witness.machine_safe_rule] {
        assert_eq!(
            rule.source_combination()
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
            rule.right_hand_side()
                .iter()
                .map(|term| (term.shift(), term.coefficient()))
                .collect::<Vec<_>>(),
            build
                .bulk
                .rule()
                .right_hand_side()
                .iter()
                .map(|term| (term.shift(), term.coefficient()))
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(build.bulk.guards().len(), 2);
    assert_eq!(
        build.bulk.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(
        build.bulk.application_domain().bounds(),
        source_bounds(i64::MIN + 1)
    );
    assert_eq!(build.bulk.fixed_restrictions(), fixed_source_face());
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
            &[-1, 1, 1, 1, 1, free],
            Default::default(),
        )
        .unwrap();
        let metrics = concrete_metrics(&replay);
        assert_eq!(
            (
                metrics.0, metrics.1, metrics.2, metrics.3, metrics.4, metrics.5
            ),
            (5, 27, 5, 33, 2, 92)
        );
        assert!(metrics.6 >= 19);
    }
    assert_eq!(
        build
            .bulk
            .assignment_for_target(&key([-1, 1, 1, 1, 1, i64::MIN]))
            .unwrap(),
        Some(vec![-1, 1, 1, 1, 1, i64::MIN + 1]),
    );
    assert_eq!(
        build
            .bulk
            .assignment_for_target(&key([-1, 1, 1, 1, 1, -2]))
            .unwrap(),
        Some(vec![-1, 1, 1, 1, 1, -1]),
    );
    for outside in [
        [-1, 1, 1, 1, 1, -1],
        [-2, 1, 1, 1, 1, -2],
        [-1, 1, 1, 1, 2, -2],
    ] {
        assert!(
            build
                .bulk
                .assignment_for_target(&key(outside))
                .unwrap()
                .is_none()
        );
    }
    let (_context, rebuilt) = derive_opposite_inactive_numerator_pair_bulk().unwrap();
    assert_eq!(rebuilt.rule(), build.bulk.rule());
    assert_eq!(
        rebuilt.sources().relations(),
        build.bulk.sources().relations()
    );
    assert_eq!(
        rebuilt.application_domain(),
        build.bulk.application_domain()
    );
}

#[test]
fn exact_s4_owned_orbit_and_endpoint_children_are_pinned() {
    let (_context, bulk) = derive_opposite_inactive_numerator_pair_bulk().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    for free in [-2, -7, i64::MIN] {
        let target = key([-1, 1, 1, 1, 1, free]);
        let orbit = canonicalizer.orbit(&target).unwrap();
        assert_eq!(orbit.group_order(), 24);
        assert_eq!(orbit.orbit_size(), 6);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == 4)
        );
        assert_eq!(orbit.canonical().integral(), &target);
        assert!(bulk.assignment_for_target(&target).unwrap().is_some());
        for unowned in [
            [free, -1, 1, 1, 1, 1],
            [-1, 1, 1, 1, 2, free],
            [free, 1, 1, 1, 2, -1],
        ] {
            let canonical = canonicalizer.canonicalize(&key(unowned)).unwrap();
            assert!(
                bulk.assignment_for_target(canonical.canonical())
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert!(
        bulk.assignment_for_target(&key([-1, 1, 1, 1, 1, -1]))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        canonical_children(&canonicalizer, &bulk, &[-1, 1, 1, 1, 1, -1]),
        [
            vec![-1, 1, 1, 1, 1, -1],
            vec![0, -1, 1, -1, 2, 1],
            vec![0, 1, 1, 1, 1, -2],
            vec![0, 1, 1, 1, 1, -1],
            vec![0, 1, 1, 1, 1, -1],
        ],
    );
}

fn assert_rule(context: &crate::algebra::IndexedCoefficientContext, rule: &ParametricRule) {
    assert_eq!(rule.anchor().powers(), OPPOSITE_PAIR_BULK_REPLAY_ANCHOR);
    assert_eq!(rule.pivot().values(), OPPOSITE_PAIR_BULK_PIVOT);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>(),
    );
    let d = context
        .lift(&context.base().coefficient_fixture("d"))
        .unwrap();
    let n = context.index(FREE_POSITION).unwrap();
    let q = context
        .sub(
            &context
                .sub(
                    &context.mul(&context.integer(3), &d).unwrap(),
                    &context.mul(&context.integer(2), &n).unwrap(),
                )
                .unwrap(),
            &context.integer(4),
        )
        .unwrap();
    let reciprocal = context.div(&context.one(), &q).unwrap();
    let scale = |factor| context.mul(&context.integer(factor), &reciprocal).unwrap();
    assert_eq!(
        rule.source_combination()
            .iter()
            .map(|source| source.coefficient())
            .collect::<Vec<_>>(),
        [&reciprocal, &scale(2), &scale(5), &scale(-8), &scale(-6)],
    );
    let pivot = context
        .div(
            &context.mul(&context.integer(-1), &q).unwrap(),
            &context.integer(6),
        )
        .unwrap();
    assert_eq!(rule.pivot_guard().coefficient(), &pivot);
    let first = context
        .mul(
            &context
                .sub(
                    &context.sub(&d, &context.integer(4)).unwrap(),
                    &context.mul(&context.integer(4), &n).unwrap(),
                )
                .unwrap(),
            &reciprocal,
        )
        .unwrap();
    let fourth = context
        .mul(&context.mul(&context.integer(-6), &n).unwrap(), &reciprocal)
        .unwrap();
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.coefficient())
            .collect::<Vec<_>>(),
        [&first, &scale(8), &scale(-2), &fourth, &scale(-2)],
    );
    assert_eq!(rule.replay().source_rows_used(), 5);
    assert_eq!(rule.replay().shift_columns_checked(), 12);
    assert_eq!(rule.replay().exact_operations(), 54);
    assert_eq!(
        concrete_metrics(rule.concrete_replay()),
        (5, 27, 5, 33, 2, 92, 19)
    );
}

fn assert_complete_provenance(sources: &SourceViewBatch, search: &SectorSearchDiamond) {
    assert_eq!(sources.len(), search.offset_count() * ORDINARY_ROWS.len());
    for (ordinal, source) in sources.provenance().iter().enumerate() {
        assert_eq!(source.translated().offset(), &search.offsets()[ordinal / 9]);
        assert_eq!(
            source.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal % 9]
        );
    }
}

fn assert_selected_provenance(
    sources: &SourceViewBatch,
    complete_ordinals: &[usize],
    search: &SectorSearchDiamond,
) {
    assert_eq!(sources.len(), complete_ordinals.len());
    for (source, &ordinal) in sources.provenance().iter().zip(complete_ordinals) {
        assert_eq!(source.translated().offset(), &search.offsets()[ordinal / 9]);
        assert_eq!(
            source.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal % 9]
        );
    }
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|source| source.source_ordinal())
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

fn key(powers: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(-1, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, -1),
    ]
}
