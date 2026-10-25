use super::*;
use crate::algebra::Coefficient;

struct ExistingProvider {
    calls: Arc<Mutex<usize>>,
    fail: bool,
}
impl VacuumFactorProvider for ExistingProvider {
    type Error = String;
    fn reduce(
        &mut self,
        family: Arc<IntegralFamily>,
        target: &IntegralKey,
        _: crate::reduction::ReductionLimits,
    ) -> Result<VacuumFactorReduction, String> {
        *self.calls.lock().unwrap() += 1;
        if self.fail {
            return Err("selected program is unavailable".into());
        }
        Ok(VacuumFactorReduction {
            terms: vec![VacuumFactorTerm {
                family: family.clone(),
                integral: target.clone(),
                coefficient: family.coefficient_context().one(),
            }],
            nonzero_conditions: vec![family.coefficient_context().coefficient_fixture("d-9")],
            ..Default::default()
        })
    }
}

fn conditional_two_loop() -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    let d4 = c.coefficient_fixture("d-4");
    Arc::new(
        IntegralFamily::new(
            "zero-proof-domain",
            vec!["q1".into(), "q2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(c.integer(-1), vec![d4, c.zero(), c.zero()]),
                AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
                AffineDenominator::new(c.integer(-1), vec![c.one(), c.integer(-2), c.one()]),
            ],
            vec![],
            vec![c.zero(); 3],
        )
        .unwrap(),
    )
}

#[test]
fn native_free_loop_proof_includes_numerators_and_reuses_support_and_factor_caches() {
    let f = conditional_two_loop();
    let c = f.coefficient_context();
    // This 3x3 basis has determinant 2*(d-4). Native FamilyDomain retains
    // that exact numerator; it does not promise primitive/monic guards.
    let d4 = c.coefficient_fixture("d-4");
    let expected_determinant = c.try_mul(&c.integer(2), &d4, Default::default()).unwrap();
    assert_eq!(f.domain().basis_determinant(), &expected_determinant);
    let determinant_guard: Coefficient = f.domain().basis_determinant().numerator.clone().into();
    assert_eq!(
        c.try_div(&determinant_guard, &d4, Default::default())
            .unwrap(),
        c.integer(2),
        "the native guard has exactly the same nonzero locus as d-4"
    );
    let domain_guards = f
        .domain()
        .conditions()
        .map(|condition| Coefficient::from(condition.polynomial().clone()))
        .collect::<Vec<_>>();
    assert!(domain_guards.contains(&determinant_guard));
    let calls = Arc::new(Mutex::new(0));
    let mut reducer = VacuumProductReducer::try_new(
        ExistingProvider {
            calls: calls.clone(),
            fail: true,
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    for (powers, expected_analysis, expected_support_hits) in
        [([0, 1, 0], 1, 0), ([-1, 2, -2], 0, 1), ([0, 1, 0], 0, 0)]
    {
        let result = reducer
            .reduce_products([(vec![factor(&f, powers)], c.one())], [])
            .unwrap();
        assert!(result.is_zero());
        for guard in &domain_guards {
            assert!(
                result.nonzero_conditions().contains(guard),
                "native family guard was lost: {guard}"
            );
        }
        assert_eq!(result.statistics().provider_calls, 0);
        assert_eq!(result.statistics().zero_analyses, expected_analysis);
        assert_eq!(result.statistics().zero_cache_hits, expected_support_hits);
    }
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(reducer.cache_statistics().cached_zero_supports, 1);
    assert_eq!(reducer.cache_statistics().cached_factors, 2);
    assert!(reducer.cache_statistics().coefficient_bytes > 0);
    reducer.clear_cache().unwrap();
    assert_eq!(reducer.cache_statistics().cached_zero_supports, 0);
    assert_eq!(reducer.cache_statistics().coefficient_bytes, 0);
    let result = reducer
        .reduce_products([(vec![factor(&f, [0, 1, 0])], c.one())], [])
        .unwrap();
    assert_eq!(result.statistics().zero_analyses, 1);
    for guard in &domain_guards {
        assert!(result.nonzero_conditions().contains(guard));
    }
}

#[test]
fn zero_factor_does_not_skip_sibling_guards_or_missing_programs() {
    let free = family("free-loop-factor", 2);
    let sibling = conditional_family();
    let c = free.coefficient_context();
    let factors = vec![factor(&free, [0, 1, -1]), factor(&sibling, [1])];
    for fail in [false, true] {
        let calls = Arc::new(Mutex::new(0));
        let mut reducer = VacuumProductReducer::try_new(
            ExistingProvider {
                calls: calls.clone(),
                fail,
            },
            free.clone(),
            Default::default(),
        )
        .unwrap();
        let result = reducer.reduce_products([(factors.clone(), c.one())], []);
        if fail {
            assert!(matches!(
                result,
                Err(VacuumProductReductionError::Provider { .. })
            ));
        } else {
            let result = result.unwrap();
            assert!(result.is_zero());
            assert!(
                result
                    .nonzero_conditions()
                    .contains(&c.coefficient_fixture("d-4"))
            );
            assert!(
                result
                    .nonzero_conditions()
                    .contains(&c.coefficient_fixture("d-9"))
            );
            assert_eq!(result.statistics().provider_calls, 1);
            let warm = reducer
                .reduce_products([(factors.clone(), c.one())], [])
                .unwrap();
            assert_eq!(warm.nonzero_conditions(), result.nonzero_conditions());
            assert_eq!(warm.statistics().provider_calls, 0);
        }
        assert_eq!(*calls.lock().unwrap(), 1);
    }
}

#[test]
fn inconclusive_zero_test_still_requires_provider_and_is_cached_by_support() {
    let f = family("massive-not-proved-zero", 1);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(0));
    let mut reducer = VacuumProductReducer::try_new(
        ExistingProvider {
            calls: calls.clone(),
            fail: false,
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let first = reducer
        .reduce_products([(vec![factor(&f, [1])], c.one())], [])
        .unwrap();
    assert!(!first.is_zero());
    assert_eq!(first.statistics().zero_analyses, 1);
    assert_eq!(first.statistics().provider_calls, 1);
    let next = reducer
        .reduce_products([(vec![factor(&f, [2])], c.one())], [])
        .unwrap();
    assert!(!next.is_zero());
    assert_eq!(next.statistics().zero_cache_hits, 1);
    assert_eq!(next.statistics().provider_calls, 1);
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[test]
fn zero_proofs_obey_native_analysis_and_shared_cache_allowances() {
    let f = family("bounded-free-loop", 2);
    let c = f.coefficient_context();
    for mode in 0..4 {
        let calls = Arc::new(Mutex::new(0));
        let mut limits = VacuumProductReductionLimits::default();
        limits.diagnostics_enabled = true;
        match mode {
            0 => limits.max_zero_analyses = 0,
            1 => limits.zero.max_rank_columns = 0,
            2 => limits.reduction.max_cached_integrals = 0,
            3 => limits.reduction.max_cached_integrals = 1,
            _ => unreachable!(),
        }
        let mut reducer = VacuumProductReducer::try_new(
            ExistingProvider {
                calls: calls.clone(),
                fail: false,
            },
            f.clone(),
            limits,
        )
        .unwrap();
        assert!(
            reducer
                .reduce_products([(vec![factor(&f, [0, 1, 0])], c.one())], [])
                .is_err()
        );
        assert_eq!(*calls.lock().unwrap(), 0);
        assert_eq!(reducer.cache_statistics().cached_factors, 0);
        if mode == 3 {
            assert_eq!(reducer.cache_statistics().cached_zero_supports, 1);
        } else {
            assert_eq!(reducer.cache_statistics().cached_zero_supports, 0);
        }
        reducer.clear_cache().unwrap();
        assert_eq!(reducer.cache_statistics().coefficient_bytes, 0);
    }
}
