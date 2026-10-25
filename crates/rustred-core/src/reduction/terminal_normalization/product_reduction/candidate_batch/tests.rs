use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy, symmetry};
use crate::solver::{
    CoordinateCase, ExceptionalConditions, Integral, IntegralOrder, RuleCandidate, SearchStats,
    SectorRule, SectorSolution, SectorStats,
};

fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}
fn fixture() -> (Arc<IntegralFamily>, CandidateReducer<1>) {
    let c = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "batch-fixture",
            vec!["k".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![AffineDenominator::new(c.integer(-1), vec![c.one()])],
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    );
    // This obtains the established indexed map only. All formulas below are
    // explicit synthetic DAG fixtures; no IBPs are generated or claimed.
    let indexed = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let rules = (2..=5)
        .map(|power| SectorRule {
            dispatch_policy: Default::default(),
            candidate: RuleCandidate {
                case: CoordinateCase::new([Some(power)]).unwrap().into(),
                target: Integral::numeric([power]).unwrap(),
                rhs: vec![crate::solver::Term {
                    integral: Integral::numeric([power - 1]).unwrap(),
                    coefficient: indexed
                        .lift(&c.coefficient_fixture("1/(d-4)"))
                        .unwrap()
                        .raw()
                        .clone(),
                }],
                sources: vec![],
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions::default(),
        })
        .collect();
    let solution = SectorSolution {
        order: IntegralOrder::new([true], [false]),
        finite_case_policy: Default::default(),
        max_numerator_rank: None,
        rules,
        finite_residuals: vec![Integral::numeric([1]).unwrap()],
        stats: SectorStats::default(),
    };
    let reducer = CandidateReducer::try_new(
        &family,
        [true],
        OrderingPolicy::SpiredUncutV1,
        [([true], solution)],
        vec![],
        Default::default(),
    )
    .unwrap();
    (family, reducer)
}
fn run(
    reducer: &mut CandidateReducer<1>,
    family: Arc<IntegralFamily>,
    terms: Vec<(IntegralKey, Coefficient)>,
    limits: ReductionLimits,
) -> Result<VacuumFactorReduction, Error> {
    scoped(reducer, |reducer| {
        reduce_candidate_terms(reducer, family, terms, [], limits)
    })
}

#[test]
fn partial_candidate_factor_cannot_enter_a_complete_product_result() {
    use crate::solver::UnresolvedPolicy;
    let (family, mut reducer) = fixture();
    let c = family.coefficient_context();
    reducer
        .set_unresolved_policy(UnresolvedPolicy::Preserve)
        .unwrap();
    let result = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one()), (key(9), c.one())],
        Default::default(),
    );
    assert!(matches!(result, Err(Error::UnresolvedFactor { target, .. }) if target == key(9)));
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    let complete = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one())],
        Default::default(),
    )
    .unwrap();
    assert_eq!(complete.terms[0].integral, key(1));
    assert_eq!(
        complete.terms[0].coefficient,
        c.coefficient_fixture("1/(d-4)^3")
    );
    let unsafe_result = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one())],
        ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 0,
            ..Default::default()
        },
    );
    assert!(matches!(unsafe_result, Err(Error::Provider { .. })));
}

#[test]
fn candidate_batch_reuses_guarded_subtrees_and_reports_only_actual_work() {
    let (family, mut reducer) = fixture();
    let c = family.coefficient_context();
    let result = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one()), (key(5), c.one())],
        ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 4,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.work.rule_applications, 4); // Cold single calls need 3+4.
    assert_eq!(result.work.cache_hits, 1);
    assert_eq!(result.terms.len(), 1);
    assert_eq!(result.terms[0].integral, key(1));
    assert_eq!(
        result.terms[0].coefficient,
        c.coefficient_fixture("1/(d-4)^3+1/(d-4)^4")
    );
    assert!(
        result
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    assert_eq!(reducer.statistics().cached_coefficient_bytes(), 0);
    let zero = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one()), (key(5), c.coefficient_fixture("4-d"))],
        Default::default(),
    )
    .unwrap();
    assert!(zero.terms.is_empty());
    assert!(
        zero.nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert_eq!(zero.work.rule_applications, 4);
    assert_eq!(reducer.limits(), ReductionLimits::default());
}

#[test]
fn candidate_batch_clears_partial_native_cache_on_missing_rule_and_budget_errors() {
    let (family, mut reducer) = fixture();
    let c = family.coefficient_context();
    assert!(
        run(
            &mut reducer,
            family.clone(),
            vec![(key(4), c.one()), (key(9), c.one())],
            Default::default()
        )
        .is_err()
    );
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    assert_eq!(reducer.statistics().cached_coefficient_terms(), 0);
    assert_eq!(reducer.statistics().cached_coefficient_bytes(), 0);
    for limits in [
        ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 3,
            ..Default::default()
        },
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_coefficient_terms: 0,
            ..Default::default()
        },
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_coefficient_bytes: 0,
            ..Default::default()
        },
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_integrals: 5,
            ..Default::default()
        },
    ] {
        assert!(
            run(
                &mut reducer,
                family.clone(),
                vec![(key(4), c.one()), (key(5), c.one())],
                limits
            )
            .is_err()
        );
        assert_eq!(reducer.statistics().cached_integrals(), 0);
        assert_eq!(reducer.statistics().cached_coefficient_bytes(), 0);
    }
    let recovered = run(
        &mut reducer,
        family.clone(),
        vec![(key(4), c.one()), (key(5), c.one())],
        Default::default(),
    )
    .unwrap();
    assert_eq!(recovered.work.rule_applications, 4);
}

#[test]
fn existing_guarded_candidate_rechecks_inclusive_warm_cache_against_tightened_grant() {
    let (_, mut reducer) = fixture();
    reducer
        .reduce_unit_mass_guarded(&key(4), Default::default())
        .unwrap();
    let before = reducer.statistics();
    assert!(
        reducer
            .reduce_unit_mass_guarded(
                &key(4),
                ReductionLimits {
                    diagnostics_enabled: true,
                    max_cached_coefficient_terms: before.cached_coefficient_terms() - 1,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(reducer.statistics().cache_hits(), before.cache_hits());
    reducer.clear_cache().unwrap();
}

#[test]
fn public_transported_candidate_batch_authenticates_family_and_clears_at_both_boundaries() {
    let (family, mut reducer) = fixture();
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        symmetry::MomentumMap::new(
            symmetry::CoefficientMatrix::try_new(1, 1, [c.one()]).unwrap(),
            symmetry::CoefficientMatrix::try_new(1, 0, []).unwrap(),
            symmetry::CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    let prepared = integral_transport::compile(
        &family,
        family.clone(),
        Arc::new(map),
        Mask::try_new([true]).unwrap(),
        Mask::try_new([true]).unwrap(),
        Default::default(),
    )
    .unwrap();
    reducer
        .reduce_unit_mass_guarded(&key(5), Default::default())
        .unwrap();
    let result = reduce_transported_candidate_factor(
        &mut reducer,
        &prepared,
        &key(4),
        family.clone(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result.work.rule_applications, 3,
        "old cache is not hidden in a new scoped request"
    );
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    let mut wrong = (*family).denominators().to_vec();
    wrong[0] = AffineDenominator::new(c.integer(-2), vec![c.one()]);
    let foreign = Arc::new(
        IntegralFamily::new(
            "foreign",
            vec!["k".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            wrong,
            vec![],
            vec![c.zero()],
        )
        .unwrap(),
    );
    reducer
        .reduce_unit_mass_guarded(&key(4), Default::default())
        .unwrap();
    assert!(
        reduce_transported_candidate_factor(
            &mut reducer,
            &prepared,
            &key(4),
            foreign,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    assert_eq!(reducer.statistics().cached_integrals(), 0);
}
