//! A saved single-sector candidate is transport, not a complete downset.
use super::*;
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::solver::{
    CandidateReductionError, SectorConfig, SectorSolution, SectorSolveOptions, SectorSolver,
};

fn solve<const N: usize>(
    request: &FamilyCandidatesRequest,
    sector: [bool; N],
) -> (IntegralFamily, SectorSolution<N>) {
    let family = preparation::family(&request.source, request.input_format).unwrap();
    let root = preparation::root(N, &request.nonpositive_indices).unwrap();
    let prepared =
        preparation::prepare::<N>(family, &root, request.permutation.as_deref()).unwrap();
    let solver = SectorSolver::new(
        &prepared.sources,
        sector,
        SectorConfig {
            zero_sectors: prepared.zeros.clone(),
            permutation: prepared.permutation,
            symbolic_exact_backend: request.exact_backend.solver_backend(),
            numerical_exact_backend: request.exact_backend.numerical_backend(),
            ..Default::default()
        },
    )
    .unwrap();
    let solution = solver
        .solve_sector(SectorSolveOptions {
            numerical_depth: request.numerical_depth,
            max_numerator_rank: request.max_numerator_rank,
            finite_case_policy: request.finite_case_policy,
            finite_case_limits: request.finite_case_limits,
            case_intersection_limits: request.case_intersection_limits,
            ..Default::default()
        })
        .unwrap();
    (prepared.family, solution)
}

#[test]
fn public_single_sector_export_matches_normal_native_transport_and_application() {
    let request = FamilyCandidatesRequest::new(K1);
    let (family, solution) = solve(&request, [true]);
    let bytes =
        crate::encode_generated_candidate_sector(&request, &family, [true], &solution).unwrap();
    let generated = family_candidates(request).unwrap();
    assert_same_program(&bytes, generated.bundle());
    let inspected = inspect_generated_candidate_bundle(&bytes, Default::default()).unwrap();
    assert_eq!(inspected.schema, CANDIDATE_BUNDLE_SCHEMA);
    assert_eq!(inspected.status, "uncertified-candidates");
    assert_eq!(inspected.solved_sectors, 1);
    let (_, mut reference) = load_generated_candidate_bundle::<1>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let (_, mut loaded) =
        load_generated_candidate_bundle::<1>(&bytes, Default::default(), Default::default())
            .unwrap();
    for power in [1, 2, 6] {
        let key = IntegralKey::try_new([power]).unwrap();
        assert_eq!(
            loaded.reduce_unit_mass(&key).unwrap().terms(),
            reference.reduce_unit_mass(&key).unwrap().terms()
        );
    }
}

#[test]
fn exported_partial_sector_preserves_order_scope_sources_and_missing_successors() {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.permutation = Some(vec![2, 0, 1]);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    let (family, solution) = solve(&request, [true; 3]);
    let bytes = encode_generated_candidate_sector(&request, &family, [true; 3], &solution).unwrap();
    let decoded = codec::read(&bytes, request.bundle_limits).unwrap();
    assert_eq!(decoded.permutation, request.permutation);
    assert_eq!(decoded.sectors.len(), 1);
    assert_eq!(decoded.root_sector, [true; 3]);
    let inspected = inspect_generated_candidate_bundle(&bytes, request.bundle_limits).unwrap();
    assert_eq!(inspected.max_numerator_rank, Some(2));
    assert_eq!(
        inspected.finite_case_policy,
        FiniteCasePolicy::RetainRankFinite
    );
    assert_eq!(inspected.finite_residuals, solution.finite_residuals.len());
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sources = rustred::solver::SourceSystem::<3>::from_family(&family).unwrap();
    let restored = codec::solutions::<3>(
        &decoded,
        &context,
        sources.index_variables(),
        request.bundle_limits,
    )
    .unwrap();
    let restored = &restored[0].1;
    assert_eq!(restored.finite_residuals, solution.finite_residuals);
    assert_eq!(restored.rules.len(), solution.rules.len());
    for (before, after) in solution.rules.iter().zip(&restored.rules) {
        assert_eq!(before.candidate.case, after.candidate.case);
        assert_eq!(before.candidate.target, after.candidate.target);
        assert_eq!(before.candidate.sources, after.candidate.sources);
        assert_eq!(before.candidate.rhs.len(), after.candidate.rhs.len());
        for (a, b) in before.candidate.rhs.iter().zip(&after.candidate.rhs) {
            assert_eq!(a.integral, b.integral);
            assert_eq!(a.coefficient, b.coefficient);
        }
        assert_eq!(before.exceptions.branches, after.exceptions.branches);
    }
    let (_, mut loaded) =
        load_generated_candidate_bundle::<3>(&bytes, request.bundle_limits, Default::default())
            .unwrap();
    // A valid massive pinch was never exported. It cannot silently become a
    // terminal merely because the parent root or rank metadata includes it.
    let missing = IntegralKey::try_new([0, 1, 1]).unwrap();
    assert!(
        matches!(loaded.reduce_unit_mass(&missing), Err(CandidateReductionError::Uncovered { target }) if target == missing)
    );
    let outside = IntegralKey::try_new([-3, 1, 1]).unwrap();
    assert!(matches!(
        loaded.reduce_unit_mass(&outside),
        Err(CandidateReductionError::OutsideNumeratorRank { .. })
    ));
}

#[test]
fn export_rejects_mislabeled_scope_source_and_transport_limits() {
    let request = FamilyCandidatesRequest::new(K1);
    let (family, solution) = solve(&request, [true]);
    let mut variants = Vec::new();
    let mut changed = request.clone();
    changed.source = K1.replace("q^2-1", "q^2-2");
    variants.push((changed, "source differs"));
    let mut changed = request.clone();
    changed.nonpositive_indices = vec![0];
    variants.push((changed, "outside request root"));
    let mut changed = request.clone();
    changed.max_numerator_rank = Some(10);
    variants.push((changed, "numerator-rank scope differs"));
    let mut changed = request.clone();
    changed.permutation = Some(vec![1]);
    variants.push((changed, "permutation"));
    let mut changed = request.clone();
    changed.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    variants.push((changed, "requires max_numerator_rank"));
    let mut changed = request.clone();
    changed.finite_case_limits.max_visited_points += 1;
    variants.push((changed, "require retain-rank-finite"));
    for (changed, expected) in variants {
        let error =
            encode_generated_candidate_sector(&changed, &family, [true], &solution).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Input);
        assert!(error.message().contains(expected), "{expected}: {error}");
    }
    let other_family = preparation::family(K3, InputFormat::Toml).unwrap();
    assert!(
        encode_generated_candidate_sector(&request, &other_family, [true], &solution)
            .unwrap_err()
            .message()
            .contains("arity mismatch")
    );
    let mut scoped = request.clone();
    scoped.max_numerator_rank = Some(2);
    let (scoped_family, scoped_solution) = solve(&scoped, [true]);
    scoped.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert!(
        encode_generated_candidate_sector(&scoped, &scoped_family, [true], &scoped_solution)
            .unwrap_err()
            .message()
            .contains("finite-case policy differs")
    );
    for coefficient_limit in [false, true] {
        let mut tiny = request.clone();
        if coefficient_limit {
            tiny.bundle_limits.max_coefficient_bytes = 1;
        } else {
            tiny.bundle_limits.max_bundle_bytes = 1;
        }
        assert_eq!(
            encode_generated_candidate_sector(&tiny, &family, [true], &solution)
                .unwrap_err()
                .kind(),
            AppErrorKind::Limit
        );
    }
}

#[test]
fn export_does_not_search_or_open_requested_checkpoint() {
    let mut request = FamilyCandidatesRequest::new(K1);
    let (family, solution) = solve(&request, [true]);
    let expected = encode_generated_candidate_sector(&request, &family, [true], &solution).unwrap();
    // Even invalid execution-only settings cannot launch a solve through an
    // already-generated transport operation.
    request.n_cores = 0;
    request.case_intersection_limits.max_work_items = 0;
    let directory = super::checkpoint::Directory::new();
    let nonexistent = directory.0.join("must-not-be-created");
    request.checkpoint = Some(CandidateCheckpointOptions::new(&nonexistent));
    let bytes = encode_generated_candidate_sector(&request, &family, [true], &solution).unwrap();
    assert_same_program(&expected, &bytes);
    assert!(!nonexistent.exists());
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 0);
}
