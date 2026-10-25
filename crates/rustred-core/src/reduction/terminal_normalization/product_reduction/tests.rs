use super::super::VacuumIntegralKey;
use super::*;
use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
mod combination;
mod zero;

fn family(name: &str, loops: usize) -> Arc<IntegralFamily> {
    let c = CoefficientContext::new(["d"]);
    let mut momenta = Vec::new();
    for i in 0..loops {
        let mut row = vec![0; loops];
        row[i] = 1;
        momenta.push(row);
    }
    for i in 0..loops {
        for j in i + 1..loops {
            let mut row = vec![0; loops];
            row[i] = 1;
            row[j] = -1;
            momenta.push(row);
        }
    }
    let denominators = momenta
        .iter()
        .map(|row| {
            AffineDenominator::new(
                c.integer(-1),
                (0..loops)
                    .flat_map(|i| {
                        (i..loops).map(move |j| row[i] * row[j] * if i == j { 1 } else { 2 })
                    })
                    .map(|n| c.integer(n))
                    .collect(),
            )
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            name.to_owned(),
            (0..loops).map(|i| format!("q{i}")).collect(),
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![c.zero(); momenta.len()],
        )
        .unwrap(),
    )
}
fn key(powers: impl IntoIterator<Item = i64>) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
fn factor(family: &Arc<IntegralFamily>, powers: impl IntoIterator<Item = i64>) -> VacuumFactor {
    VacuumFactor {
        family: family.clone(),
        integral: key(powers),
    }
}
fn term(
    family: &Arc<IntegralFamily>,
    powers: impl IntoIterator<Item = i64>,
    n: i64,
) -> VacuumFactorTerm {
    VacuumFactorTerm {
        family: family.clone(),
        integral: key(powers),
        coefficient: family.coefficient_context().integer(n),
    }
}

struct PolynomialProvider {
    calls: Arc<Mutex<Vec<usize>>>,
}

#[test]
fn typed_missing_factor_coverage_is_distinct_from_unsafe_provider_failure() {
    struct Provider(bool);
    impl VacuumFactorProvider for Provider {
        type Error = VacuumProductReductionError;
        fn error_is_missing_coverage(error: &Self::Error) -> bool {
            matches!(error, VacuumProductReductionError::UnresolvedFactor { .. })
        }
        fn reduce(
            &mut self,
            family: Arc<IntegralFamily>,
            target: &IntegralKey,
            _: crate::reduction::ReductionLimits,
        ) -> Result<VacuumFactorReduction, Self::Error> {
            if self.0 {
                Err(VacuumProductReductionError::UnresolvedFactor {
                    family: family.fingerprint().into(),
                    target: target.clone(),
                })
            } else {
                Err(VacuumProductReductionError::InvalidInput(
                    "unsafe source condition".into(),
                ))
            }
        }
    }
    let f = family("unresolved-factor", 1);
    for missing in [false, true] {
        let mut reducer =
            VacuumProductReducer::try_new(Provider(missing), f.clone(), Default::default())
                .unwrap();
        let result =
            reducer.reduce_products([(vec![factor(&f, [3])], f.coefficient_context().one())], []);
        if missing {
            assert!(
                matches!(result, Err(VacuumProductReductionError::UnresolvedFactor { target, .. }) if target == key([3]))
            );
        } else {
            assert!(matches!(
                result,
                Err(VacuumProductReductionError::Provider { .. })
            ));
        }
        assert_eq!(reducer.cache_statistics().cached_factors, 0);
    }
}
impl VacuumFactorProvider for PolynomialProvider {
    type Error = String;
    fn reduce(
        &mut self,
        family: Arc<IntegralFamily>,
        target: &IntegralKey,
        limits: crate::reduction::ReductionLimits,
    ) -> Result<VacuumFactorReduction, String> {
        self.calls
            .lock()
            .unwrap()
            .push(limits.max_rule_applications);
        if limits.max_rule_applications == 0 {
            return Err("remaining rule allowance is zero".into());
        }
        let (a, b) = match target.powers() {
            [3] => (2, 3),
            [4] => (-3, 2),
            _ => return Err("missing saved program".into()),
        };
        Ok(VacuumFactorReduction {
            terms: vec![term(&family, [1], a), term(&family, [2], b)],
            nonzero_conditions: vec![family.coefficient_context().coefficient_fixture("d-4")],
            work: VacuumFactorWork {
                rule_applications: 1,
                ..Default::default()
            },
        })
    }
}

#[test]
fn exact_multi_terminal_product_convolution_sorts_and_coalesces() {
    let f = family("polynomial", 1);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new(
        PolynomialProvider {
            calls: calls.clone(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let input = || vec![(vec![factor(&f, [3]), factor(&f, [4])], c.one())];
    let actual = reducer
        .reduce_products(input(), [c.coefficient_fixture("d-7")])
        .unwrap();
    let a = VacuumIntegralKey::from_family(&f, key([1]));
    let b = VacuumIntegralKey::from_family(&f, key([2]));
    assert_eq!(
        actual.terms(),
        &BTreeMap::from([
            (vec![a.clone(), a.clone()], c.integer(-6)),
            (vec![a, b.clone()], c.integer(-5)),
            (vec![b.clone(), b], c.integer(6))
        ])
    );
    assert_eq!(actual.statistics().provider_calls, 2);
    assert_eq!(actual.statistics().rule_applications, 2);
    assert!(
        actual
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-4"))
    );
    let warm = reducer
        .reduce_products(input(), [c.coefficient_fixture("d-7")])
        .unwrap();
    assert_eq!(actual.terms(), warm.terms());
    assert_eq!(actual.nonzero_conditions(), warm.nonzero_conditions());
    assert_eq!(warm.statistics().provider_calls, 0);
    assert_eq!(warm.statistics().cache_hits, 2);
    assert_eq!(calls.lock().unwrap().len(), 2);
    reducer.clear_cache().unwrap();
    assert_eq!(reducer.cache_statistics().cached_factors, 0);
    assert_eq!(reducer.cache_statistics().coefficient_bytes, 0);
    reducer.reduce_products(input(), []).unwrap();
    assert_eq!(calls.lock().unwrap().len(), 4);
}

struct RecursiveProvider {
    calls: Arc<Mutex<Vec<usize>>>,
}
impl VacuumFactorProvider for RecursiveProvider {
    type Error = String;
    fn reduce(
        &mut self,
        family: Arc<IntegralFamily>,
        target: &IntegralKey,
        _: crate::reduction::ReductionLimits,
    ) -> Result<VacuumFactorReduction, String> {
        self.calls.lock().unwrap().push(family.loop_count());
        let terms = match family.loop_count() {
            3 => vec![term(&family, [1, 1, 1, 0, 0, 0], 5)],
            1 => vec![term(&family, [1], 2)],
            _ => vec![VacuumFactorTerm {
                family: family.clone(),
                integral: target.clone(),
                coefficient: family.coefficient_context().one(),
            }],
        };
        Ok(VacuumFactorReduction {
            terms,
            nonzero_conditions: vec![
                family
                    .coefficient_context()
                    .coefficient_fixture(&format!("d-{}", family.loop_count())),
            ],
            ..Default::default()
        })
    }
}

#[test]
fn provider_terminals_refactor_only_into_strictly_lower_loop_calls() {
    let f = family("recursive", 3);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new(
        RecursiveProvider {
            calls: calls.clone(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let result = reducer
        .reduce_products([(vec![factor(&f, [1, 1, 1, 1, 1, 1])], c.one())], [])
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), [3, 1]);
    assert_eq!(result.terms().len(), 1);
    let (product, coefficient) = result.terms().first_key_value().unwrap();
    assert_eq!(coefficient, &c.integer(40));
    assert_eq!(product.len(), 3);
    assert!(product.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(
        product
            .iter()
            .all(|factor| result.families()[factor.family_fingerprint()].loop_count() == 1)
    );
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-3"))
    );
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-1"))
    );
}

#[test]
fn connected_and_numerator_provider_terminals_stop_at_same_loop() {
    let f = family("fixed", 2);
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new(
        RecursiveProvider {
            calls: calls.clone(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let result = reducer
        .reduce_products(
            [
                (vec![factor(&f, [1, 1, 1])], c.one()),
                (vec![factor(&f, [1, 1, -1])], c.one()),
            ],
            [],
        )
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), [2, 2]);
    assert_eq!(result.terms().len(), 2);
    assert!(result.terms().keys().all(|product|product.len()==1 && product[0].family_fingerprint()==f.fingerprint()));
}

#[test]
fn shared_provider_rule_and_cartesian_budgets_are_enforced() {
    let f = family("bounded", 1);
    let c = f.coefficient_context();
    for (rules, terms) in [(1, 100), (100, 2)] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut limits = VacuumProductReductionLimits::default();
        limits.diagnostics_enabled = true;
        limits.reduction.max_rule_applications = rules;
        limits.reduction.max_factorization_terms = terms;
        let mut reducer = VacuumProductReducer::try_new(
            PolynomialProvider {
                calls: calls.clone(),
            },
            f.clone(),
            limits,
        )
        .unwrap();
        assert!(
            reducer
                .reduce_products([(vec![factor(&f, [3]), factor(&f, [4])], c.one())], [])
                .is_err()
        );
        if rules == 1 {
            assert_eq!(*calls.lock().unwrap(), [1, 0]);
        }
    }
}

struct FixedProvider {
    scale: i64,
    output: Option<Arc<IntegralFamily>>,
    zero_guard: bool,
}
impl VacuumFactorProvider for FixedProvider {
    type Error = String;
    fn reduce(
        &mut self,
        family: Arc<IntegralFamily>,
        _: &IntegralKey,
        _: crate::reduction::ReductionLimits,
    ) -> Result<VacuumFactorReduction, String> {
        let output = self.output.clone().unwrap_or(family);
        Ok(VacuumFactorReduction {
            terms: vec![term(&output, [1], self.scale)],
            nonzero_conditions: if self.zero_guard {
                vec![output.coefficient_context().zero()]
            } else {
                vec![]
            },
            ..Default::default()
        })
    }
}

#[test]
fn provider_identity_is_session_owned_and_input_identity_is_family_qualified() {
    let a = family("a", 1);
    let b = family("b", 1);
    let c = a.coefficient_context();
    for scale in [1, 2] {
        let mut reducer = VacuumProductReducer::try_new(
            FixedProvider {
                scale,
                output: None,
                zero_guard: false,
            },
            a.clone(),
            Default::default(),
        )
        .unwrap();
        let result = reducer
            .reduce_products([(vec![factor(&a, [3]), factor(&b, [3])], c.one())], [])
            .unwrap();
        let (product, coefficient) = result.terms().first_key_value().unwrap();
        assert_eq!(product.len(), 2);
        assert_ne!(
            product[0].family_fingerprint(),
            product[1].family_fingerprint()
        );
        assert_eq!(coefficient, &c.integer(scale * scale));
        assert_eq!(result.statistics().provider_calls, 2);
    }
}

#[test]
fn missing_provider_bad_arity_vanished_guards_and_loop_mismatch_fail_closed() {
    let f = family("errors", 1);
    let c = f.coefficient_context();
    let mut missing = VacuumProductReducer::try_new(
        PolynomialProvider {
            calls: Default::default(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        missing.reduce_products([(vec![factor(&f, [5])], c.one())], []),
        Err(VacuumProductReductionError::Provider { .. })
    ));
    assert!(
        missing
            .reduce_products([(vec![factor(&f, [1, 0])], c.zero())], [])
            .is_err()
    );
    let mut vanished = VacuumProductReducer::try_new(
        FixedProvider {
            scale: 1,
            output: None,
            zero_guard: true,
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    assert!(
        vanished
            .reduce_products([(vec![factor(&f, [3])], c.one())], [])
            .is_err()
    );
    assert_eq!(vanished.cache_statistics().cached_factors, 0);
    let mut mismatch = VacuumProductReducer::try_new(
        FixedProvider {
            scale: 0,
            output: Some(family("wrong", 2)),
            zero_guard: false,
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    assert!(
        mismatch
            .reduce_products([(vec![factor(&f, [3])], c.one())], [])
            .is_err()
    );
    assert_eq!(mismatch.cache_statistics().cached_factors, 0);
}

#[test]
fn cancelled_input_keeps_denominator_conditions_and_avoids_provider() {
    let f = family("cancel", 1);
    let c = f.coefficient_context();
    let mut reducer = VacuumProductReducer::try_new(
        PolynomialProvider {
            calls: Default::default(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let result = reducer
        .reduce_products(
            [
                (vec![factor(&f, [3])], c.coefficient_fixture("1/(d-9)")),
                (vec![factor(&f, [3])], c.coefficient_fixture("-1/(d-9)")),
            ],
            [],
        )
        .unwrap();
    assert!(result.is_zero());
    assert_eq!(result.statistics().provider_calls, 0);
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-9"))
    );
}

#[test]
fn retained_cache_coefficients_and_guards_obey_existing_resource_caps() {
    let f = family("cache", 1);
    let c = f.coefficient_context();
    for (integrals, bytes) in [(1, usize::MAX), (100, 1)] {
        let mut limits = VacuumProductReductionLimits::default();
        limits.diagnostics_enabled = true;
        limits.reduction.max_cached_integrals = integrals;
        limits.reduction.max_cached_coefficient_bytes = bytes;
        let mut reducer = VacuumProductReducer::try_new(
            PolynomialProvider {
                calls: Default::default(),
            },
            f.clone(),
            limits,
        )
        .unwrap();
        assert!(
            reducer
                .reduce_products([(vec![factor(&f, [3]), factor(&f, [4])], c.one())], [])
                .is_err()
        );
        reducer.clear_cache().unwrap();
        assert_eq!(reducer.cache_statistics().coefficient_bytes, 0);
        assert_eq!(reducer.cache_statistics().cached_factors, 0);
    }
}

fn conditional_family() -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    let guard = context.coefficient_fixture("d-4");
    let family = Arc::new(
        IntegralFamily::new(
            "conditional-product-factor",
            vec!["q".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.integer(-1),
                vec![guard.clone()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    assert!(
        family
            .domain()
            .conditions()
            .any(|condition| condition.polynomial() == &guard.numerator)
    );
    family
}

#[test]
fn cancelled_input_retains_its_native_family_domain_without_calling_provider() {
    let f = conditional_family();
    let c = f.coefficient_context();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut reducer = VacuumProductReducer::try_new(
        PolynomialProvider {
            calls: calls.clone(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    let result = reducer
        .reduce_products(
            [
                (vec![factor(&f, [3])], c.one()),
                (vec![factor(&f, [3])], c.integer(-1)),
            ],
            [],
        )
        .unwrap();
    assert!(result.is_zero());
    assert!(
        result
            .nonzero_conditions()
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(reducer.cache_statistics().cached_factors, 0);
}

struct ReturnedCancellation {
    family: Arc<IntegralFamily>,
}
impl VacuumFactorProvider for ReturnedCancellation {
    type Error = String;
    fn reduce(
        &mut self,
        _: Arc<IntegralFamily>,
        _: &IntegralKey,
        _: crate::reduction::ReductionLimits,
    ) -> Result<VacuumFactorReduction, Self::Error> {
        Ok(VacuumFactorReduction {
            terms: vec![term(&self.family, [1], 1), term(&self.family, [1], -1)],
            ..Default::default()
        })
    }
}

#[test]
fn cancelled_returned_family_domain_survives_cached_zero_expansion() {
    let f = family("unconditional-input", 1);
    let c = f.coefficient_context();
    // Native basis validation may retain a harmless constant-one condition;
    // what matters here is that d-4 originates only in the returned family.
    let returned_guard = c.coefficient_fixture("d-4");
    assert!(
        f.domain()
            .conditions()
            .all(|condition| condition.polynomial() != &returned_guard.numerator)
    );
    let mut reducer = VacuumProductReducer::try_new(
        ReturnedCancellation {
            family: conditional_family(),
        },
        f.clone(),
        Default::default(),
    )
    .unwrap();
    for expected_calls in [1, 0] {
        let result = reducer
            .reduce_products([(vec![factor(&f, [3])], c.one())], [])
            .unwrap();
        assert!(result.is_zero());
        assert!(
            result
                .nonzero_conditions()
                .contains(&c.coefficient_fixture("d-4"))
        );
        assert_eq!(result.statistics().provider_calls, expected_calls);
    }
    assert!(reducer.cache_statistics().coefficient_terms > 0);
    let entry = reducer.cache.values().next().unwrap();
    assert!(entry.conditions.contains(&c.coefficient_fixture("d-4")));
}
