use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::Mask;
use crate::sector::symmetry::{CoefficientMatrix, Limits, MomentumMap, verify};

use super::{Error, ExpansionError, ExpansionLimits, Prepared, compile};

#[path = "trace_support_tests.rs"]
mod trace_support_tests;

fn family(
    name: &str,
    context: &CoefficientContext,
    numerator: [&str; 4],
    shift: bool,
) -> Arc<IntegralFamily> {
    let zero = context.zero();
    let one = context.one();
    Arc::new(
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(one.clone(), vec![one.clone(), zero.clone(), zero.clone()]),
                AffineDenominator::new(one.clone(), vec![zero.clone(), zero.clone(), one.clone()]),
                AffineDenominator::new(
                    context.coefficient_fixture(numerator[0]),
                    numerator[1..]
                        .iter()
                        .map(|s| context.coefficient_fixture(s))
                        .collect(),
                ),
            ],
            vec![],
            vec![if shift { one } else { zero.clone() }, zero.clone(), zero],
        )
        .unwrap(),
    )
}

fn fixture() -> (Arc<IntegralFamily>, Arc<IntegralFamily>) {
    let c = CoefficientContext::new(["d", "a"]);
    (
        family("source", &c, ["1", "1", "1", "-1"], false),
        family("target", &c, ["0", "0", "1", "0"], false),
    )
}

fn momentum(context: &CoefficientContext, entries: [&str; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(2, 2, entries.map(|s| context.coefficient_fixture(s))).unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn prepared(source: &IntegralFamily, target: Arc<IntegralFamily>, active: [bool; 3]) -> Prepared {
    let map = verify(
        source,
        &target,
        momentum(source.coefficient_context(), ["1", "0", "0", "1"]),
        Limits::default(),
    )
    .unwrap();
    compile(
        source,
        target,
        Arc::new(map),
        Mask::try_new(active).unwrap(),
        Mask::try_new(active).unwrap(),
        ExpansionLimits::default(),
    )
    .unwrap()
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn terms(prepared: &Prepared, powers: [i64; 3]) -> BTreeMap<IntegralKey, Coefficient> {
    prepared
        .transport(&key(powers), ExpansionLimits::default())
        .unwrap()
        .terms()
        .iter()
        .map(|term| (term.key().clone(), term.coefficient().clone()))
        .collect()
}

#[test]
fn identity_preserves_arbitrary_positive_powers_and_finite_numerators() {
    let (_, target) = fixture();
    let p = prepared(&target, target.clone(), [true, true, false]);
    for powers in [[i64::MAX, 7, 0], [1, 3, -4], [0, -2, -3]] {
        assert_eq!(
            terms(&p, powers),
            BTreeMap::from([(key(powers), target.coefficient_context().one())])
        );
    }
}

#[test]
fn exact_verified_permutation_agrees_with_existing_permutation_transport() {
    let (_, target) = fixture();
    let swap = || {
        verify(
            &target,
            &target,
            momentum(target.coefficient_context(), ["0", "1", "1", "0"]),
            Limits::default(),
        )
        .unwrap()
    };
    let old = crate::sector::symmetry::permutation::compile(&target, swap()).unwrap();
    let p = compile(
        &target,
        target.clone(),
        Arc::new(swap()),
        Mask::try_new([true, true, false]).unwrap(),
        Mask::try_new([true, true, false]).unwrap(),
        ExpansionLimits::default(),
    )
    .unwrap();
    for powers in [[4, 2, -3], [0, 7, -1], [-2, 0, -1]] {
        let mut expected = [0; 3];
        old.transport_into(&powers, &mut expected).unwrap();
        assert_eq!(
            terms(&p, powers),
            BTreeMap::from([(key(expected), target.coefficient_context().one())])
        );
    }
}

#[test]
fn affine_numerator_routes_constants_cancellations_and_pinches() {
    let (source, target) = fixture();
    let p = prepared(&source, target.clone(), [true, true, false]);
    let c = target.coefficient_context();
    assert_eq!(
        terms(&p, [1, 1, -1]),
        BTreeMap::from([
            (key([1, 1, -1]), c.one()),
            (key([0, 1, 0]), c.one()),
            (key([1, 0, 0]), c.integer(-1)),
            (key([1, 1, 0]), c.one()),
        ])
    );
    let square = p
        .transport(&key([1, 1, -2]), ExpansionLimits::default())
        .unwrap();
    assert_eq!(square.terms().len(), 10);
    assert_eq!(square.source_family_fingerprint(), source.fingerprint());
    assert_eq!(square.target_family_fingerprint(), target.fingerprint());
    for endpoint in square.terms() {
        let rank: u64 = endpoint
            .key()
            .powers()
            .iter()
            .filter(|p| **p < 0)
            .map(|p| p.unsigned_abs())
            .sum();
        assert!(rank <= 2);
        assert!(endpoint.key().powers()[2] <= 0);
        assert!(endpoint.key().powers()[..2].iter().all(|p| *p <= 1));
    }
}

#[test]
fn multiple_numerator_factors_include_root_pinches() {
    let (source, target) = fixture();
    let p = prepared(&source, target.clone(), [true, false, false]);
    let c = target.coefficient_context();
    assert_eq!(
        terms(&p, [1, -1, -1]),
        BTreeMap::from([
            (key([1, -1, -1]), c.one()),
            (key([0, -1, 0]), c.one()),
            (key([1, -2, 0]), c.integer(-1)),
            (key([1, -1, 0]), c.one()),
        ])
    );
    let p = prepared(&source, target.clone(), [true, true, false]);
    assert_eq!(
        terms(&p, [-1, 1, 0]),
        BTreeMap::from([(key([-1, 1, 0]), c.one())])
    );
}

#[test]
fn inverse_verified_affine_maps_exactly_cancel_all_extra_terms() {
    let (source, target) = fixture();
    let forward = prepared(&source, target.clone(), [true, true, false]);
    let backward = prepared(&target, source.clone(), [true, true, false]);
    let context = source.coefficient_context();
    for powers in [[2, 3, -2], [0, 1, -3], [-1, 0, -2]] {
        let mut sum: BTreeMap<IntegralKey, Coefficient> = BTreeMap::new();
        for term in forward
            .transport(&key(powers), ExpansionLimits::default())
            .unwrap()
            .terms()
        {
            for returned in backward
                .transport(term.key(), ExpansionLimits::default())
                .unwrap()
                .terms()
            {
                let product = context
                    .try_mul(
                        term.coefficient(),
                        returned.coefficient(),
                        Default::default(),
                    )
                    .unwrap();
                let previous = sum.remove(returned.key()).unwrap_or_else(|| context.zero());
                let value = context
                    .try_add(&previous, &product, Default::default())
                    .unwrap();
                if !value.is_zero() {
                    sum.insert(returned.key().clone(), value);
                }
            }
        }
        assert_eq!(sum, BTreeMap::from([(key(powers), context.one())]));
    }
}

#[test]
fn admission_rejects_foreign_families_roots_shifts_and_nonunit_maps() {
    let (source, target) = fixture();
    let map = Arc::new(
        verify(
            &source,
            &target,
            momentum(source.coefficient_context(), ["1", "0", "0", "1"]),
            Limits::default(),
        )
        .unwrap(),
    );
    let root = Mask::try_new([true, true, false]).unwrap();
    assert!(matches!(
        compile(
            &target,
            target.clone(),
            map.clone(),
            root.clone(),
            root.clone(),
            Default::default()
        ),
        Err(Error::FamilyMismatch { side: "source" })
    ));
    assert!(matches!(
        compile(
            &source,
            source.clone(),
            map.clone(),
            root.clone(),
            root.clone(),
            Default::default()
        ),
        Err(Error::FamilyMismatch { side: "target" })
    ));
    assert!(matches!(
        compile(
            &source,
            target.clone(),
            map.clone(),
            Mask::try_new([true]).unwrap(),
            root.clone(),
            Default::default()
        ),
        Err(Error::RootArity { .. })
    ));
    assert!(matches!(
        compile(
            &source,
            target.clone(),
            map,
            root.clone(),
            Mask::try_new([true, false, false]).unwrap(),
            Default::default()
        ),
        Err(Error::ActiveBijection)
    ));
    let shifted = family(
        "shifted",
        source.coefficient_context(),
        ["0", "0", "1", "0"],
        true,
    );
    let shift_map = Arc::new(
        verify(
            &shifted,
            &shifted,
            momentum(shifted.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        compile(
            &shifted,
            shifted.clone(),
            shift_map,
            root.clone(),
            root.clone(),
            Default::default()
        ),
        Err(Error::AnalyticPowerShift { .. })
    ));
    let nonunit = Arc::new(
        verify(
            &target,
            &target,
            momentum(target.coefficient_context(), ["2", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        compile(
            &target,
            target.clone(),
            nonunit,
            root.clone(),
            root.clone(),
            Default::default()
        ),
        Err(Error::NonUnitJacobian)
    ));
    let affine_active = Arc::new(
        verify(
            &source,
            &target,
            momentum(source.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        compile(
            &source,
            target.clone(),
            affine_active,
            Mask::try_new([false, false, true]).unwrap(),
            root,
            Default::default()
        ),
        Err(Error::NonUnitActiveRow { .. })
    ));
}

#[test]
fn admission_distinguishes_unresolved_conditions_and_nonconstant_inactive_rows() {
    let (_, target) = fixture();
    let root = Mask::try_new([true, true, false]).unwrap();
    let parametric = family(
        "parametric",
        target.coefficient_context(),
        ["0", "a", "1", "0"],
        false,
    );
    let map = Arc::new(
        verify(
            &parametric,
            &target,
            momentum(target.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        compile(
            &parametric,
            target.clone(),
            map,
            root.clone(),
            root.clone(),
            Default::default()
        ),
        Err(Error::NonconstantCoefficient { .. })
    ));
    let conditional = family(
        "conditional",
        target.coefficient_context(),
        ["0", "0", "a", "0"],
        false,
    );
    let map = Arc::new(
        verify(
            &conditional,
            &target,
            momentum(target.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        compile(
            &conditional,
            target,
            map,
            root.clone(),
            root,
            Default::default()
        ),
        Err(Error::UnresolvedCondition { .. })
    ));
}

#[test]
fn fixed_input_admission_and_tiny_limits_fail_without_partial_output() {
    let (source, target) = fixture();
    let p = prepared(&source, target, [true, true, false]);
    assert!(matches!(
        p.transport(&IntegralKey::try_new([1]).unwrap(), Default::default()),
        Err(Error::WrongInputArity { .. })
    ));
    assert!(matches!(
        p.transport(&key([1, 1, 1]), Default::default()),
        Err(Error::PositiveOutsideRoot { axis: 2, .. })
    ));
    assert!(matches!(
        p.transport(&key([1, 1, i64::MIN]), Default::default()),
        Err(Error::Expansion(ExpansionError::ResourceLimit { .. }))
    ));
    let tiny = ExpansionLimits {
        max_endpoints: 1,
        ..Default::default()
    };
    assert!(matches!(
        p.transport(&key([1, 1, -2]), tiny),
        Err(Error::Expansion(ExpansionError::ResourceLimit { .. }))
    ));
    let tiny = ExpansionLimits {
        max_factors: 0,
        ..Default::default()
    };
    assert!(matches!(
        p.transport(&key([1, 1, -1]), tiny),
        Err(Error::Expansion(ExpansionError::ResourceLimit { .. }))
    ));
    assert_eq!(p.transport(&key([1, 1, 0]), tiny).unwrap().terms().len(), 1);
}

#[test]
fn shared_prepared_map_is_deterministic_across_workers() {
    let (source, target) = fixture();
    let prepared = Arc::new(prepared(&source, target, [true, true, false]));
    let expected = prepared
        .transport(&key([2, 1, -3]), Default::default())
        .unwrap();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let owner = prepared.clone();
                scope.spawn(move || {
                    owner
                        .transport(&key([2, 1, -3]), Default::default())
                        .unwrap()
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap(), expected);
        }
    });
}

#[test]
fn rational_inactive_coefficients_are_native_and_scaled_active_rows_are_rejected() {
    let (_, target) = fixture();
    let rational = family(
        "rational",
        target.coefficient_context(),
        ["2", "0", "1/2", "0"],
        false,
    );
    let p = prepared(&rational, target.clone(), [true, true, false]);
    let c = target.coefficient_context();
    assert_eq!(
        terms(&p, [1, 1, -1]),
        BTreeMap::from([
            (key([1, 1, -1]), c.coefficient_fixture("1/2")),
            (key([1, 1, 0]), c.integer(2)),
        ])
    );
    let zero = c.zero();
    let one = c.one();
    let massless = Arc::new(
        IntegralFamily::new(
            "massless-transport",
            vec!["k1".into(), "k2".into()],
            vec![],
            c.clone(),
            c.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(zero.clone(), vec![one.clone(), zero.clone(), zero.clone()]),
                AffineDenominator::new(zero.clone(), vec![zero.clone(), zero.clone(), one.clone()]),
                AffineDenominator::new(zero.clone(), vec![zero.clone(), one, zero.clone()]),
            ],
            vec![],
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap(),
    );
    let map = Arc::new(
        verify(
            &massless,
            &massless,
            momentum(c, ["2", "0", "0", "1/2"]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(matches!(
        map.jacobian(),
        crate::sector::symmetry::Jacobian::Unit { .. }
    ));
    let root = Mask::try_new([true, true, false]).unwrap();
    assert!(matches!(
        compile(
            &massless,
            massless.clone(),
            map,
            root.clone(),
            root,
            Default::default()
        ),
        Err(Error::NonUnitActiveRow { axis: 0 })
    ));
}

#[test]
fn compiled_map_storage_and_total_rank_overflow_are_bounded() {
    let (source, target) = fixture();
    let map = Arc::new(
        verify(
            &source,
            &target,
            momentum(source.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    let root = Mask::try_new([true, true, false]).unwrap();
    let tiny = ExpansionLimits {
        max_relation_coefficient_entries: 1,
        ..Default::default()
    };
    assert!(matches!(
        compile(&source, target.clone(), map, root.clone(), root, tiny),
        Err(Error::Expansion(ExpansionError::ResourceLimit { .. }))
    ));
    let p = prepared(&source, target, [true, true, false]);
    assert!(matches!(
        p.transport(&key([i64::MIN, i64::MIN, 0]), Default::default()),
        Err(Error::Expansion(
            ExpansionError::ResourceCountOverflow { .. }
        ))
    ));
}

#[test]
fn selected_rows_are_cumulatively_admitted_before_coefficient_cloning() {
    let (source, target) = fixture();
    let p = prepared(&source, target, [true, true, false]);
    let limits = ExpansionLimits {
        max_retained_coefficient_terms: 8,
        ..Default::default()
    };
    assert!(
        matches!(p.transport(&key([-1, -1, -1]), limits), Err(Error::Expansion(ExpansionError::ResourceLimit { resource: "multi-affine retained coefficient terms", requested, limit: 8 })) if requested > 8)
    );
    let limits = ExpansionLimits {
        max_retained_coefficient_clone_owned_bytes: 1,
        ..Default::default()
    };
    assert!(matches!(
        p.transport(&key([1, 1, -1]), limits),
        Err(Error::Expansion(ExpansionError::ResourceLimit {
            resource: "multi-affine retained coefficient clone-owned bytes",
            ..
        }))
    ));
    // Rows which are not numerators are shared, not cloned or charged as ingress.
    let limits = ExpansionLimits {
        max_retained_coefficient_terms: 4,
        ..Default::default()
    };
    assert_eq!(
        p.transport(&key([1, 1, 0]), limits).unwrap().terms().len(),
        1
    );
}

#[test]
fn verified_denominator_identity_does_not_authorize_a_dimension_shift() {
    let (_, target) = fixture();
    let shifted = Arc::new(
        IntegralFamily::new(
            "dimension-shifted",
            target.loop_momenta().to_vec(),
            target.external_momenta().to_vec(),
            target.coefficient_context().clone(),
            target.coefficient_context().coefficient_fixture("d+2"),
            target.denominators().to_vec(),
            target.external_gram().to_vec(),
            target.power_shifts().to_vec(),
        )
        .unwrap(),
    );
    // The momentum verifier authenticates the algebraic denominator map,
    // deliberately not a dimension-recurrence relation between integrals.
    let map = Arc::new(
        verify(
            &target,
            &shifted,
            momentum(target.coefficient_context(), ["1", "0", "0", "1"]),
            Default::default(),
        )
        .unwrap(),
    );
    let root = Mask::try_new([true, true, false]).unwrap();
    assert!(matches!(
        compile(
            &target,
            shifted,
            map,
            root.clone(),
            root,
            Default::default()
        ),
        Err(Error::DimensionMismatch)
    ));
}
