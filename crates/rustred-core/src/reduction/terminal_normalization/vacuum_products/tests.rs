use super::*;
use std::collections::BTreeSet;

fn vacuum(momenta: &[Vec<i64>]) -> IntegralFamily {
    use crate::algebra::CoefficientContext;
    use crate::family::AffineDenominator;
    let loops = momenta[0].len();
    let c = CoefficientContext::new(["d"]);
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let coefficients = (0..loops)
                .flat_map(|i| {
                    (i..loops).map(move |j| momentum[i] * momentum[j] * if i == j { 1 } else { 2 })
                })
                .map(|value| c.integer(value))
                .collect();
            AffineDenominator::new(c.integer(-1), coefficients)
        })
        .collect();
    IntegralFamily::new(
        "product-test",
        (0..loops).map(|i| format!("k{i}")).collect(),
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![c.zero(); momenta.len()],
    )
    .unwrap()
}

fn family(loops: usize) -> IntegralFamily {
    let mut rows = Vec::new();
    for i in 0..loops {
        let mut row = vec![0; loops];
        row[i] = 1;
        rows.push(row);
    }
    for i in 0..loops {
        for j in i + 1..loops {
            let mut row = vec![0; loops];
            row[i] = 1;
            row[j] = -1;
            rows.push(row);
        }
    }
    vacuum(&rows)
}
fn key(powers: impl IntoIterator<Item = i64>) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

#[test]
fn independent_tadpoles_become_sorted_symbolic_factors_at_arbitrary_loop_count() {
    for loops in [2, 3, 5] {
        let family = family(loops);
        let mut powers = vec![0; family.denominator_count()];
        for i in 0..loops {
            powers[i] = (i + 1) as i64;
        }
        let target = key(powers);
        let plan = VacuumProductPlan::prepare(
            &family,
            &BTreeSet::from([target.clone()]),
            Default::default(),
        )
        .unwrap();
        let product = &plan.products()[&target];
        assert_eq!(product.coefficient(), &family.coefficient_context().one());
        assert_eq!(product.factors().len(), loops);
        assert_eq!(plan.families().len(), 1);
        assert_eq!(
            product
                .factors()
                .iter()
                .map(|factor| factor.integral().powers()[0])
                .collect::<Vec<_>>(),
            (1..=loops as i64).collect::<Vec<_>>()
        );
        assert_eq!(plan.families().values().next().unwrap().loop_count(), 1);
    }
}

#[test]
fn two_correlated_blocks_and_tadpole_are_a_verified_five_loop_product() {
    let family = family(5);
    // q0,q1,(q0-q1); q2,q3,(q2-q3); q4. Other powers are zero.
    let mut powers = vec![0; 15];
    powers[..5].fill(1);
    powers[5] = 1;
    powers[12] = 1;
    let target = key(powers);
    let plan = VacuumProductPlan::prepare(
        &family,
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    let mut loops = plan.products()[&target]
        .factors()
        .iter()
        .map(|factor| plan.families()[factor.family_fingerprint()].loop_count())
        .collect::<Vec<_>>();
    loops.sort();
    assert_eq!(loops, [1, 2, 2]);
    let pair = plan.products()[&target]
        .factors()
        .iter()
        .filter(|factor| plan.families()[factor.family_fingerprint()].loop_count() == 2)
        .collect::<Vec<_>>();
    assert_eq!(pair[0], pair[1]);
    assert_eq!(pair[0].integral().powers(), [1, 1, 1]);
}

#[test]
fn changed_unimodular_parent_basis_and_dots_keep_canonical_product_keys() {
    let plain = family(3);
    // Apply q0=k0+k1, q1=k1, q2=-k2 to every propagator momentum.
    let changed = vacuum(&[
        vec![1, 1, 0],
        vec![0, 1, 0],
        vec![0, 0, -1],
        vec![1, 0, 0],
        vec![1, 1, 1],
        vec![0, 1, 1],
    ]);
    let target = key([2, 3, 4, 5, 0, 0]);
    let targets = BTreeSet::from([target.clone()]);
    let a = VacuumProductPlan::prepare(&plain, &targets, Default::default()).unwrap();
    let b = VacuumProductPlan::prepare(&changed, &targets, Default::default()).unwrap();
    assert_eq!(a.products()[&target], b.products()[&target]);
    assert_eq!(a.products()[&target].factors().len(), 2);
}

#[test]
fn numerator_coupling_connected_and_nonunimodular_keys_remain_flat() {
    let family = family(2);
    let targets = BTreeSet::from([key([1, 1, -2]), key([1, 1, 1]), key([1, 0, 0])]);
    assert!(
        VacuumProductPlan::prepare(&family, &targets, Default::default())
            .unwrap()
            .products()
            .is_empty()
    );
    let scaled = vacuum(&[vec![2, 0], vec![0, 1], vec![1, -1]]);
    assert!(
        VacuumProductPlan::prepare(
            &scaled,
            &BTreeSet::from([key([1, 1, 0])]),
            Default::default()
        )
        .unwrap()
        .products()
        .is_empty()
    );
}

#[test]
fn support_preparation_is_reused_and_search_limits_fail_explicitly() {
    let family = family(3);
    let targets = BTreeSet::from([key([1, 1, 1, 0, 0, 0]), key([3, 2, 1, 0, 0, 0])]);
    let plan = VacuumProductPlan::prepare(&family, &targets, Default::default()).unwrap();
    assert_eq!(plan.statistics().analyzed_supports, 1);
    assert_eq!(plan.statistics().factored_terminals, 2);
    assert!(matches!(
        VacuumProductPlan::prepare(
            &family,
            &targets,
            VacuumProductLimits {
                max_canonical_candidates: 0,
                ..Default::default()
            }
        ),
        Err(VacuumProductError::ResourceLimit { .. })
    ));
}
