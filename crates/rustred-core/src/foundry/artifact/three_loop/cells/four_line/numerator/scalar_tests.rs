use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewConstruction};
use crate::foundry::parametric::{ParametricRuleError, replay_rule_at_concrete_assignment};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::InteriorBounds;

use super::super::super::super::{
    K6ReachabilityTerminals, canonical_family, canonical_s4, exact_zero_sectors,
};
use super::super::super::support::complete_ordinary_sources;
use super::super::FOUR_LINE_SECTOR;
use super::super::corner::{
    derive_exact_corner_cell, fixed_base_corner, project_complete_exact_corner_sources,
};
use super::scalar::{
    BULK_REPLAY_ANCHOR, INACTIVE_NUMERATOR_PIVOT, InactiveNumeratorBuild,
    derive_inactive_numerator_build, derive_inactive_numerator_cells, fixed_scalar_face,
    inactive_numerator_search_depth,
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

const ENDPOINT_SELECTION: [usize; 4] = [0, 4, 18, 21];
const BULK_SELECTION: [usize; 5] = [9, 13, 18, 21, 22];

const ENDPOINT_RHS: [[i64; 6]; 2] = [[0, 0, 0, 0, 0, 0], [0, -1, 0, 0, 1, 0]];
const BULK_RHS: [[i64; 6]; 3] = [[0, 0, 0, 0, 0, 0], [0, -1, 1, 0, 0, 0], [0, 0, 0, 0, 0, 1]];

#[test]
fn generated_depth_one_search_and_selected_bulk_projection_are_exact() {
    let InactiveNumeratorBuild {
        context,
        endpoint,
        bulk,
        endpoint_selected_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        bulk_selection_witness,
    } = derive_inactive_numerator_build(true).unwrap();
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        inactive_numerator_search_depth(),
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
            .map(|offset| offset.as_slice())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        endpoint.sources().len(),
        DEPTH_ONE_OFFSETS.len() * ORDINARY_ROWS.len()
    );
    assert_complete_provenance(&endpoint, &search);
    assert_eq!(
        endpoint_selected_complete_source_ordinals.as_ref(),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        endpoint
            .rule()
            .source_combination()
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        ENDPOINT_SELECTION
    );
    assert_selected_provenance(
        &endpoint,
        &ENDPOINT_SELECTION,
        &[
            ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
            ([-1, 0, 0, 0, 0, 0], "ordinary-ibp:1:1"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
        ],
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let SourceViewConstruction::ResidualProjection(endpoint_projection) =
        endpoint.sources().construction()
    else {
        panic!("endpoint must retain complete exact-corner projection evidence")
    };
    assert_eq!(
        endpoint_projection.domain().bounds(),
        FOUR_LINE_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(
        endpoint_projection.fixed_restrictions(),
        fixed_base_corner()
    );
    assert_eq!(
        endpoint_projection.stabilizer_group_elements(),
        [0, 1, 2, 3, 20, 21, 22, 23]
    );
    assert_eq!(endpoint_projection.original_relations().len(), 7 * 9);
    assert_eq!(endpoint_projection.term_projections().len(), 7 * 9);
    assert!(
        endpoint
            .sources()
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );

    assert_eq!(
        bulk_selected_complete_source_ordinals.as_ref(),
        BULK_SELECTION
    );
    let selection = bulk_selection_witness.expect("exact test retains complete bulk selection");
    assert_eq!(selection.sources.len(), 7 * 9);
    assert_complete_provenance_from_sources(&selection.sources, &search);
    assert_eq!(
        selection
            .rule
            .source_combination()
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        BULK_SELECTION
    );
    assert_eq!(selection.rule.anchor().powers(), BULK_REPLAY_ANCHOR);
    assert_eq!(selection.rule.pivot().values(), INACTIVE_NUMERATOR_PIVOT);
    let SourceViewConstruction::ResidualProjection(selection_projection) =
        selection.sources.construction()
    else {
        panic!("complete bulk selection must retain projection evidence")
    };
    assert_eq!(
        selection_projection.domain().bounds(),
        scalar_face_bounds(i64::MIN + 2, -1)
    );
    assert_eq!(
        selection_projection.fixed_restrictions(),
        fixed_scalar_face()
    );
    assert_eq!(
        selection_projection.stabilizer_group_elements(),
        [0, 1, 2, 3]
    );
    assert_eq!(selection_projection.original_relations().len(), 7 * 9);
    assert_eq!(selection_projection.term_projections().len(), 7 * 9);
    assert!(
        selection
            .sources
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );

    assert_eq!(bulk.sources().len(), BULK_SELECTION.len());
    assert_selected_provenance(
        &bulk,
        &[0, 1, 2, 3, 4],
        &[
            ([0, 0, 0, 0, 0, -1], "ordinary-ibp:0:0"),
            ([0, 0, 0, 0, 0, -1], "ordinary-ibp:1:1"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:1"),
        ],
    );
    let SourceViewConstruction::ResidualProjection(bulk_projection) = bulk.sources().construction()
    else {
        panic!("selected bulk must retain independent projection evidence")
    };
    assert_eq!(
        bulk_projection.domain().bounds(),
        scalar_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(bulk_projection.fixed_restrictions(), fixed_scalar_face());
    assert_eq!(bulk_projection.stabilizer_group_elements(), [0, 1, 2, 3]);
    assert_eq!(
        bulk_projection.original_relations().len(),
        BULK_SELECTION.len()
    );
    assert_eq!(
        bulk_projection.term_projections().len(),
        BULK_SELECTION.len()
    );
    assert!(
        bulk.sources()
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );
}

#[test]
fn exact_rules_guards_replay_and_machine_endpoints_are_pinned() {
    let (context, endpoint, bulk) = derive_inactive_numerator_cells().unwrap();
    assert_rule(
        &endpoint,
        FOUR_LINE_SECTOR,
        &ENDPOINT_RHS,
        (4, 19, 46),
        (4, 17, 2, 20, 2, 57, 15),
    );
    assert_rule(
        &bulk,
        BULK_REPLAY_ANCHOR,
        &BULK_RHS,
        (5, 10, 48),
        (5, 24, 3, 28, 2, 79, 21),
    );
    assert_eq!(guard_expressions(&endpoint), ["6-3*d", "-6+3*d"]);
    assert_eq!(guard_expressions(&bulk), ["6-3*d+2*n5", "-6+3*d-2*n5"]);
    assert_dimension_guards_have_constant_nonzero_lead(&context, &endpoint);
    assert_dimension_guards_have_constant_nonzero_lead(&context, &bulk);

    assert_eq!(
        endpoint.application_domain().bounds(),
        FOUR_LINE_SECTOR.map(|value| InteriorBounds::new(value, value))
    );
    assert_eq!(endpoint.fixed_restrictions(), fixed_base_corner());
    assert_eq!(
        bulk.application_domain().bounds(),
        scalar_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(bulk.fixed_restrictions(), fixed_scalar_face());
    assert!(endpoint.terms().iter().all(|term| term.descent().verify()));
    assert!(bulk.terms().iter().all(|term| term.descent().verify()));

    assert_replay_at(&context, &endpoint, FOUR_LINE_SECTOR, (4, 17, 2, 20, 2, 57));
    for inactive in [-1, -7, i64::MIN + 1] {
        let assignment = [0, 1, 1, 1, 1, inactive];
        assert!(bulk.guards().iter().all(|guard| {
            !context
                .specialize_polynomial(guard.polynomial(), &assignment, Default::default())
                .unwrap()
                .is_zero()
        }));
        assert_replay_at(&context, &bulk, assignment, (5, 24, 3, 28, 2, 79));
    }

    let key = |inactive| IntegralKey::try_new([0, 1, 1, 1, 1, inactive]).unwrap();
    assert_eq!(
        bulk.assignment_for_target(&key(i64::MIN)).unwrap(),
        Some(vec![0, 1, 1, 1, 1, i64::MIN + 1])
    );
    assert_eq!(
        bulk.assignment_for_target(&key(-2)).unwrap(),
        Some(vec![0, 1, 1, 1, 1, -1])
    );
    assert!(bulk.assignment_for_target(&key(-1)).unwrap().is_none());
    assert!(endpoint.assignment_for_target(&key(-2)).unwrap().is_none());
    assert_eq!(
        endpoint.assignment_for_target(&key(-1)).unwrap(),
        Some(FOUR_LINE_SECTOR.to_vec())
    );
    assert!(endpoint.assignment_for_target(&key(0)).unwrap().is_none());
}

#[test]
fn symmetry_and_canonical_descendants_keep_the_open_frontier_explicit() {
    let (_context, endpoint, bulk) = derive_inactive_numerator_cells().unwrap();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let terminals = K6ReachabilityTerminals::try_new().unwrap();

    let endpoint_children = canonical_children(&canonicalizer, &endpoint, &FOUR_LINE_SECTOR);
    assert_eq!(
        endpoint_children,
        [vec![0, 1, 1, 1, 1, 0], vec![0, 0, 1, 0, 2, 1]]
    );
    assert!(terminals.classify(&key(&endpoint_children[0])).is_none());
    assert!(matches!(
        terminals.classify(&key(&endpoint_children[1])),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));

    for inactive in [-1, -7, i64::MIN + 1] {
        let assignment = [0, 1, 1, 1, 1, inactive];
        let children = canonical_children(&canonicalizer, &bulk, &assignment);
        assert_eq!(
            children,
            [
                vec![0, 1, 1, 1, 1, inactive],
                vec![0, 0, 2, inactive, 1, 1],
                vec![0, 1, 1, 1, 1, inactive + 1],
            ]
        );
        // The two four-line children recurse toward the endpoint. The
        // pinched numerator child is deliberately not mislabeled as an
        // existing factorization terminal.
        assert!(terminals.classify(&key(&children[1])).is_none());
    }

    for inactive in [-1, -2, -7, i64::MIN] {
        let left = canonicalizer
            .canonicalize(&IntegralKey::try_new([inactive, 1, 1, 1, 1, 0]).unwrap())
            .unwrap();
        let right = canonicalizer
            .canonicalize(&IntegralKey::try_new([0, 1, 1, 1, 1, inactive]).unwrap())
            .unwrap();
        assert_eq!(left.canonical(), right.canonical());
        assert_eq!(right.canonical().powers(), [0, 1, 1, 1, 1, inactive]);
    }
}

#[test]
fn endpoint_target_first_appears_at_complete_depth_one() {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let zero_sectors = exact_zero_sectors(&canonicalizer).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let (completed, _) = complete_ordinary_sources(&generator).unwrap();
    let depth_zero = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR).unwrap(),
        0,
        SectorSearchLimits::default(),
    )
    .unwrap();
    let sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        depth_zero.offsets().iter().cloned(),
    )
    .unwrap();
    assert!(matches!(
        derive_exact_corner_cell(&generator, sources, &INACTIVE_NUMERATOR_PIVOT),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    ));
    assert_eq!(inactive_numerator_search_depth(), 1);
}

fn assert_complete_provenance(cell: &RuleCell, search: &SectorSearchDiamond) {
    assert_complete_provenance_from_sources(cell.sources(), search);
}

fn assert_complete_provenance_from_sources(
    sources: &crate::foundry::cell::SourceViewBatch,
    search: &SectorSearchDiamond,
) {
    for (offset, provenance) in search.offsets().iter().zip(sources.provenance().chunks(9)) {
        assert_eq!(provenance.len(), 9);
        for (source, row) in provenance.iter().zip(ORDINARY_ROWS) {
            assert_eq!(source.translated().offset(), offset);
            assert_eq!(source.translated().source_row().stable_string(), row);
            assert!(source.symmetry().is_none());
        }
    }
}

fn assert_selected_provenance(
    cell: &RuleCell,
    source_ordinals: &[usize],
    expected: &[([i64; 6], &str)],
) {
    assert_eq!(source_ordinals.len(), expected.len());
    for (&ordinal, (offset, row)) in source_ordinals.iter().zip(expected) {
        let provenance = &cell.sources().provenance()[ordinal];
        assert_eq!(provenance.translated().offset().values(), offset);
        assert_eq!(provenance.translated().source_row().stable_string(), *row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_rule(
    cell: &RuleCell,
    anchor: [i64; 6],
    rhs: &[[i64; 6]],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(cell.rule().anchor().powers(), anchor);
    assert_eq!(cell.rule().pivot().values(), INACTIVE_NUMERATOR_PIVOT);
    assert_eq!(
        cell.rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        rhs.iter().map(|shift| shift.as_slice()).collect::<Vec<_>>()
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

fn guard_expressions(cell: &RuleCell) -> Vec<String> {
    cell.guards()
        .iter()
        .map(|guard| guard.polynomial().to_expression().to_string())
        .collect()
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
    assert_eq!(replay.integral_keys_checked(), metrics.3);
    assert_eq!(replay.nonzero_guards_checked(), metrics.4);
    assert_eq!(replay.exact_operations(), metrics.5);
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

fn scalar_face_bounds(lower: i64, upper: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, upper),
    ]
}
