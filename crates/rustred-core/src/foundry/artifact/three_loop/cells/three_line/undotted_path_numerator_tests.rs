use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{
    ResidualTermDisposition, RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch,
    SourceViewConstruction,
};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleError, replay_rule_at_concrete_assignment,
};
use crate::foundry::search::{
    ReachabilityTerminalKind, ReachabilityTerminalProvider, SectorSearchDiamond, SectorSearchLimits,
};
use crate::sector::InteriorBounds;

use super::super::super::{K6ReachabilityTerminals, canonical_family, canonical_s4};
use super::undotted_path_numerator::{
    UNDOTTED_BULK_REPLAY_ANCHOR, UNDOTTED_PATH_NUMERATOR_PIVOT, UNDOTTED_PATH_SECTOR,
    UndottedPathNumeratorBuild, derive_undotted_bulk_candidate, derive_undotted_endpoint_candidate,
    derive_undotted_path_numerator_build, fixed_endpoint, fixed_free_face,
    undotted_path_numerator_search_depth,
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

const ENDPOINT_SELECTION: [usize; 6] = [12, 13, 15, 16, 27, 28];
const UNSAFE_COMPLETE_SOURCE_ORDINALS: [usize; 4] = [19, 20, 24, 25];
const MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS: [usize; 59] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 21, 22, 23, 26, 27, 28, 29,
    30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 59, 60, 61, 62,
];
const BULK_SELECTION_IN_SAFE_SPAN: [usize; 5] = [18, 19, 23, 26, 29];
const BULK_SELECTION_IN_COMPLETE_SPAN: [usize; 5] = [18, 21, 27, 30, 33];

const ENDPOINT_RHS: [[i64; 6]; 1] = [[0, 0, 0, 0, 0, 0]];
const BULK_RHS: [[i64; 6]; 2] = [[0, 0, 0, 0, 0, 0], [0, 0, 0, 1, 0, 0]];

#[test]
fn generated_undotted_path_numerator_slice_is_exact_and_machine_wide() {
    assert!(matches!(
        derive_undotted_endpoint_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    ));
    assert!(matches!(
        derive_undotted_bulk_candidate(0),
        Err(ArtifactError::ParametricRule(
            ParametricRuleError::TargetShiftAbsent
        ))
    ));
    assert_eq!(undotted_path_numerator_search_depth(), 1);

    let UndottedPathNumeratorBuild {
        context,
        endpoint,
        bulk,
        endpoint_selected_complete_source_ordinals,
        machine_safe_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        selection_witness,
    } = derive_undotted_path_numerator_build(true).unwrap();
    let selection = selection_witness.expect("exact test retains generated selection evidence");
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(UNDOTTED_PATH_SECTOR).unwrap(),
        undotted_path_numerator_search_depth(),
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

    assert_eq!(selection.complete_endpoint_sources.len(), 7 * 9);
    assert_complete_provenance(&selection.complete_endpoint_sources, &search);
    assert_eq!(
        endpoint_selected_complete_source_ordinals.as_ref(),
        ENDPOINT_SELECTION
    );
    assert_eq!(
        selection
            .complete_endpoint_rule
            .source_combination()
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        ENDPOINT_SELECTION
    );
    assert_raw_rule_metrics(
        &selection.complete_endpoint_rule,
        (6, 32, 82),
        (6, 30, 1, 32, 2, 89, 22),
    );
    assert_same_rule_semantics(&selection.complete_endpoint_rule, endpoint.rule());
    assert_replay_for_rule(
        &context,
        &selection.complete_endpoint_sources,
        &selection.complete_endpoint_rule,
        UNDOTTED_PATH_SECTOR,
        (6, 30, 1, 32, 2, 89),
    );

    assert_eq!(endpoint.sources().len(), ENDPOINT_SELECTION.len());
    assert_eq!(selected_ordinals(&endpoint), [0, 1, 2, 3, 4, 5]);
    assert_provenance_at_complete_ordinals(
        endpoint.sources(),
        &[0, 1, 2, 3, 4, 5],
        &[
            ([0, -1, 0, 0, 0, 0], "ordinary-ibp:1:0"),
            ([0, -1, 0, 0, 0, 0], "ordinary-ibp:1:1"),
            ([0, -1, 0, 0, 0, 0], "ordinary-ibp:2:0"),
            ([0, -1, 0, 0, 0, 0], "ordinary-ibp:2:1"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:1"),
        ],
    );

    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let complete_endpoint_projection = residual_projection(&selection.complete_endpoint_sources);
    assert_eq!(
        complete_endpoint_projection.domain().bounds(),
        scalar_face_bounds(0, 0)
    );
    assert_eq!(
        complete_endpoint_projection.fixed_restrictions(),
        fixed_endpoint()
    );
    let endpoint_stabilizer = canonicalizer
        .group_elements()
        .enumerate()
        .filter_map(|(ordinal, mapping)| {
            let image =
                std::array::from_fn::<_, 6, _>(|target| UNDOTTED_PATH_SECTOR[mapping[target]]);
            (image == UNDOTTED_PATH_SECTOR).then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        complete_endpoint_projection.stabilizer_group_elements(),
        endpoint_stabilizer
    );
    assert_eq!(
        complete_endpoint_projection.original_relations().len(),
        7 * 9
    );
    assert_eq!(complete_endpoint_projection.term_projections().len(), 7 * 9);
    let endpoint_projection = residual_projection(endpoint.sources());
    assert_eq!(
        endpoint_projection.domain().bounds(),
        scalar_face_bounds(0, 0)
    );
    assert_eq!(endpoint_projection.fixed_restrictions(), fixed_endpoint());
    assert_eq!(
        endpoint_projection.stabilizer_group_elements(),
        endpoint_stabilizer
    );
    assert_eq!(
        endpoint_projection.original_relations().len(),
        ENDPOINT_SELECTION.len()
    );
    assert_eq!(
        endpoint_projection.term_projections().len(),
        ENDPOINT_SELECTION.len()
    );
    let zero_sectors = super::super::super::exact_zero_sectors(&canonicalizer).unwrap();
    assert!(
        selection
            .complete_endpoint_sources
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );
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

    assert_eq!(selection.complete_free_sources.len(), 7 * 9);
    assert_complete_provenance(&selection.complete_free_sources, &search);
    let complete_projection = residual_projection(&selection.complete_free_sources);
    assert_eq!(
        complete_projection.domain().bounds(),
        scalar_face_bounds(i64::MIN + 2, -1)
    );
    assert_eq!(complete_projection.fixed_restrictions(), fixed_free_face());
    assert_eq!(complete_projection.stabilizer_group_elements(), [0]);
    assert_eq!(complete_projection.original_relations().len(), 7 * 9);
    assert_eq!(complete_projection.term_projections().len(), 7 * 9);

    assert_eq!(
        machine_safe_complete_source_ordinals.as_ref(),
        MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS
    );
    let excluded = (0..7 * 9)
        .filter(|ordinal| {
            MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS
                .binary_search(ordinal)
                .is_err()
        })
        .collect::<Vec<_>>();
    assert_eq!(excluded, UNSAFE_COMPLETE_SOURCE_ORDINALS);
    assert_machine_safety_partition(
        complete_projection,
        &MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS,
        &UNSAFE_COMPLETE_SOURCE_ORDINALS,
    );

    assert_eq!(selection.machine_safe_sources.len(), 59);
    assert_provenance_is_complete_subsequence(
        &selection.machine_safe_sources,
        &search,
        &MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS,
    );
    let safe_projection = residual_projection(&selection.machine_safe_sources);
    assert_eq!(
        safe_projection.domain().bounds(),
        scalar_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(safe_projection.fixed_restrictions(), fixed_free_face());
    assert_eq!(safe_projection.stabilizer_group_elements(), [0]);
    assert_eq!(safe_projection.original_relations().len(), 59);
    assert_eq!(safe_projection.term_projections().len(), 59);
    assert_eq!(
        selection
            .machine_safe_rule
            .source_combination()
            .iter()
            .map(|contribution| contribution.source_ordinal())
            .collect::<Vec<_>>(),
        BULK_SELECTION_IN_SAFE_SPAN
    );
    assert_eq!(
        bulk_selected_complete_source_ordinals.as_ref(),
        BULK_SELECTION_IN_COMPLETE_SPAN
    );
    for (&relative, &complete) in BULK_SELECTION_IN_SAFE_SPAN
        .iter()
        .zip(&BULK_SELECTION_IN_COMPLETE_SPAN)
    {
        assert_eq!(MACHINE_SAFE_COMPLETE_SOURCE_ORDINALS[relative], complete);
    }

    assert_eq!(bulk.sources().len(), 5);
    assert_eq!(selected_ordinals(&bulk), [0, 1, 2, 3, 4]);
    assert_provenance_at_complete_ordinals(
        bulk.sources(),
        &[0, 1, 2, 3, 4],
        &[
            ([0, 0, 0, -1, 0, 0], "ordinary-ibp:0:0"),
            ([0, 0, 0, -1, 0, 0], "ordinary-ibp:1:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:0:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:1:0"),
            ([0, 0, 0, 0, 0, 0], "ordinary-ibp:2:0"),
        ],
    );
    let bulk_projection = residual_projection(bulk.sources());
    assert_eq!(
        bulk_projection.domain().bounds(),
        scalar_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(bulk_projection.fixed_restrictions(), fixed_free_face());
    assert_eq!(bulk_projection.stabilizer_group_elements(), [0]);
    assert_eq!(bulk_projection.original_relations().len(), 5);
    assert_eq!(bulk_projection.term_projections().len(), 5);
    assert!(
        selection
            .complete_free_sources
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
    );
    assert!(
        selection
            .machine_safe_sources
            .verify_residual_projection(
                &context,
                &canonicalizer,
                &zero_sectors,
                RuleCellLimits::default(),
            )
            .unwrap()
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

    assert_rule(
        &endpoint,
        UNDOTTED_PATH_SECTOR,
        &ENDPOINT_RHS,
        (6, 10, 60),
        (6, 30, 1, 32, 2, 89, 22),
    );
    assert_rule(
        &bulk,
        UNDOTTED_BULK_REPLAY_ANCHOR,
        &BULK_RHS,
        (5, 11, 56),
        (5, 28, 2, 31, 1, 86, 24),
    );
    assert_eq!(guard_expressions(&endpoint), ["1-d", "-1+d"]);
    assert_eq!(guard_expressions(&bulk), ["-1+d-n3"]);
    assert_dimension_guards_have_constant_nonzero_lead(&context, &endpoint);
    assert_dimension_guards_have_constant_nonzero_lead(&context, &bulk);
    assert_eq!(
        endpoint.application_domain().bounds(),
        scalar_face_bounds(0, 0)
    );
    assert_eq!(endpoint.fixed_restrictions(), fixed_endpoint());
    assert_eq!(
        bulk.application_domain().bounds(),
        scalar_face_bounds(i64::MIN + 1, -1)
    );
    assert_eq!(bulk.fixed_restrictions(), fixed_free_face());
    assert!(endpoint.terms().iter().all(|term| term.descent().verify()));
    assert!(bulk.terms().iter().all(|term| term.descent().verify()));

    assert_replay_at(
        &context,
        &endpoint,
        UNDOTTED_PATH_SECTOR,
        (6, 30, 1, 32, 2, 89),
    );
    for free in [-1, -2, -7, i64::MIN + 1] {
        let assignment = [0, 0, 1, free, 1, 1];
        assert!(bulk.guards().iter().all(|guard| {
            !context
                .specialize_polynomial(guard.polynomial(), &assignment, Default::default())
                .unwrap()
                .is_zero()
        }));
        assert_replay_at(&context, &bulk, assignment, (5, 28, 2, 31, 1, 86));
    }

    assert_ownership_endpoints(&endpoint, &bulk);
    assert_terminal_and_same_lane_descendants(&endpoint, &bulk, &canonicalizer);
    assert_s4_orbit_boundary(&endpoint, &bulk, &canonicalizer);
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

fn assert_provenance_is_complete_subsequence(
    sources: &SourceViewBatch,
    search: &SectorSearchDiamond,
    complete_ordinals: &[usize],
) {
    assert_eq!(sources.len(), complete_ordinals.len());
    for (provenance, &complete_ordinal) in sources.provenance().iter().zip(complete_ordinals) {
        let offset = &search.offsets()[complete_ordinal / ORDINARY_ROWS.len()];
        let row = ORDINARY_ROWS[complete_ordinal % ORDINARY_ROWS.len()];
        assert_eq!(provenance.translated().offset(), offset);
        assert_eq!(provenance.translated().source_row().stable_string(), row);
        assert!(provenance.symmetry().is_none());
    }
}

fn assert_provenance_at_complete_ordinals(
    sources: &SourceViewBatch,
    source_ordinals: &[usize],
    expected: &[([i64; 6], &str)],
) {
    assert_eq!(source_ordinals.len(), expected.len());
    for (&ordinal, (offset, row)) in source_ordinals.iter().zip(expected) {
        let provenance = &sources.provenance()[ordinal];
        assert_eq!(provenance.translated().offset().values(), offset);
        assert_eq!(provenance.translated().source_row().stable_string(), *row);
        assert!(provenance.symmetry().is_none());
    }
}

fn residual_projection(
    sources: &SourceViewBatch,
) -> &crate::foundry::cell::ResidualProjectionEvidence {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        panic!("undotted path sources must retain residual projection evidence")
    };
    evidence
}

fn assert_machine_safety_partition(
    evidence: &crate::foundry::cell::ResidualProjectionEvidence,
    safe: &[usize],
    unsafe_ordinals: &[usize],
) {
    let route_is_machine_safe = |ordinal: usize| {
        evidence.term_projections()[ordinal]
            .iter()
            .all(|term| match term.disposition() {
                ResidualTermDisposition::Routed {
                    projected_shift, ..
                } => scalar_face_bounds(i64::MIN + 1, -1)
                    .iter()
                    .zip(projected_shift.iter())
                    .all(|(bounds, &delta)| {
                        bounds.lower().checked_add(delta).is_some()
                            && bounds.upper().checked_add(delta).is_some()
                    }),
                ResidualTermDisposition::CoefficientZero
                | ResidualTermDisposition::ProvedZero { .. } => true,
            })
    };
    assert!(safe.iter().all(|&ordinal| route_is_machine_safe(ordinal)));
    assert!(
        unsafe_ordinals
            .iter()
            .all(|&ordinal| !route_is_machine_safe(ordinal))
    );
    for &ordinal in unsafe_ordinals {
        let unsafe_shifts = evidence.term_projections()[ordinal]
            .iter()
            .filter_map(|term| match term.disposition() {
                ResidualTermDisposition::Routed {
                    projected_shift, ..
                } if scalar_face_bounds(i64::MIN + 1, -1)
                    .iter()
                    .zip(projected_shift.iter())
                    .any(|(bounds, &delta)| {
                        bounds.lower().checked_add(delta).is_none()
                            || bounds.upper().checked_add(delta).is_none()
                    }) =>
                {
                    Some(projected_shift.to_vec())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!unsafe_shifts.is_empty());
        assert!(unsafe_shifts.iter().all(|shift| shift[3] <= -2));
    }
}

fn selected_ordinals(cell: &RuleCell) -> Vec<usize> {
    cell.rule()
        .source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect()
}

fn assert_same_rule_semantics(complete: &ParametricRule, compact: &ParametricRule) {
    assert_eq!(complete.anchor(), compact.anchor());
    assert_eq!(complete.ordering(), compact.ordering());
    assert_eq!(complete.pivot(), compact.pivot());
    assert_eq!(
        complete.right_hand_side().len(),
        compact.right_hand_side().len()
    );
    for (complete_term, compact_term) in complete
        .right_hand_side()
        .iter()
        .zip(compact.right_hand_side())
    {
        assert_eq!(complete_term.shift(), compact_term.shift());
        assert_eq!(complete_term.coefficient(), compact_term.coefficient());
    }
    assert_eq!(
        complete.pivot_guard().nonzero_polynomial(),
        compact.pivot_guard().nonzero_polynomial()
    );
    assert_eq!(
        complete
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>(),
        compact
            .nonzero_guards()
            .iter()
            .map(|guard| guard.polynomial())
            .collect::<Vec<_>>()
    );
}

fn assert_rule(
    cell: &RuleCell,
    anchor: [i64; 6],
    rhs: &[[i64; 6]],
    parametric: (usize, usize, usize),
    concrete: (usize, usize, usize, usize, usize, usize, usize),
) {
    assert_eq!(cell.rule().anchor().powers(), anchor);
    assert_eq!(cell.rule().pivot().values(), UNDOTTED_PATH_NUMERATOR_PIVOT);
    assert_eq!(
        cell.rule()
            .right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        rhs.iter().map(|shift| shift.as_slice()).collect::<Vec<_>>()
    );
    assert_raw_rule_metrics(cell.rule(), parametric, concrete);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
}

fn assert_raw_rule_metrics(
    rule: &ParametricRule,
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
    assert_replay_for_rule(context, cell.sources(), cell.rule(), assignment, metrics);
}

fn assert_replay_for_rule(
    context: &IndexedCoefficientContext,
    sources: &SourceViewBatch,
    rule: &ParametricRule,
    assignment: [i64; 6],
    metrics: (usize, usize, usize, usize, usize, usize),
) {
    let replay = replay_rule_at_concrete_assignment(
        context,
        sources.relations(),
        rule,
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

fn assert_ownership_endpoints(endpoint: &RuleCell, bulk: &RuleCell) {
    let target = |free| IntegralKey::try_new([0, 0, 1, free, 1, 1]).unwrap();
    assert_eq!(
        bulk.assignment_for_target(&target(i64::MIN)).unwrap(),
        Some(vec![0, 0, 1, i64::MIN + 1, 1, 1])
    );
    assert_eq!(
        bulk.assignment_for_target(&target(-2)).unwrap(),
        Some(vec![0, 0, 1, -1, 1, 1])
    );
    assert!(bulk.assignment_for_target(&target(-1)).unwrap().is_none());
    assert!(
        endpoint
            .assignment_for_target(&target(-2))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        endpoint.assignment_for_target(&target(-1)).unwrap(),
        Some(UNDOTTED_PATH_SECTOR.to_vec())
    );
    assert!(
        endpoint
            .assignment_for_target(&target(0))
            .unwrap()
            .is_none()
    );
}

fn assert_terminal_and_same_lane_descendants(
    endpoint: &RuleCell,
    bulk: &RuleCell,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
) {
    let terminals = K6ReachabilityTerminals::try_new().unwrap();
    let endpoint_children = canonical_children(canonicalizer, endpoint, &UNDOTTED_PATH_SECTOR);
    assert_eq!(endpoint_children, [vec![0, 0, 1, 0, 1, 1]]);
    assert!(matches!(
        terminals.classify(&key(&endpoint_children[0])),
        Some(terminal)
            if terminal.kind() == ReachabilityTerminalKind::Factorization
                && terminal.owner_ordinal() == 2
    ));

    for free in [-1, -7, i64::MIN + 1] {
        let assignment = [0, 0, 1, free, 1, 1];
        assert_eq!(
            canonical_children(canonicalizer, bulk, &assignment),
            [vec![0, 0, 1, free, 1, 1], vec![0, 0, 1, free + 1, 1, 1]]
        );
    }
}

fn assert_s4_orbit_boundary(
    endpoint: &RuleCell,
    bulk: &RuleCell,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
) {
    for free in [-1, -2, -7, i64::MIN] {
        let canonical_lane = IntegralKey::try_new([0, 0, 1, free, 1, 1]).unwrap();
        let equivalent_inactive_slot = IntegralKey::try_new([0, free, 1, 0, 1, 1]).unwrap();
        let separate_orbit = IntegralKey::try_new([free, 0, 1, 0, 1, 1]).unwrap();
        let canonical = canonicalizer.canonicalize(&canonical_lane).unwrap();
        let equivalent = canonicalizer
            .canonicalize(&equivalent_inactive_slot)
            .unwrap();
        let separate = canonicalizer.canonicalize(&separate_orbit).unwrap();
        assert_eq!(canonical.canonical(), equivalent.canonical());
        assert_eq!(canonical.canonical().powers(), canonical_lane.powers());
        assert_ne!(canonical.canonical(), separate.canonical());
        assert_eq!(
            canonicalizer.orbit(&canonical_lane).unwrap().orbit_size(),
            24
        );
        assert_eq!(
            canonicalizer.orbit(&separate_orbit).unwrap().orbit_size(),
            12
        );
        assert!(
            bulk.assignment_for_target(&separate_orbit)
                .unwrap()
                .is_none()
        );
        assert!(
            endpoint
                .assignment_for_target(&separate_orbit)
                .unwrap()
                .is_none()
        );
    }
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
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, upper),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}
