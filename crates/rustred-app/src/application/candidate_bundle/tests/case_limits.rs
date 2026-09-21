//! Work-resource transport never changes saved mathematical policy.
use super::*;
use rustred::family::IntegralKey;

fn enlarged() -> CaseIntersectionLimits {
    CaseIntersectionLimits {
        max_work_items: 16_384,
        max_terms_per_conjunction: 400_000,
        max_normalizations: 4096,
        max_factorizations: 16_384,
    }
}

#[test]
fn application_intersection_limits_reach_real_geometry_failure() {
    for rank in [None, Some(10)] {
        let mut request = FamilyCandidatesRequest::new(K1);
        request.max_numerator_rank = rank;
        request.case_intersection_limits.max_work_items = 0;
        let error = family_candidates(request).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Execution);
        assert!(
            error.message().contains("WorkItems budget 0 exhausted"),
            "{error}"
        );
        assert!(error.message().contains("shared per-conjunction limits:"));
        assert!(error.message().contains("max_work_items: 0"));
        assert!(error.message().contains("work_items: 0"));
    }
}

#[test]
fn larger_case_resources_preserve_small_exact_reductions_and_report_effective_values() {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.max_numerator_rank = Some(2);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert_eq!(
        request.case_intersection_limits,
        CaseIntersectionLimits::default()
    );
    let original = family_candidates(request.clone()).unwrap();
    request.case_intersection_limits = enlarged();
    let raised = family_candidates(request.clone()).unwrap();
    // Fixed small-fixture equality is a regression check, not a promise that
    // different resource limits preserve arbitrary search/seed chronology.
    assert_same_program(original.bundle(), raised.bundle());
    let report: toml::Value = toml::from_str(raised.to_toml()).unwrap();
    for (key, value) in [
        ("case_max_work_items", 16_384),
        ("case_max_terms_per_conjunction", 400_000),
        ("case_max_normalizations", 4096),
        ("case_max_factorizations", 16_384),
    ] {
        assert_eq!(report[key].as_integer(), Some(value));
    }
    let (_, mut before) = load_generated_candidate_bundle::<3>(
        original.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let (_, mut after) = load_generated_candidate_bundle::<3>(
        raised.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    for powers in [[3, 1, 1], [0, 3, 1], [-1, 2, 1], [-2, 2, 1], [0, 0, 1]] {
        let key = IntegralKey::try_new(powers).unwrap();
        assert_eq!(
            before.reduce_unit_mass(&key).unwrap().terms(),
            after.reduce_unit_mass(&key).unwrap().terms()
        );
    }
}
