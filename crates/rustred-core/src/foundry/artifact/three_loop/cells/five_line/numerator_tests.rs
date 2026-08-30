use crate::algebra::{IndexedAlgebraLimits, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{
    FixedIndexRestriction, RuleCell, RuleCellDomainProof, SourceViewConstruction,
};
use crate::foundry::parametric::{ParametricRuleError, replay_rule_at_concrete_assignment};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::InteriorBounds;

use super::numerator::*;
use super::*;

const SCALAR_RHS: [[i64; 6]; 8] = [
    [-1, -1, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, -1, 1, 0, 0, 0],
    [0, -1, 0, 0, 1, 0],
    [1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, -1],
    [0, -1, 0, 0, 1, -1],
    [1, 0, 0, 0, 0, -1],
];

const ADJACENT_RHS: [[i64; 6]; 7] = [
    [0, 1, 0, 0, -1, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -1, 1],
    [0, 0, 0, 0, -1, 0],
    [0, 0, -1, 0, -1, 1],
    [0, -1, 0, 0, 0, 0],
    [0, -1, 0, 0, -1, 1],
];

const OPPOSITE_RHS: [[i64; 6]; 21] = [
    [-1, -1, 1, 0, 0, 0],
    [0, 1, 0, 0, 0, -1],
    [0, 1, 0, 0, -1, 0],
    [0, 1, -1, 0, 0, 0],
    [0, 0, 1, 0, 0, -1],
    [0, 0, 1, -1, 0, 0],
    [0, 0, 0, 1, 0, -1],
    [0, 0, 0, 1, -1, 0],
    [0, 0, 0, 0, 1, -1],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -1, 1],
    [0, 0, 0, -1, 1, 0],
    [0, 0, 0, -1, 0, 1],
    [1, 0, 0, 0, 0, 0],
    [0, 0, 1, -1, 0, -1],
    [0, 0, 0, 0, 0, -1],
    [0, 0, 0, 0, -1, 0],
    [0, 0, 0, -1, 0, 0],
    [0, -1, 0, 0, 0, 0],
    [1, 0, 0, 0, -1, 0],
    [1, -1, 0, 0, 0, 0],
];

#[test]
fn generated_sources_retain_exact_selection_and_projection_evidence() {
    let (context, cells) = derive_five_line_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();

    for cell in [
        &cells.scalar_numerator_bulk,
        &cells.scalar_numerator_endpoint,
    ] {
        assert_eq!(cell.sources().len(), SCALAR_SELECTION.len());
        assert_provenance(
            cell,
            &[
                ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
                ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
                ([0, 0, 0, 0, 0, -1], "ordinary-ibp:0:0"),
                ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
                ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
            ],
        );
        let SourceViewConstruction::ResidualProjection(evidence) = cell.sources().construction()
        else {
            panic!("scalar numerator sources must retain residual projection")
        };
        assert_eq!(
            evidence.domain().bounds(),
            [
                InteriorBounds::new(i64::MIN + 1, 0),
                InteriorBounds::new(1, 1),
                InteriorBounds::new(1, 1),
                InteriorBounds::new(1, 1),
                InteriorBounds::new(1, 1),
                InteriorBounds::new(1, 1),
            ]
        );
        assert_eq!(
            evidence.fixed_restrictions(),
            (1..6)
                .map(|position| FixedIndexRestriction::new(position, 1))
                .collect::<Vec<_>>()
        );
        let missing_edge_stabilizer = canonicalizer
            .group_elements()
            .enumerate()
            .filter_map(|(ordinal, mapping)| (mapping[0] == 0).then_some(ordinal))
            .collect::<Vec<_>>();
        assert_eq!(missing_edge_stabilizer.len(), 4);
        assert_eq!(
            evidence.stabilizer_group_elements(),
            missing_edge_stabilizer
        );
        assert_eq!(evidence.original_relations().len(), SCALAR_SELECTION.len());
        assert_eq!(evidence.term_projections().len(), SCALAR_SELECTION.len());
        assert!(
            cell.sources()
                .verify_residual_projection(
                    &context,
                    &canonicalizer,
                    &zero_sectors,
                    Default::default(),
                )
                .unwrap()
        );
    }

    for cell in [
        &cells.adjacent_numerator_bulk,
        &cells.adjacent_numerator_endpoint,
    ] {
        assert!(matches!(
            cell.sources().construction(),
            SourceViewConstruction::Direct
        ));
        assert_provenance(cell, &[([0, 0, 0, 0, -1, 0], "ordinary-ibp:1:1")]);
    }
    let opposite_provenance = [
        ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
        ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:1:1"),
        ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:1:2"),
        ([0, 0, 0, 0, 0, -1], "ordinary-ibp:0:2"),
        ([0, 0, 0, 0, 0, -1], "ordinary-ibp:2:2"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:1"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:2"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:1"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:2:1"),
        ([0, 0, 0, 0, 0, 0], "ordinary-ibp:2:2"),
    ];
    for cell in [
        &cells.opposite_numerator_bulk,
        &cells.opposite_numerator_endpoint,
    ] {
        assert!(matches!(
            cell.sources().construction(),
            SourceViewConstruction::Direct
        ));
        assert_provenance(cell, &opposite_provenance);
    }
}

#[test]
fn exact_rules_guards_pruning_and_domains_are_pinned() {
    let (context, cells) = derive_five_line_cells().unwrap();
    assert_pair_rule(
        &cells.scalar_numerator_bulk,
        &cells.scalar_numerator_endpoint,
        &SCALAR_RHS,
        (5, 29, 8, 3, 38, 105),
    );
    assert_pair_rule(
        &cells.adjacent_numerator_bulk,
        &cells.adjacent_numerator_endpoint,
        &ADJACENT_RHS,
        (1, 8, 7, 1, 16, 40),
    );
    assert_pair_rule(
        &cells.opposite_numerator_bulk,
        &cells.opposite_numerator_endpoint,
        &OPPOSITE_RHS,
        (12, 117, 21, 9, 139, 382),
    );

    assert_guard_expressions(
        &cells.scalar_numerator_bulk,
        &["2-d+n0", "-2+d-n0", "-4+2*d-2*n0"],
    );
    assert_guard_expressions(&cells.scalar_numerator_endpoint, &["2-d", "-2+d", "-4+2*d"]);
    assert_guard_expressions(&cells.adjacent_numerator_bulk, &["-1+n4"]);
    assert_guard_expressions(&cells.adjacent_numerator_endpoint, &["-1+n4"]);
    assert_guard_expressions(
        &cells.opposite_numerator_bulk,
        &[
            "-n3",
            "n4",
            "-2*n1",
            "-n2",
            "n3",
            "n2",
            "n1",
            "2+2*d-2*n0-4*n1-2*n2",
            "1+d-n0-2*n1-n2",
        ],
    );
    assert_guard_expressions(
        &cells.opposite_numerator_endpoint,
        &[
            "-n3",
            "n4",
            "-2*n1",
            "-n2",
            "n3",
            "n2",
            "n1",
            "2+2*d-4*n1-2*n2",
            "1+d-2*n1-n2",
        ],
    );
    for cell in [
        &cells.scalar_numerator_bulk,
        &cells.scalar_numerator_endpoint,
        &cells.opposite_numerator_bulk,
        &cells.opposite_numerator_endpoint,
    ] {
        assert_dimension_guards_have_constant_nonzero_lead(&context, cell);
    }

    assert_eq!(
        cells.scalar_numerator_bulk.application_domain().bounds(),
        [
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ]
    );
    assert_eq!(
        cells
            .scalar_numerator_endpoint
            .application_domain()
            .bounds(),
        FIVE_LINE_SECTOR.map(|power| InteriorBounds::new(power, power))
    );
    assert_eq!(
        cells.adjacent_numerator_bulk.application_domain().bounds(),
        [
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
        ]
    );
    assert_eq!(
        cells
            .adjacent_numerator_endpoint
            .application_domain()
            .bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
        ]
    );
    assert_eq!(
        cells.opposite_numerator_bulk.application_domain().bounds(),
        [
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(2, i64::MAX - 1),
        ]
    );
    assert_eq!(
        cells
            .opposite_numerator_endpoint
            .application_domain()
            .bounds(),
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(2, i64::MAX - 1),
        ]
    );
    assert_eq!(
        cells.scalar_numerator_bulk.fixed_restrictions(),
        (1..6)
            .map(|position| FixedIndexRestriction::new(position, 1))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        cells.scalar_numerator_endpoint.fixed_restrictions(),
        (0..6)
            .map(|position| {
                FixedIndexRestriction::new(position, if position == 0 { 0 } else { 1 })
            })
            .collect::<Vec<_>>()
    );
    assert!(
        cells
            .adjacent_numerator_bulk
            .fixed_restrictions()
            .is_empty()
    );
    assert!(
        cells
            .opposite_numerator_bulk
            .fixed_restrictions()
            .is_empty()
    );
    assert_eq!(
        cells.adjacent_numerator_endpoint.fixed_restrictions(),
        [FixedIndexRestriction::new(0, 0)]
    );
    assert_eq!(
        cells.opposite_numerator_endpoint.fixed_restrictions(),
        [FixedIndexRestriction::new(0, 0)]
    );
    assert_exact_endpoint_pruning(
        &context,
        &cells.scalar_numerator_endpoint,
        &SCALAR_ENDPOINT_PRUNED,
    );
    assert_exact_endpoint_pruning(
        &context,
        &cells.adjacent_numerator_endpoint,
        &ADJACENT_ENDPOINT_PRUNED,
    );
    assert_exact_endpoint_pruning(
        &context,
        &cells.opposite_numerator_endpoint,
        &OPPOSITE_ENDPOINT_PRUNED,
    );
}

#[test]
fn held_out_replay_and_machine_endpoints_are_exact() {
    let (context, cells) = derive_five_line_cells().unwrap();
    assert_replay_at(
        &context,
        &cells.scalar_numerator_bulk,
        [-7, 1, 1, 1, 1, 1],
        (5, 29, 8, 3, 38, 105),
    );
    assert_replay_at(
        &context,
        &cells.scalar_numerator_endpoint,
        [0, 1, 1, 1, 1, 1],
        (5, 29, 8, 3, 38, 104),
    );
    assert_replay_at(
        &context,
        &cells.adjacent_numerator_bulk,
        [-7, 3, 2, 4, 2, 5],
        (1, 8, 7, 1, 16, 40),
    );
    assert_replay_at(
        &context,
        &cells.adjacent_numerator_endpoint,
        [0, 3, 2, 4, 2, 5],
        (1, 8, 7, 1, 16, 40),
    );
    assert_replay_at(
        &context,
        &cells.opposite_numerator_bulk,
        [-7, 2, 3, 2, 4, 3],
        (12, 117, 21, 9, 139, 386),
    );
    assert_replay_at(
        &context,
        &cells.opposite_numerator_endpoint,
        [0, 2, 3, 2, 4, 3],
        (12, 117, 21, 9, 139, 385),
    );

    for (bulk, endpoint, active) in [
        (
            &cells.scalar_numerator_bulk,
            &cells.scalar_numerator_endpoint,
            [1, 1, 1, 1, 1],
        ),
        (
            &cells.adjacent_numerator_bulk,
            &cells.adjacent_numerator_endpoint,
            [1, 1, 1, 2, 1],
        ),
        (
            &cells.opposite_numerator_bulk,
            &cells.opposite_numerator_endpoint,
            [1, 1, 1, 1, 2],
        ),
    ] {
        let key = |inactive| {
            IntegralKey::try_new([
                inactive, active[0], active[1], active[2], active[3], active[4],
            ])
            .unwrap()
        };
        assert_eq!(
            bulk.assignment_for_target(&key(i64::MIN)).unwrap(),
            Some(vec![
                i64::MIN + 1,
                active[0],
                active[1],
                active[2],
                active[3],
                active[4],
            ])
        );
        assert!(bulk.assignment_for_target(&key(-2)).unwrap().is_some());
        assert!(bulk.assignment_for_target(&key(-1)).unwrap().is_none());
        assert!(endpoint.assignment_for_target(&key(-2)).unwrap().is_none());
        assert_eq!(
            endpoint.assignment_for_target(&key(-1)).unwrap(),
            Some(vec![
                0, active[0], active[1], active[2], active[3], active[4],
            ])
        );
        assert!(endpoint.assignment_for_target(&key(0)).unwrap().is_none());
        assert!(
            bulk.assignment_for_target(&key(i64::MAX))
                .unwrap()
                .is_none()
        );
    }

    assert!(
        cells
            .adjacent_numerator_bulk
            .assignment_for_target(&IntegralKey::try_new([-2, 1, 1, 1, i64::MAX, 1]).unwrap())
            .unwrap()
            .is_some()
    );
    assert!(
        cells
            .adjacent_numerator_bulk
            .assignment_for_target(&IntegralKey::try_new([-2, i64::MAX, 1, 1, 2, 1]).unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        cells
            .opposite_numerator_bulk
            .assignment_for_target(&IntegralKey::try_new([-2, 1, 1, 1, 1, i64::MAX - 1]).unwrap())
            .unwrap()
            .is_some()
    );
    assert!(
        cells
            .opposite_numerator_bulk
            .assignment_for_target(&IntegralKey::try_new([-2, 1, 1, 1, 1, i64::MAX]).unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn source_minimality_and_s4_orbits_are_explicit() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let (completed, source_count) = complete_ordinary_sources(&generator).unwrap();

    for removed in 0..SCALAR_SELECTION.len() {
        let selection = SCALAR_SELECTION
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(position, source)| (position != removed).then_some(source))
            .collect::<Vec<_>>();
        let sources = scalar_corner_sources(
            &generator,
            &completed,
            source_count,
            &selection,
            &canonicalizer,
            &zero_sectors,
        )
        .unwrap();
        assert_eq!(
            derive_numerator_rule(&generator, &sources),
            Err(ArtifactError::ParametricRule(
                ParametricRuleError::TargetShiftNotPivot
            ))
        );
    }
    for removed in 0..OPPOSITE_SELECTION.len() {
        let selection = OPPOSITE_SELECTION
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(position, source)| (position != removed).then_some(source))
            .collect::<Vec<_>>();
        let sources = direct_selected_sources(
            &generator,
            &completed,
            source_count,
            &OPPOSITE_TRANSLATIONS,
            &selection,
        )
        .unwrap();
        assert!(derive_numerator_rule(&generator, &sources).is_err());
    }

    let (context, cells) = derive_five_line_cells().unwrap();
    let canonical_adjacent = IntegralKey::try_new([-8, 1, 1, 1, 2, 1]).unwrap();
    for slot in 1..5 {
        let mut decorated = [-8, 1, 1, 1, 1, 1];
        decorated[slot] = 2;
        assert_eq!(
            canonicalizer
                .canonicalize(&IntegralKey::try_new(decorated).unwrap())
                .unwrap()
                .canonical(),
            &canonical_adjacent
        );
    }
    let canonical_opposite = IntegralKey::try_new([-8, 1, 1, 1, 1, 2]).unwrap();
    assert_eq!(
        canonicalizer
            .canonicalize(&canonical_opposite)
            .unwrap()
            .canonical(),
        &canonical_opposite
    );
    assert_ne!(canonical_adjacent, canonical_opposite);
    assert!(
        cells
            .adjacent_numerator_bulk
            .assignment_for_target(&canonical_adjacent)
            .unwrap()
            .is_some()
    );
    assert!(
        cells
            .opposite_numerator_bulk
            .assignment_for_target(&canonical_adjacent)
            .unwrap()
            .is_none()
    );
    assert!(
        cells
            .opposite_numerator_bulk
            .assignment_for_target(&canonical_opposite)
            .unwrap()
            .is_some()
    );
    assert!(
        cells
            .adjacent_numerator_bulk
            .assignment_for_target(&canonical_opposite)
            .unwrap()
            .is_none()
    );
    assert_eq!(context.fingerprint(), generator.context().fingerprint());
}

fn assert_provenance(cell: &RuleCell, expected: &[([i64; 6], &str)]) {
    assert_eq!(cell.sources().provenance().len(), expected.len());
    for (provenance, (offset, row)) in cell.sources().provenance().iter().zip(expected) {
        assert_eq!(provenance.translated().offset().values(), offset);
        assert_eq!(provenance.translated().source_row().stable_string(), *row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_pair_rule(
    bulk: &RuleCell,
    endpoint: &RuleCell,
    rhs: &[[i64; 6]],
    metrics: (usize, usize, usize, usize, usize, usize),
) {
    for cell in [bulk, endpoint] {
        assert_eq!(cell.rule().anchor().powers(), NUMERATOR_RULE_ANCHOR);
        assert_eq!(cell.rule().pivot().values(), NUMERATOR_PIVOT);
        assert_eq!(
            cell.rule()
                .right_hand_side()
                .iter()
                .map(|term| term.shift().values())
                .collect::<Vec<_>>(),
            rhs.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
        );
        let replay = cell.rule().concrete_replay();
        assert_eq!(replay.source_contributions_checked(), metrics.0);
        assert_eq!(replay.source_terms_checked(), metrics.1);
        assert_eq!(replay.right_hand_side_terms_checked(), metrics.2);
        assert_eq!(replay.nonzero_guards_checked(), metrics.3);
        assert_eq!(replay.integral_keys_checked(), metrics.4);
        assert_eq!(replay.exact_operations(), metrics.5);
        assert_eq!(
            cell.domain_proof(),
            RuleCellDomainProof::ReprovedSectorMonotone
        );
        assert!(cell.terms().iter().all(|term| term.descent().verify()));
    }
}

fn assert_guard_expressions(cell: &RuleCell, expected: &[&str]) {
    let actual = cell
        .guards()
        .iter()
        .map(|guard| normalized_additive_terms(&guard.polynomial().to_expression().to_string()))
        .collect::<Vec<_>>();
    let expected = expected
        .iter()
        .map(|expression| normalized_additive_terms(expression))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

/// Symbolica's globally registered symbol order may vary when this test runs
/// concurrently with other contexts. Pin the exact signed term multisets,
/// without treating presentation order as algebraic evidence.
fn normalized_additive_terms(expression: &str) -> Vec<String> {
    let mut terms = expression
        .replace('-', "+-")
        .split('+')
        .filter(|term| !term.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    terms.sort_unstable();
    terms
}

fn assert_dimension_guards_have_constant_nonzero_lead(
    context: &IndexedCoefficientContext,
    cell: &RuleCell,
) {
    assert_eq!(context.base().parameter_names(), &["d"]);
    for guard in cell.guards() {
        let raw = guard.polynomial().raw();
        let max_dimension_degree = raw
            .exponents_iter()
            .map(|exponents| exponents[0])
            .max()
            .unwrap();
        if max_dimension_degree == 0 {
            continue;
        }
        let leading = raw
            .coefficients
            .iter()
            .zip(raw.exponents_iter())
            .filter(|(_, exponents)| exponents[0] == max_dimension_degree)
            .collect::<Vec<_>>();
        assert_eq!(leading.len(), 1);
        assert!(leading[0].1[1..].iter().all(|&power| power == 0));
        assert!(!leading[0].0.is_zero());
    }
}

fn assert_exact_endpoint_pruning(
    context: &IndexedCoefficientContext,
    endpoint: &RuleCell,
    expected: &[usize],
) {
    assert_eq!(endpoint.pruned_rhs_ordinals(), expected);
    let exactly_zero = endpoint
        .rule()
        .right_hand_side()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, term)| {
            let (specialized, _guard) = context
                .specialize_fixed_indices_sealed(
                    term.coefficient(),
                    &[(0, 0)],
                    IndexedAlgebraLimits::default(),
                )
                .unwrap();
            specialized.is_zero().then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(exactly_zero, expected);
    assert_eq!(
        endpoint
            .terms()
            .iter()
            .map(|term| term.source_rhs_ordinal())
            .collect::<Vec<_>>(),
        (0..endpoint.rule().right_hand_side().len())
            .filter(|ordinal| !expected.contains(ordinal))
            .collect::<Vec<_>>()
    );
}

fn assert_replay_at(
    context: &IndexedCoefficientContext,
    cell: &RuleCell,
    assignment: [i64; 6],
    metrics: (usize, usize, usize, usize, usize, usize),
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
    assert_eq!(replay.nonzero_guards_checked(), metrics.3);
    assert_eq!(replay.integral_keys_checked(), metrics.4);
    assert_eq!(replay.exact_operations(), metrics.5);
}
