//! End-to-end validation for residual-affine prepare-point re-elimination.
//!
//! The equal-mass sunset below is only a compact concrete source of generated
//! IBP/LI rows and nontrivial residual-affine branches.  The tests never
//! encode a recurrence: every submitted equation is selected from the row
//! span authenticated by generated sector discovery.

use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::{
    AffineDenominator, AffineParametricOrderingLimits, AffinePreparePointScheduleCertificate,
    AffinePreparePointScheduleLimits, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, CoefficientContext,
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationCompiler,
    GeneratedResidualAffineBranchBoundRelationLimits,
    GeneratedResidualAffineBranchConcreteReplayLimits,
    GeneratedResidualAffineBranchReeliminationCertificate,
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationCompiler,
    GeneratedResidualAffineBranchReeliminationError,
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedResidualAffineBranchReeliminationRowOutcome,
    GeneratedResidualAffineBranchReeliminationRowWitness, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricIbpGenerator,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemLimits,
    ResidualAffineBranchSystemOutcome, ResidualProductLocusBooleanCoverCertificate,
    ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverLimits,
    ResidualProductLocusBooleanNodeOutcome, SectorMask,
};
use symbolica::prelude::Integer;

fn sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

struct Candidate {
    family: IntegralFamily,
    context: ParametricCoefficientContext,
    cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    schedule: Arc<AffinePreparePointScheduleCertificate>,
}

fn candidates(bits: &str, through_depth: usize) -> Vec<Candidate> {
    let family = sunset(&format!(
        "generated-residual-affine-reelimination-sunset-{bits}"
    ));
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        SectorMask::try_from_bit_string(bits).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue = Arc::new(
        GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, queue_limits)
            .unwrap(),
    );
    let cover = Arc::new(
        ResidualProductLocusBooleanCoverCompiler::compile(
            &family,
            &context,
            queue,
            0,
            ResidualProductLocusBooleanCoverLimits::default(),
        )
        .unwrap(),
    );

    let mut result = Vec::new();
    for terminal_ordinal in cover.nodes().iter().filter_map(|node| {
        matches!(
            node.outcome(),
            ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
        )
        .then_some(node.ordinal())
    }) {
        let branch = Arc::new(
            ResidualAffineBranchSystemCertificate::compile(
                &family,
                &context,
                cover.clone(),
                terminal_ordinal,
                ResidualAffineBranchSystemLimits::default(),
            )
            .unwrap(),
        );
        if !matches!(
            branch.outcome(),
            ResidualAffineBranchSystemOutcome::GuardedAffineMap
        ) {
            continue;
        }
        let guards = Arc::new(
            ResidualAffineBranchGuardCompositionCertificate::compile(
                &family,
                &context,
                cover.clone(),
                branch.clone(),
                ResidualAffineBranchGuardCompositionLimits::default(),
            )
            .unwrap(),
        );
        if guards.has_contradiction() {
            continue;
        }
        let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
            &family,
            &context,
            cover.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            branch.clone(),
            AffineParametricOrderingLimits::default(),
        )
        .unwrap();
        let authority = AffineStartReplayAuthority::ResidualBooleanBranch {
            family: &family,
            context: &context,
            cover: &cover,
        };
        let schedule = Arc::new(
            AffinePreparePointScheduleCertificate::compile_with_authority(
                authority,
                ordering,
                through_depth,
                AffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        result.push(Candidate {
            family: family.clone(),
            context: context.clone(),
            cover: cover.clone(),
            branch,
            guards,
            schedule,
        });
    }
    result
}

macro_rules! assert_bound_source {
    ($bound:expr, $candidate:expr, $witness:expr) => {{
        let span = $candidate.cover.source_queue().discovery().row_span_arc();
        assert!(Arc::ptr_eq($bound.row_span(), span));
        assert!(Arc::ptr_eq($bound.branch(), &$candidate.branch));
        assert!(Arc::ptr_eq($bound.branch_guards(), &$candidate.guards));
        assert_eq!($bound.source_row_ordinal(), $witness.source_row_ordinal());
        assert_eq!($bound.translation(), $witness.translation());
    }};
}

fn assert_witness_matches_direct_row(
    candidate: &Candidate,
    witness: &GeneratedResidualAffineBranchReeliminationRowWitness,
    per_row: GeneratedResidualAffineBranchBoundRelationLimits,
) {
    let direct = GeneratedResidualAffineBranchBoundRelationCompiler::compile(
        &candidate.family,
        &candidate.context,
        witness.source_row_ordinal(),
        witness.translation().clone(),
        candidate.branch.clone(),
        candidate.guards.clone(),
        per_row,
    )
    .unwrap();

    match (witness.outcome(), direct) {
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(actual),
            GeneratedResidualAffineBranchBoundRelationCompilation::Retained(expected),
        ) => {
            assert_bound_source!(actual, candidate, witness);
            assert_eq!(actual.target_row_id(), expected.target_row_id());
            assert_eq!(actual.relation_manifest(), expected.relation_manifest());
            assert_eq!(actual.base_assumptions(), expected.base_assumptions());
            assert_eq!(actual.condition_witnesses(), expected.condition_witnesses());
            assert_eq!(actual.stats(), expected.stats());
        }
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(actual),
            GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(expected),
        ) => {
            assert_bound_source!(actual, candidate, witness);
            assert_eq!(actual.target_row_id(), expected.target_row_id());
            assert_eq!(actual.reason(), expected.reason());
            assert_eq!(actual.base_assumptions(), expected.base_assumptions());
            assert_eq!(
                actual.private_free_index_guards(),
                expected.private_free_index_guards()
            );
            assert_eq!(actual.condition_witnesses(), expected.condition_witnesses());
            assert_eq!(actual.stats(), expected.stats());
        }
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(actual),
            GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(expected),
        ) => {
            assert_bound_source!(actual, candidate, witness);
            assert_eq!(actual.target_row_id(), expected.target_row_id());
            assert_eq!(actual.reason(), expected.reason());
            assert_eq!(actual.stats(), expected.stats());
        }
        (actual, expected) => panic!(
            "expanded witness outcome does not match direct one-row compilation: {actual:?} versus {expected:?}"
        ),
    }
}

fn first_eliminated(
    bits: &str,
    through_depth: usize,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> (
    Candidate,
    GeneratedResidualAffineBranchReeliminationCertificate,
) {
    for candidate in candidates(bits, through_depth) {
        let compilation = GeneratedResidualAffineBranchReeliminationCompiler::compile(
            &candidate.family,
            &candidate.context,
            candidate.schedule.clone(),
            candidate.guards.clone(),
            limits,
        )
        .unwrap();
        if let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(certificate) =
            compilation
        {
            return (candidate, certificate);
        }
    }
    panic!("sector {bits} has no eliminated residual-affine branch at depth {through_depth}");
}

fn compile_candidate(
    candidate: &Candidate,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationError,
> {
    GeneratedResidualAffineBranchReeliminationCompiler::compile(
        &candidate.family,
        &candidate.context,
        candidate.schedule.clone(),
        candidate.guards.clone(),
        limits,
    )
}

fn affine_ambient_point(
    branch: &ResidualAffineBranchSystemCertificate,
    free_values: &[i64],
) -> Option<Vec<i64>> {
    let map = branch.affine_map()?;
    if free_values.len() != map.free_positions().len() {
        return None;
    }
    (0..map.ambient_arity())
        .map(|row| {
            let mut value = map.constant(row)?.clone();
            for (free_ordinal, &free_position) in map.free_positions().iter().enumerate() {
                value += map.linear_coefficient(row, free_position)?
                    * Integer::from(free_values[free_ordinal]);
            }
            value.to_i64()
        })
        .collect()
}

fn centered_free_points(free_count: usize, radius: i64) -> Vec<Vec<i64>> {
    let mut coordinates = vec![0];
    for magnitude in 1..=radius {
        coordinates.push(magnitude);
        coordinates.push(-magnitude);
    }
    let point_count = coordinates
        .len()
        .pow(u32::try_from(free_count).expect("the sunset free-coordinate count fits u32"));
    (0..point_count)
        .map(|mut ordinal| {
            (0..free_count)
                .map(|_| {
                    let value = coordinates[ordinal % coordinates.len()];
                    ordinal /= coordinates.len();
                    value
                })
                .collect()
        })
        .collect()
}

#[test]
fn generated_sunset_exposes_authenticated_affine_reelimination_inputs() {
    let candidates = candidates("111", 1);
    assert!(!candidates.is_empty());
    for candidate in candidates {
        assert!(Arc::ptr_eq(
            candidate.schedule.ordering().residual_branch().unwrap(),
            &candidate.branch
        ));
        assert!(Arc::ptr_eq(
            candidate.guards.source_branch(),
            &candidate.branch
        ));
        assert!(Arc::ptr_eq(
            candidate.guards.source_cover(),
            &candidate.cover
        ));
        assert_eq!(candidate.schedule.layers().len(), 2);
        candidate
            .guards
            .replay(&candidate.family, &candidate.context)
            .unwrap();
        candidate
            .schedule
            .replay_with_authority(AffineStartReplayAuthority::ResidualBooleanBranch {
                family: &candidate.family,
                context: &candidate.context,
                cover: &candidate.cover,
            })
            .unwrap();
    }
}

#[test]
fn point_major_generated_rows_match_direct_compilation_and_affine_column_order() {
    let (candidate, certificate) = first_eliminated(
        "111",
        1,
        GeneratedResidualAffineBranchReeliminationLimits::default(),
    );
    assert!(Arc::ptr_eq(certificate.schedule(), &candidate.schedule));
    assert!(Arc::ptr_eq(certificate.branch(), &candidate.branch));
    assert!(Arc::ptr_eq(certificate.branch_guards(), &candidate.guards));
    assert_eq!(
        certificate.ordering_identity(),
        candidate.schedule.ordering().stable_manifest()
    );

    let span = candidate.cover.source_queue().discovery().row_span_arc();
    let mut expanded_ordinal = 0usize;
    let mut retained = 0usize;
    let mut unavailable = 0usize;
    let mut expected_support = BTreeSet::new();
    let mut expected_row_local_assumptions = 0usize;
    let mut expected_row_local_origins = 0usize;

    for (layer_ordinal, layer) in candidate.schedule.layers().iter().enumerate() {
        for (prepare_point_ordinal, translation) in layer.ordered_translations().iter().enumerate()
        {
            for source_row_ordinal in 0..span.rows().len() {
                let witness = &certificate.witnesses()[expanded_ordinal];
                assert_eq!(witness.expanded_ordinal(), expanded_ordinal);
                assert_eq!(witness.layer_ordinal(), layer_ordinal);
                assert_eq!(witness.depth(), layer.depth());
                assert_eq!(witness.prepare_point_ordinal(), prepare_point_ordinal);
                assert_eq!(witness.source_row_ordinal(), source_row_ordinal);
                assert_eq!(witness.translation(), translation);
                assert_witness_matches_direct_row(
                    &candidate,
                    witness,
                    certificate.limits().per_row,
                );

                match witness.outcome() {
                    GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(bound) => {
                        retained += 1;
                        let support = witness
                            .retained_support_shifts()
                            .expect("a retained row has a public support transcript");
                        assert!(!support.is_empty());
                        expected_support.extend(support.iter().cloned());
                        expected_row_local_assumptions += bound.base_assumptions().len();
                        expected_row_local_origins += bound
                            .base_assumptions()
                            .iter()
                            .map(|assumption| assumption.condition().origins().len())
                            .sum::<usize>();
                    }
                    GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(_) => {
                        unavailable += 1;
                        assert!(witness.retained_support_shifts().is_none());
                    }
                    GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(_) => {
                        panic!("an eliminated database cannot contain an empty-branch outcome")
                    }
                }
                expanded_ordinal += 1;
            }
        }
    }
    assert_eq!(expanded_ordinal, certificate.witnesses().len());
    assert_eq!(retained, certificate.retained_row_count());
    assert_eq!(retained, certificate.stats().retained_rows());
    assert_eq!(unavailable, certificate.stats().unavailable_rows());
    assert_eq!(certificate.stats().empty_outcomes(), 0);
    assert_eq!(
        certificate.stats().row_local_base_assumptions(),
        expected_row_local_assumptions
    );
    assert_eq!(
        certificate.stats().row_local_base_assumption_origins(),
        expected_row_local_origins
    );

    let expected_common = candidate
        .guards
        .entries()
        .iter()
        .filter_map(|entry| entry.class().condition())
        .collect::<Vec<_>>();
    assert_eq!(
        certificate.common_premises().collect::<Vec<_>>(),
        expected_common
    );
    assert_eq!(
        certificate.stats().common_branch_premises(),
        expected_common.len()
    );

    let ordering = candidate.schedule.ordering();
    let mut expected_columns = expected_support.into_iter().collect::<Vec<_>>();
    expected_columns.sort_by(|left, right| {
        ordering
            .key_for_shift(left)
            .unwrap()
            .cmp(&ordering.key_for_shift(right).unwrap())
            .then_with(|| left.cmp(right))
    });
    assert_eq!(certificate.columns_easiest_first(), expected_columns);
    assert_eq!(certificate.stats().columns(), expected_columns.len());
    assert_eq!(
        certificate.pivot_count() + certificate.free_column_count(),
        expected_columns.len()
    );
    certificate
        .replay(&candidate.family, &candidate.context)
        .unwrap();

    let foreign_family = sunset("generated-residual-affine-reelimination-foreign-family");
    assert!(matches!(
        certificate.replay(&foreign_family, &candidate.context),
        Err(GeneratedResidualAffineBranchReeliminationError::WrongFamily)
    ));
    let foreign_context = ParametricCoefficientContext::try_new(
        candidate.family.coefficient_context(),
        "generated-residual-affine-reelimination-foreign-context",
        candidate.context.index_count(),
    )
    .unwrap();
    assert!(matches!(
        certificate.replay(&candidate.family, &foreign_context),
        Err(GeneratedResidualAffineBranchReeliminationError::WrongContext)
    ));
}

#[test]
fn cumulative_limits_accept_the_exact_census_and_reject_one_below() {
    let (candidate, baseline) = first_eliminated(
        "111",
        0,
        GeneratedResidualAffineBranchReeliminationLimits::default(),
    );
    let stats = baseline.stats();
    let mut exact = GeneratedResidualAffineBranchReeliminationLimits::default();
    exact.max_schedule_layers = stats.schedule_layers();
    exact.max_prepare_points = stats.prepare_points();
    exact.max_source_rows = stats.source_rows();
    exact.max_expanded_rows = stats.scheduled_expanded_rows();
    exact.max_row_witnesses = stats.row_witnesses();
    exact.max_translation_components = stats.translation_components();
    exact.max_retained_rows = stats.retained_rows();
    exact.max_unavailable_rows = stats.unavailable_rows();
    exact.max_witness_support_components = stats.witness_support_components();
    exact.max_cumulative_row_algebra_work = stats.cumulative_row_algebra_work();
    exact.max_cumulative_row_integer_bit_work = stats.cumulative_row_integer_bit_work();
    exact.max_cumulative_row_normalization_input_term_pairs =
        stats.cumulative_row_normalization_input_term_pairs();
    exact.max_cumulative_row_guard_origin_bytes = stats.cumulative_row_guard_origin_bytes();
    exact.max_cumulative_row_retained_terms = stats.cumulative_row_retained_terms();
    exact.max_cumulative_row_retained_bytes = stats.cumulative_row_retained_bytes();
    exact.max_row_local_base_assumptions = stats.row_local_base_assumptions();
    exact.max_row_local_base_assumption_origins = stats.row_local_base_assumption_origins();
    exact.max_elimination_input_terms = stats.elimination_input_terms();
    exact.max_elimination_input_guards = stats.elimination_input_guards();
    exact.max_elimination_input_guard_origins = stats.elimination_input_guard_origins();
    exact.max_columns = stats.columns();
    exact.max_column_key_components = stats.column_key_components();
    exact.max_column_key_integer_bits = stats.column_key_integer_bits();
    exact.max_ordering_identity_bytes = stats.ordering_identity_bytes();
    let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(exact_certificate) =
        compile_candidate(&candidate, exact).unwrap()
    else {
        panic!("the exact cumulative census changed the baseline outcome")
    };
    assert_eq!(exact_certificate.stats(), stats);

    macro_rules! one_below {
        ($field:ident, $getter:ident, $resource:literal) => {{
            let used = stats.$getter();
            assert!(used > 0, "{} must have a nonzero census", $resource);
            let mut limits = exact;
            limits.$field = used - 1;
            assert!(matches!(
                compile_candidate(&candidate, limits),
                Err(GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                }) if resource == $resource && requested == used && limit == used - 1
            ));
        }};
    }

    one_below!(
        max_expanded_rows,
        scheduled_expanded_rows,
        "affine branch expanded rows"
    );
    one_below!(
        max_row_witnesses,
        row_witnesses,
        "affine branch row witnesses"
    );
    one_below!(
        max_translation_components,
        translation_components,
        "affine branch translation components"
    );
    one_below!(
        max_cumulative_row_algebra_work,
        cumulative_row_algebra_work,
        "affine branch cumulative row algebra work"
    );
    one_below!(
        max_cumulative_row_integer_bit_work,
        cumulative_row_integer_bit_work,
        "affine branch cumulative row integer-bit work"
    );
    one_below!(
        max_cumulative_row_retained_terms,
        cumulative_row_retained_terms,
        "affine branch retained row terms"
    );
    one_below!(
        max_cumulative_row_retained_bytes,
        cumulative_row_retained_bytes,
        "affine branch retained row bytes"
    );
    one_below!(
        max_elimination_input_terms,
        elimination_input_terms,
        "affine branch elimination input terms"
    );
    one_below!(max_columns, columns, "affine branch columns");
    one_below!(
        max_column_key_integer_bits,
        column_key_integer_bits,
        "affine branch column-key integer bits"
    );
    one_below!(
        max_ordering_identity_bytes,
        ordering_identity_bytes,
        "affine branch ordering identity bytes"
    );
}

#[test]
fn retained_rows_and_pivot_traces_replay_at_three_valid_affine_points() {
    let (candidate, certificate) = first_eliminated(
        "011",
        0,
        GeneratedResidualAffineBranchReeliminationLimits::default(),
    );
    let free_count = candidate
        .branch
        .affine_map()
        .expect("a guarded affine branch has an exact map")
        .free_positions()
        .len();
    let limits = GeneratedResidualAffineBranchConcreteReplayLimits::default();
    let mut checked = 0usize;
    for free_values in centered_free_points(free_count, 12) {
        let Some(ambient) = affine_ambient_point(&candidate.branch, &free_values) else {
            continue;
        };
        if !candidate
            .branch
            .guarded_affine_map_applies_at_original_indices(&candidate.context, &ambient)
            .unwrap()
        {
            continue;
        }
        let Ok(stats) = certificate.replay_at_free_values(&candidate.context, &free_values, limits)
        else {
            continue;
        };
        assert_eq!(stats.source_rows(), certificate.retained_row_count());
        assert_eq!(stats.pivots(), certificate.pivot_count());
        assert_eq!(
            stats.specialized_relations(),
            stats.source_rows() + stats.pivots()
        );
        assert!(stats.specialized_terms() > 0);
        checked += 1;
        if checked == 3 {
            break;
        }
    }
    assert_eq!(checked, 3, "expected three valid concrete affine probes");
}
