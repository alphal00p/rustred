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
use super::factorized_dot_numerator_rays::{
    FREE_POSITION, FactorizedThreeLineDotNumeratorBuild, PATH_MIDDLE_DOT_NUMERATOR_PIVOT,
    PATH_SOURCE_SECTOR, STAR_SOURCE_SECTOR, STAR_SPOKE_DOT_NUMERATOR_PIVOT,
    derive_complete_path_candidate, derive_complete_star_candidate,
    derive_factorized_three_line_dot_numerator_build,
    derive_factorized_three_line_dot_numerator_rays, fixed_source, search_depth,
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
const PATH_SELECTION: [usize; 2] = [0, 1];
const STAR_SELECTION: [usize; 2] = [4, 7];
const PATH_TARGET: [i64; 6] = [0, 0, 1, -1, 1, 2];
const STAR_TARGET: [i64; 6] = [0, 0, 1, 1, -1, 2];
const RHS: [[i64; 6]; 1] = [[0; 6]];

#[test]
fn complete_depth_zero_selection_and_independent_reprojection_are_exact() {
    let FactorizedThreeLineDotNumeratorBuild {
        context,
        path_middle_ray,
        star_spoke_ray,
        path_selected_complete_source_ordinals,
        star_selected_complete_source_ordinals,
        selection_witness,
    } = derive_factorized_three_line_dot_numerator_build(true).unwrap();
    let witness = selection_witness.expect("exact tests retain complete spans");
    assert_eq!(search_depth(), 0);
    let path_search = search(PATH_SOURCE_SECTOR);
    let star_search = search(STAR_SOURCE_SECTOR);
    for search in [&path_search, &star_search] {
        assert_eq!(search.offset_count(), 1);
        assert_eq!(search.offsets()[0].values(), [0; 6]);
    }
    assert_eq!(witness.complete_path_sources.len(), ORDINARY_ROWS.len());
    assert_eq!(witness.complete_star_sources.len(), ORDINARY_ROWS.len());
    assert_complete_provenance(&witness.complete_path_sources);
    assert_complete_provenance(&witness.complete_star_sources);
    assert_eq!(
        selected_ordinals(&witness.complete_path_rule),
        PATH_SELECTION
    );
    assert_eq!(
        path_selected_complete_source_ordinals.as_ref(),
        PATH_SELECTION
    );
    assert_eq!(
        selected_ordinals(&witness.complete_star_rule),
        STAR_SELECTION
    );
    assert_eq!(
        star_selected_complete_source_ordinals.as_ref(),
        STAR_SELECTION
    );
    assert_selected_provenance(path_middle_ray.sources(), &PATH_SELECTION);
    assert_selected_provenance(star_spoke_ray.sources(), &STAR_SELECTION);
    assert_eq!(selected_ordinals(path_middle_ray.rule()), [0, 1]);
    assert_eq!(selected_ordinals(star_spoke_ray.rule()), [0, 1]);

    // Re-running the complete generic calls independently gives the exact
    // same complete rules retained by the coherent build.
    assert_eq!(
        derive_complete_path_candidate().unwrap(),
        witness.complete_path_rule
    );
    assert_eq!(
        derive_complete_star_candidate().unwrap(),
        witness.complete_star_rule
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for sources in [
        &witness.complete_path_sources,
        path_middle_ray.sources(),
        &witness.complete_star_sources,
        star_spoke_ray.sources(),
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
        &witness.complete_path_sources,
        PATH_SOURCE_SECTOR,
        ORDINARY_ROWS.len(),
    );
    assert_projection(
        path_middle_ray.sources(),
        PATH_SOURCE_SECTOR,
        PATH_SELECTION.len(),
    );
    assert_projection(
        &witness.complete_star_sources,
        STAR_SOURCE_SECTOR,
        ORDINARY_ROWS.len(),
    );
    assert_projection(
        star_spoke_ray.sources(),
        STAR_SOURCE_SECTOR,
        STAR_SELECTION.len(),
    );
}

#[test]
fn exact_coefficients_guards_replay_domains_and_machine_boundary_are_pinned() {
    let build = derive_factorized_three_line_dot_numerator_build(true).unwrap();
    let witness = build
        .selection_witness
        .as_ref()
        .expect("exact tests retain complete spans");
    assert_rule(
        &witness.complete_path_rule,
        PATH_SOURCE_SECTOR,
        &PATH_MIDDLE_DOT_NUMERATOR_PIVOT,
        (2, 8, 21),
        (2, 9, 1, 11, 2, 30, 9),
    );
    assert_rule(
        build.path_middle_ray.rule(),
        PATH_SOURCE_SECTOR,
        &PATH_MIDDLE_DOT_NUMERATOR_PIVOT,
        (2, 5, 18),
        (2, 9, 1, 11, 2, 30, 9),
    );
    assert_rule(
        &witness.complete_star_rule,
        STAR_SOURCE_SECTOR,
        &STAR_SPOKE_DOT_NUMERATOR_PIVOT,
        (2, 7, 16),
        (2, 6, 1, 8, 2, 23, 7),
    );
    assert_rule(
        build.star_spoke_ray.rule(),
        STAR_SOURCE_SECTOR,
        &STAR_SPOKE_DOT_NUMERATOR_PIVOT,
        (2, 3, 12),
        (2, 6, 1, 8, 2, 23, 7),
    );
    assert_complete_compact_equivalence(&witness.complete_path_rule, build.path_middle_ray.rule());
    assert_complete_compact_equivalence(&witness.complete_star_rule, build.star_spoke_ray.rule());

    assert_eq!(
        source_coefficient_expressions(build.path_middle_ray.rule()),
        ["-1/2/n5", "-1/2/n5"]
    );
    assert_eq!(
        coefficient_expression(build.path_middle_ray.rule().pivot_guard().coefficient()),
        "-2*n5"
    );
    assert_eq!(
        guard_expressions(build.path_middle_ray.rule()),
        ["-2*n5", "2*n5"]
    );
    assert_eq!(
        rhs_coefficient_expressions(build.path_middle_ray.rule()),
        ["1/2*d/n5"]
    );
    assert_eq!(
        source_coefficient_expressions(build.star_spoke_ray.rule()),
        ["-1/2/n5", "-1/2/n5"]
    );
    assert_eq!(
        coefficient_expression(build.star_spoke_ray.rule().pivot_guard().coefficient()),
        "2*n5"
    );
    assert_eq!(
        guard_expressions(build.star_spoke_ray.rule()),
        ["-n5", "2*n5"]
    );
    assert_eq!(
        rhs_coefficient_expressions(build.star_spoke_ray.rule()),
        ["1/2*d/n5"]
    );
    assert_eq!(
        cell_guard_expressions(&build.path_middle_ray),
        ["-2*n5", "2*n5"]
    );
    assert_eq!(
        cell_guard_expressions(&build.star_spoke_ray),
        ["-n5", "2*n5"]
    );

    for (cell, sector, target, metrics) in [
        (
            &build.path_middle_ray,
            PATH_SOURCE_SECTOR,
            PATH_TARGET,
            (2, 9, 1, 11, 2, 30, 9),
        ),
        (
            &build.star_spoke_ray,
            STAR_SOURCE_SECTOR,
            STAR_TARGET,
            (2, 6, 1, 8, 2, 23, 7),
        ),
    ] {
        assert!(cell.pruned_rhs_ordinals().is_empty());
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert_eq!(
            cell.application_domain().bounds(),
            application_bounds(sector)
        );
        assert_eq!(cell.fixed_restrictions(), fixed_source(sector));
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
        assert_eq!(
            cell.assignment_for_target(&key(&target)).unwrap(),
            Some(sector.to_vec())
        );
        let mut maximum_target = target;
        maximum_target[FREE_POSITION] = i64::MAX;
        let mut maximum_source = sector;
        maximum_source[FREE_POSITION] = i64::MAX - 1;
        assert_eq!(
            cell.assignment_for_target(&key(&maximum_target)).unwrap(),
            Some(maximum_source.to_vec())
        );
        let mut boundary = target;
        boundary[FREE_POSITION] = 1;
        assert!(
            cell.assignment_for_target(&key(&boundary))
                .unwrap()
                .is_none()
        );
        for source_power in [1, 7, i64::MAX - 1] {
            let mut assignment = sector;
            assignment[FREE_POSITION] = source_power;
            assert_replay_at(&build.context, cell, assignment, metrics);
        }
    }

    let (_context, second_path, second_star) =
        derive_factorized_three_line_dot_numerator_rays().unwrap();
    for (first, second) in [
        (&build.path_middle_ray, &second_path),
        (&build.star_spoke_ray, &second_star),
    ] {
        assert_eq!(first.rule(), second.rule());
        assert_eq!(first.application_domain(), second.application_domain());
        assert_eq!(first.fixed_restrictions(), second.fixed_restrictions());
        assert_eq!(first.guards(), second.guards());
    }
}

#[test]
fn exhaustive_s4_geometry_nonownership_and_factorization_children_are_exact() {
    let (_context, path, star) = derive_factorized_three_line_dot_numerator_rays().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();

    let mut path_classes = BTreeMap::<Vec<i64>, BTreeSet<(usize, usize)>>::new();
    for numerator in [0, 1, 3] {
        for dot in [2, 4, 5] {
            let mut powers = PATH_SOURCE_SECTOR;
            powers[numerator] = -1;
            powers[dot] = 2;
            path_classes
                .entry(canonical(&canonicalizer, powers))
                .or_default()
                .insert((numerator, dot));
        }
    }
    assert_eq!(
        path_classes,
        [
            (vec![-1, 0, 1, 0, 1, 2], [(0, 5)].into_iter().collect()),
            (
                vec![-1, 0, 1, 0, 2, 1],
                [(0, 2), (0, 4)].into_iter().collect(),
            ),
            (PATH_TARGET.to_vec(), [(1, 5), (3, 5)].into_iter().collect(),),
            (
                vec![0, 0, 1, -1, 2, 1],
                [(1, 2), (3, 4)].into_iter().collect(),
            ),
            (
                vec![0, 0, 2, -1, 1, 1],
                [(1, 4), (3, 2)].into_iter().collect(),
            ),
        ]
        .into_iter()
        .collect()
    );
    for representative in path_classes.keys() {
        let target = key(representative);
        let orbit = canonicalizer.orbit(&target).unwrap();
        let (size, multiplicity) = if representative.as_slice() == [-1, 0, 1, 0, 1, 2] {
            (12, 2)
        } else {
            (24, 1)
        };
        assert_eq!(orbit.orbit_size(), size);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == multiplicity)
        );
        assert_eq!(orbit.canonical().integral(), &target);
        assert_eq!(
            path.assignment_for_target(&target).unwrap().is_some(),
            representative.as_slice() == PATH_TARGET
        );
    }

    let mut star_classes = BTreeMap::<Vec<i64>, BTreeSet<(usize, usize)>>::new();
    for numerator in [0, 1, 4] {
        for dot in [2, 3, 5] {
            let mut powers = STAR_SOURCE_SECTOR;
            powers[numerator] = -1;
            powers[dot] = 2;
            star_classes
                .entry(canonical(&canonicalizer, powers))
                .or_default()
                .insert((numerator, dot));
        }
    }
    assert_eq!(
        star_classes.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([STAR_TARGET.to_vec(), vec![0, 0, 2, 1, -1, 1]])
    );
    assert_eq!(star_classes.values().map(BTreeSet::len).sum::<usize>(), 9);
    for representative in star_classes.keys() {
        let target = key(representative);
        let orbit = canonicalizer.orbit(&target).unwrap();
        let (size, multiplicity) = if representative.as_slice() == STAR_TARGET {
            (24, 1)
        } else {
            (12, 2)
        };
        assert_eq!(orbit.orbit_size(), size);
        assert!(
            orbit
                .images()
                .iter()
                .all(|image| image.routing_multiplicity() == multiplicity)
        );
        assert_eq!(orbit.canonical().integral(), &target);
        assert_eq!(
            star.assignment_for_target(&target).unwrap().is_some(),
            representative.as_slice() == STAR_TARGET
        );
    }

    for power in [2, 7] {
        let path_owned = [0, 0, 1, -1, 1, power];
        let star_owned = [0, 0, 1, 1, -1, power];
        assert!(
            path.assignment_for_target(&key(&path_owned))
                .unwrap()
                .is_some()
        );
        assert!(
            star.assignment_for_target(&key(&star_owned))
                .unwrap()
                .is_some()
        );
        for unowned in [
            [0, 0, 1, -1, power, 1],
            [0, 0, power, -1, 1, 1],
            [0, 0, power, 1, -1, 1],
        ] {
            assert!(
                path.assignment_for_target(&key(&unowned))
                    .unwrap()
                    .is_none()
            );
            assert!(
                star.assignment_for_target(&key(&unowned))
                    .unwrap()
                    .is_none()
            );
        }
    }

    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    for (cell, source_sector, owner) in [
        (&path, PATH_SOURCE_SECTOR, 2),
        (&star, STAR_SOURCE_SECTOR, 1),
    ] {
        for source_power in [1, 7, i64::MAX - 1] {
            let mut assignment = source_sector;
            assignment[FREE_POSITION] = source_power;
            let children = canonical_children(&canonicalizer, cell, &assignment);
            assert_eq!(children.len(), 1);
            let mut expected = source_sector;
            expected[FREE_POSITION] = source_power;
            assert_eq!(children[0], expected.to_vec());
            assert!(matches!(
                terminals.classify(&key(&children[0])),
                Some(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }
    }
}

fn search(sector: [i64; 6]) -> SectorSearchDiamond {
    SectorSearchDiamond::try_new(
        IntegralKey::try_new(sector).unwrap(),
        search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap()
}

fn assert_complete_provenance(sources: &crate::foundry::cell::SourceViewBatch) {
    assert_eq!(sources.len(), ORDINARY_ROWS.len());
    for (provenance, row) in sources.provenance().iter().zip(ORDINARY_ROWS) {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(provenance.translated().source_row().stable_string(), row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_selected_provenance(sources: &crate::foundry::cell::SourceViewBatch, ordinals: &[usize]) {
    assert_eq!(sources.len(), ordinals.len());
    for (provenance, &ordinal) in sources.provenance().iter().zip(ordinals) {
        assert_eq!(provenance.translated().offset().values(), [0; 6]);
        assert_eq!(
            provenance.translated().source_row().stable_string(),
            ORDINARY_ROWS[ordinal]
        );
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_projection(
    sources: &crate::foundry::cell::SourceViewBatch,
    sector: [i64; 6],
    rows: usize,
) {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        panic!("generated rays must retain residual projection evidence")
    };
    assert_eq!(evidence.domain().bounds(), source_bounds(sector));
    assert_eq!(evidence.fixed_restrictions(), fixed_source(sector));
    assert_eq!(evidence.original_relations().len(), rows);
    assert_eq!(evidence.term_projections().len(), rows);
}

fn assert_rule(
    rule: &ParametricRule,
    sector: [i64; 6],
    pivot: &[i64; 6],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(rule.anchor().powers(), sector);
    assert_eq!(rule.pivot().values(), pivot);
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
    assert_eq!(concrete_metrics(&replay), metrics);
}

fn selected_ordinals(rule: &ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|source| source.source_ordinal())
        .collect()
}

fn source_coefficient_expressions(rule: &ParametricRule) -> Vec<String> {
    rule.source_combination()
        .iter()
        .map(|source| coefficient_expression(source.coefficient()))
        .collect()
}

fn rhs_coefficient_expressions(rule: &ParametricRule) -> Vec<String> {
    rule.right_hand_side()
        .iter()
        .map(|term| coefficient_expression(term.coefficient()))
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

fn source_bounds(sector: [i64; 6]) -> [InteriorBounds; 6] {
    let mut bounds = sector.map(|power| InteriorBounds::new(power, power));
    bounds[FREE_POSITION] = InteriorBounds::new(1, i64::MAX);
    bounds
}

fn application_bounds(sector: [i64; 6]) -> [InteriorBounds; 6] {
    let mut bounds = sector.map(|power| InteriorBounds::new(power, power));
    bounds[FREE_POSITION] = InteriorBounds::new(1, i64::MAX - 1);
    bounds
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
