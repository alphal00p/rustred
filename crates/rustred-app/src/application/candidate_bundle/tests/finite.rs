//! Deliberate nonminimal leaves are distinct from failed search and closure.
use super::*;
use rustred::family::IntegralKey;

#[test]
fn finite_retention_policy_cold_loads_and_preserves_rank_boundary() {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.max_numerator_rank = Some(2);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    let generated = family_candidates(request.clone()).unwrap();
    let inspected =
        inspect_generated_candidate_bundle(generated.bundle(), Default::default()).unwrap();
    assert_eq!(inspected.finite_case_policy, request.finite_case_policy);
    assert_eq!(inspected.finite_case_limits, request.finite_case_limits);
    assert_eq!(inspected.max_numerator_rank, Some(2));
    assert!(inspected.finite_residuals > 0);
    let report: toml::Value = toml::from_str(generated.to_toml()).unwrap();
    assert_eq!(
        report["finite_case_policy"].as_str(),
        Some("retain-rank-finite")
    );
    let (_, mut loaded) = load_generated_candidate_bundle::<3>(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    for powers in [[1, 1, 1], [2, 1, 1], [0, 1, 1], [-1, 1, 1], [-2, 1, 1]] {
        let target = IntegralKey::try_new(powers).unwrap();
        let first = loaded.reduce_unit_mass(&target).unwrap();
        assert!(!first.terms().is_empty());
        assert_eq!(
            first.terms(),
            loaded.reduce_unit_mass(&target).unwrap().terms()
        );
    }
    assert!(matches!(
        loaded.reduce_unit_mass(&IntegralKey::try_new([-3, 1, 1]).unwrap()),
        Err(rustred::solver::CandidateReductionError::OutsideNumeratorRank { .. })
    ));
    assert!(
        certify_candidates(CandidateCertificationRequest::new(generated.bundle()))
            .unwrap_err()
            .message()
            .contains("rank-scoped candidates cannot be certified")
    );
    request.n_cores = 2;
    let parallel = family_candidates(request).unwrap();
    assert_same_program(generated.bundle(), parallel.bundle());
}

#[test]
fn finite_retention_rejects_invalid_request_before_parsing_or_search() {
    let mut request = FamilyCandidatesRequest::new("not parsed");
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert!(
        family_candidates(request.clone())
            .unwrap_err()
            .message()
            .contains("requires max_numerator_rank")
    );
    request.max_numerator_rank = Some(10);
    request.finite_case_limits.max_visited_points = 0;
    assert!(
        family_candidates(request.clone())
            .unwrap_err()
            .message()
            .contains("must be positive")
    );
    request.finite_case_limits = FiniteCaseLimits::default();
    request.finite_case_limits.max_retained_terminals = 0;
    assert!(
        family_candidates(request.clone())
            .unwrap_err()
            .message()
            .contains("must be positive")
    );
    request.finite_case_policy = FiniteCasePolicy::SearchFinite;
    assert!(
        family_candidates(request)
            .unwrap_err()
            .message()
            .contains("require retain-rank-finite")
    );
}
