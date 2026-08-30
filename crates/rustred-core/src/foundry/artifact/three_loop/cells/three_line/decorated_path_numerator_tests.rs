use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{
    RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::parametric::{ParametricRuleError, replay_rule_at_concrete_assignment};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::InteriorBounds;

use super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::support::complete_ordinary_sources;
use super::decorated_path_numerator::{
    BULK_REPLAY_ANCHOR, FREE_POSITION, PATH_NUMERATOR_PIVOT, PATH_SECTOR, PathNumeratorBuild,
    derive_decorated_path_numerator_cells, derive_direct_endpoint_rule, derive_free_rule,
    derive_path_numerator_build, fixed_endpoint, fixed_free_face, path_numerator_search_depth,
    project_complete_endpoint_sources, project_complete_free_sources,
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
    [0, 0, 0, -1, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 1, 0],
    [0, 0, 1, 0, 0, 0],
];

const DIRECT_ENDPOINT_SELECTION: [usize; 7] = [9, 12, 13, 15, 16, 27, 28];
const ENDPOINT_SELECTION: [usize; 10] = [18, 22, 24, 25, 26, 27, 28, 31, 33, 34];
const BULK_SELECTION: [usize; 25] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18, 21, 22, 23, 26, 27, 28, 29, 30, 31, 32, 33, 34,
];
const MACHINE_UNSAFE: [usize; 4] = [19, 20, 24, 25];
const RHS: [[i64; 6]; 2] = [[0, 0, 0, 0, 0, 0], [0, 0, 0, 1, 0, 0]];

#[test]
fn complete_depth_one_search_and_machine_safe_selection_are_exact() {
    let PathNumeratorBuild {
        context,
        endpoint,
        bulk,
        direct_endpoint_selected_complete_source_ordinals,
        endpoint_selected_complete_source_ordinals,
        machine_safe_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        selection_witness,
    } = derive_path_numerator_build(true).unwrap();
    let selection = selection_witness.expect("exact test retains complete selection evidence");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(PATH_SECTOR).unwrap(),
        path_numerator_search_depth(),
        SectorSearchLimits::default(),
    )
    .unwrap();
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

    assert_eq!(selection.direct_endpoint_sources.len(), 7 * 9);
    assert_complete_provenance(&selection.direct_endpoint_sources, &search);
    assert_eq!(
        direct_endpoint_selected_complete_source_ordinals.as_ref(),
        DIRECT_ENDPOINT_SELECTION
    );
    assert_eq!(
        selected_ordinals(&selection.direct_endpoint_rule),
        DIRECT_ENDPOINT_SELECTION
    );
    assert_raw_rule_metrics(
        &selection.direct_endpoint_rule,
        (7, 32, 90),
        (7, 34, 1, 36, 2, 103, 30),
    );
    assert_eq!(selection.complete_free_sources.len(), 7 * 9);
    assert_complete_provenance(&selection.complete_free_sources, &search);
    assert_eq!(
        endpoint_selected_complete_source_ordinals.as_ref(),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        selected_ordinals(&selection.complete_free_rule),
        ENDPOINT_SELECTION
    );
    assert_raw_rule_metrics(
        &selection.complete_free_rule,
        (10, 82, 188),
        (10, 62, 2, 65, 13, 196, 43),
    );

    assert_eq!(machine_safe_complete_source_ordinals.len(), 59);
    let unsafe_ordinals = (0..7 * 9)
        .filter(|ordinal| !machine_safe_complete_source_ordinals.contains(ordinal))
        .collect::<Vec<_>>();
    assert_eq!(unsafe_ordinals, MACHINE_UNSAFE);
    for (ordinal, relation) in selection
        .complete_free_sources
        .relations()
        .iter()
        .enumerate()
    {
        let representable = relation.terms().keys().all(|shift| {
            (0..6).all(|position| {
                let bounds = full_source_bounds()[position];
                bounds
                    .lower()
                    .checked_add(shift.values()[position])
                    .is_some()
                    && bounds
                        .upper()
                        .checked_add(shift.values()[position])
                        .is_some()
            })
        });
        assert_eq!(
            representable,
            machine_safe_complete_source_ordinals.contains(&ordinal)
        );
    }
    assert_eq!(selection.machine_safe_sources.len(), 59);
    assert_selected_complete_provenance(
        &selection.machine_safe_sources,
        &machine_safe_complete_source_ordinals,
        &search,
    );
    assert_eq!(
        selection
            .machine_safe_rule
            .source_combination()
            .iter()
            .map(|contribution| {
                machine_safe_complete_source_ordinals[contribution.source_ordinal()]
            })
            .collect::<Vec<_>>(),
        BULK_SELECTION
    );
    assert_raw_rule_metrics(
        &selection.machine_safe_rule,
        (25, 79, 393),
        (25, 176, 2, 179, 13, 512, 55),
    );
    assert_eq!(
        bulk_selected_complete_source_ordinals.as_ref(),
        BULK_SELECTION
    );

    assert_selected_complete_provenance(endpoint.sources(), &ENDPOINT_SELECTION, &search);
    assert_selected_complete_provenance(bulk.sources(), &BULK_SELECTION, &search);
    assert_eq!(
        selected_ordinals(endpoint.rule()),
        (0..10).collect::<Vec<_>>()
    );
    assert_eq!(selected_ordinals(bulk.rule()), (0..25).collect::<Vec<_>>());

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    for sources in [
        &selection.direct_endpoint_sources,
        &selection.complete_free_sources,
        &selection.machine_safe_sources,
        endpoint.sources(),
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
    assert_projection(
        &selection.complete_free_sources,
        complete_source_bounds(),
        63,
    );
    assert_projection(&selection.machine_safe_sources, full_source_bounds(), 59);
    assert_projection(endpoint.sources(), full_source_bounds(), 10);
    assert_projection(bulk.sources(), full_source_bounds(), 25);

    let SourceViewConstruction::ResidualProjection(endpoint_evidence) =
        selection.direct_endpoint_sources.construction()
    else {
        panic!("direct endpoint must retain exact residual projection evidence")
    };
    assert_eq!(
        endpoint_evidence.domain().bounds(),
        PATH_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(endpoint_evidence.fixed_restrictions(), fixed_endpoint());
    let expected_stabilizers = canonicalizer
        .group_elements()
        .enumerate()
        .filter_map(|(ordinal, mapping)| {
            mapping
                .iter()
                .enumerate()
                .all(|(target, &source)| PATH_SECTOR[source] == PATH_SECTOR[target])
                .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_stabilizers.len(), 2);
    assert_eq!(
        endpoint_evidence.stabilizer_group_elements(),
        expected_stabilizers
    );
}

#[test]
fn compact_rules_guards_replay_and_full_i64_bounds_are_pinned() {
    let (context, endpoint, bulk) = derive_decorated_path_numerator_cells().unwrap();
    assert_rule(&endpoint, (10, 18, 124), (10, 62, 2, 65, 2, 185, 43));
    assert_rule(&bulk, (25, 38, 352), (25, 176, 2, 179, 7, 506, 55));
    assert_eq!(guard_expressions(&endpoint), ["-2+2*d", "-4+4*d"]);
    assert_eq!(
        guard_expressions(&bulk),
        [
            "1-d-n3",
            "-1+2*d+d*n3-n3-d^2",
            "-1+d+n3",
            "-1+d-n3",
            "-2*n3",
            "-2+2*d-2*n3",
            "-4+4*d-4*n3",
        ]
    );
    assert_guards_nonzero_on_whole_domain(&bulk);

    assert_eq!(endpoint.pruned_rhs_ordinals(), [1]);
    assert_eq!(
        endpoint
            .terms()
            .iter()
            .map(|term| term.source_rhs_ordinal())
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(
        endpoint.application_domain().bounds(),
        PATH_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(endpoint.fixed_restrictions(), fixed_endpoint());
    assert_eq!(
        bulk.application_domain().bounds(),
        bulk_application_bounds()
    );
    assert_eq!(bulk.fixed_restrictions(), fixed_free_face());
    assert!(endpoint.terms().iter().all(|term| term.descent().verify()));
    assert!(bulk.terms().iter().all(|term| term.descent().verify()));

    assert_replay_at(
        &context,
        &endpoint,
        PATH_SECTOR,
        (10, 62, 2, 65, 2, 181, 34),
    );
    for (free, exact_operations, peak) in [(-1, 504, 42), (-7, 506, 55), (i64::MIN + 1, 506, 55)] {
        assert_replay_at(
            &context,
            &bulk,
            [0, 0, 1, free, 1, 1],
            (25, 176, 2, 179, 7, exact_operations, peak),
        );
    }

    let target = |power| IntegralKey::try_new([0, 0, 2, power, 1, 1]).unwrap();
    assert_eq!(
        bulk.assignment_for_target(&target(i64::MIN)).unwrap(),
        Some(vec![0, 0, 1, i64::MIN + 1, 1, 1])
    );
    assert_eq!(
        bulk.assignment_for_target(&target(-2)).unwrap(),
        Some(vec![0, 0, 1, -1, 1, 1])
    );
    assert!(bulk.assignment_for_target(&target(-1)).unwrap().is_none());
    assert_eq!(
        endpoint.assignment_for_target(&target(-1)).unwrap(),
        Some(PATH_SECTOR.to_vec())
    );
    assert!(
        endpoint
            .assignment_for_target(&target(-2))
            .unwrap()
            .is_none()
    );
}

#[test]
fn symmetry_and_descendants_keep_the_scalar_path_frontier_honest() {
    let (_context, endpoint, bulk) = derive_decorated_path_numerator_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();

    let endpoint_children = canonical_children(&canonicalizer, &endpoint, &PATH_SECTOR);
    assert_eq!(endpoint_children, [vec![0, 0, 1, 0, 1, 1]]);
    assert!(matches!(
        terminals.classify(&key(&endpoint_children[0])),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));

    for free in [-1, -7, i64::MIN + 1] {
        let assignment = [0, 0, 1, free, 1, 1];
        let children = canonical_children(&canonicalizer, &bulk, &assignment);
        assert_eq!(
            children,
            [vec![0, 0, 1, free, 1, 1], vec![0, 0, 1, free + 1, 1, 1],]
        );
        assert!(terminals.classify(&key(&children[0])).is_none());
        if free == -1 {
            assert!(matches!(
                terminals.classify(&key(&children[1])),
                Some(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == 2
            ));
        } else {
            assert!(terminals.classify(&key(&children[1])).is_none());
        }
    }

    let target_orbit = canonicalizer
        .orbit(&IntegralKey::try_new([0, 0, 2, -7, 1, 1]).unwrap())
        .unwrap();
    assert_eq!(target_orbit.group_order(), 24);
    assert_eq!(target_orbit.orbit_size(), 24);
    assert!(
        target_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 1)
    );
    assert_eq!(
        target_orbit.canonical().integral().powers(),
        [0, 0, 2, -7, 1, 1]
    );
    let child_orbit = canonicalizer
        .orbit(&IntegralKey::try_new([0, 0, 1, -7, 1, 1]).unwrap())
        .unwrap();
    assert_eq!(child_orbit.orbit_size(), 24);
    assert!(
        child_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 1)
    );
    assert_eq!(
        child_orbit.canonical().integral().powers(),
        [0, 0, 1, -7, 1, 1]
    );
    let endpoint_orbit = canonicalizer
        .orbit(&IntegralKey::try_new(PATH_SECTOR).unwrap())
        .unwrap();
    assert_eq!(endpoint_orbit.orbit_size(), 12);
    assert!(
        endpoint_orbit
            .images()
            .iter()
            .all(|image| image.routing_multiplicity() == 2)
    );

    // A dot and an inactive numerator on a three-line path split into five
    // inequivalent S4 orbits.  This cell owns exactly the orbit above; the
    // other four canonical representatives remain separate obligations.
    for free in [-1, -7] {
        let owned = canonicalizer
            .canonicalize(&IntegralKey::try_new([0, 0, 2, free, 1, 1]).unwrap())
            .unwrap();
        for (powers, orbit_size) in [
            ([free, 0, 1, 0, 2, 1], 24),
            ([0, 0, 1, free, 2, 1], 24),
            ([free, 0, 1, 0, 1, 2], 12),
            ([0, 0, 1, free, 1, 2], 24),
        ] {
            let alternate = IntegralKey::try_new(powers).unwrap();
            let alternate_orbit = canonicalizer.orbit(&alternate).unwrap();
            assert_eq!(alternate_orbit.orbit_size(), orbit_size);
            assert_eq!(alternate_orbit.canonical().integral(), &alternate);
            assert_ne!(owned.canonical(), &alternate);
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

#[test]
fn direct_endpoint_target_first_appears_at_complete_depth_one() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let (completed, _) = complete_ordinary_sources(&generator).unwrap();
    let depth_zero = SectorSearchDiamond::try_new(
        IntegralKey::try_new(PATH_SECTOR).unwrap(),
        0,
        SectorSearchLimits::default(),
    )
    .unwrap();
    let sources = project_complete_endpoint_sources(
        &generator,
        &completed,
        depth_zero.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )
    .unwrap();
    assert!(matches!(
        derive_direct_endpoint_rule(&generator, &sources),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftNotPivot
        ))
    ));
    let free_sources = project_complete_free_sources(
        &generator,
        &completed,
        depth_zero.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )
    .unwrap();
    assert!(matches!(
        derive_free_rule(&generator, &free_sources),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftNotPivot
        ))
    ));
    assert_eq!(path_numerator_search_depth(), 1);
}

fn assert_complete_provenance(sources: &SourceViewBatch, search: &SectorSearchDiamond) {
    for (offset, provenance) in search.offsets().iter().zip(sources.provenance().chunks(9)) {
        assert_eq!(provenance.len(), 9);
        for (source, row) in provenance.iter().zip(ORDINARY_ROWS) {
            assert_eq!(source.translated().offset(), offset);
            assert_eq!(source.translated().source_row().stable_string(), row);
            assert!(source.symmetry().is_none());
        }
    }
}

fn assert_selected_complete_provenance(
    sources: &SourceViewBatch,
    complete_ordinals: &[usize],
    search: &SectorSearchDiamond,
) {
    assert_eq!(sources.len(), complete_ordinals.len());
    for (provenance, &complete) in sources.provenance().iter().zip(complete_ordinals) {
        let offset = &search.offsets()[complete / ORDINARY_ROWS.len()];
        let row = ORDINARY_ROWS[complete % ORDINARY_ROWS.len()];
        assert_eq!(provenance.translated().offset(), offset);
        assert_eq!(provenance.translated().source_row().stable_string(), row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_projection(sources: &SourceViewBatch, bounds: [InteriorBounds; 6], rows: usize) {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        panic!("generated source selection must retain residual projection evidence")
    };
    assert_eq!(evidence.domain().bounds(), bounds);
    assert_eq!(evidence.fixed_restrictions(), fixed_free_face());
    assert_eq!(evidence.stabilizer_group_elements(), [0]);
    assert_eq!(evidence.original_relations().len(), rows);
    assert_eq!(evidence.term_projections().len(), rows);
}

fn selected_ordinals(rule: &crate::foundry::parametric::ParametricRule) -> Vec<usize> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn assert_rule(
    cell: &RuleCell,
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(cell.rule().anchor().powers(), BULK_REPLAY_ANCHOR);
    assert_eq!(cell.rule().pivot().values(), PATH_NUMERATOR_PIVOT);
    assert_eq!(
        cell.rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        RHS.iter().map(<[i64; 6]>::as_slice).collect::<Vec<_>>()
    );
    assert_eq!(cell.rule().replay().source_rows_used(), parametric.0);
    assert_eq!(cell.rule().replay().shift_columns_checked(), parametric.1);
    assert_eq!(cell.rule().replay().exact_operations(), parametric.2);
    let replay = cell.rule().concrete_replay();
    assert_eq!(replay.source_contributions_checked(), concrete.0);
    assert_eq!(replay.source_terms_checked(), concrete.1);
    assert_eq!(replay.right_hand_side_terms_checked(), concrete.2);
    assert_eq!(replay.integral_keys_checked(), concrete.3);
    assert_eq!(replay.nonzero_guards_checked(), concrete.4);
    assert_eq!(replay.exact_operations(), concrete.5);
    assert_eq!(replay.peak_retained_coefficient_terms(), concrete.6);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
}

fn assert_raw_rule_metrics(
    rule: &crate::foundry::parametric::ParametricRule,
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
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

fn guard_expressions(cell: &RuleCell) -> Vec<String> {
    cell.guards()
        .iter()
        .map(|guard| guard.polynomial().to_expression().to_string())
        .collect()
}

fn assert_guards_nonzero_on_whole_domain(cell: &RuleCell) {
    for (ordinal, guard) in cell.guards().iter().enumerate() {
        let raw = guard.polynomial().raw();
        let max_dimension_degree = raw
            .exponents_iter()
            .map(|exponents| exponents[0])
            .max()
            .unwrap();
        if ordinal == 4 {
            assert_eq!(max_dimension_degree, 0);
            let terms = raw
                .coefficients
                .iter()
                .zip(raw.exponents_iter())
                .collect::<Vec<_>>();
            assert_eq!(terms.len(), 1);
            assert_eq!(terms[0].0.to_string(), "-2");
            assert_eq!(terms[0].1[FREE_POSITION + 1], 1);
            assert!(
                terms[0]
                    .1
                    .iter()
                    .enumerate()
                    .all(|(position, &power)| position == FREE_POSITION + 1 || power == 0)
            );
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

fn assert_replay_at(
    context: &crate::algebra::IndexedCoefficientContext,
    cell: &RuleCell,
    assignment: [i64; 6],
    expected: (usize, usize, usize, usize, usize, usize, usize),
) {
    let replay = replay_rule_at_concrete_assignment(
        context,
        cell.sources().relations(),
        cell.rule(),
        &assignment,
        Default::default(),
    )
    .unwrap();
    assert_eq!(replay.source_contributions_checked(), expected.0);
    assert_eq!(replay.source_terms_checked(), expected.1);
    assert_eq!(replay.right_hand_side_terms_checked(), expected.2);
    assert_eq!(replay.integral_keys_checked(), expected.3);
    assert_eq!(replay.nonzero_guards_checked(), expected.4);
    assert_eq!(
        replay.exact_operations(),
        expected.5,
        "exact-operation mismatch at {assignment:?}"
    );
    assert_eq!(
        replay.peak_retained_coefficient_terms(),
        expected.6,
        "peak mismatch at {assignment:?}"
    );
}

fn canonical_children(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    cell: &RuleCell,
    assignment: &[i64; 6],
) -> Vec<Vec<i64>> {
    cell.terms()
        .iter()
        .map(|cell_term| {
            let shift = cell.rule().right_hand_side()[cell_term.source_rhs_ordinal()]
                .shift()
                .values();
            let raw = IntegralKey::try_new(std::array::from_fn::<_, 6, _>(|position| {
                assignment[position].checked_add(shift[position]).unwrap()
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

fn complete_source_bounds() -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(i64::MIN + 2, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

fn full_source_bounds() -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(i64::MIN + 1, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

fn bulk_application_bounds() -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(i64::MIN + 1, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}
