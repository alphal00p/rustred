use crate::algebra::CoefficientContext;
use crate::family::IntegralFamily;
use crate::identity::row::RowId;

use super::super::ParametricIbpGenerator;
use super::support::{
    assert_coefficient_eq, identity_denominators, indexed_coefficient_for_shift,
    specialize_for_test,
};

#[test]
fn one_loop_li_has_litered_sign_and_weighted_denominator_shifts() {
    let base = CoefficientContext::new(["d", "s00", "s11", "c1", "c2", "nu0", "nu1", "nu2"]);
    let s00 = base.parameter("s00").unwrap();
    let s11 = base.parameter("s11").unwrap();
    let c1 = base.parameter("c1").unwrap();
    let c2 = base.parameter("c2").unwrap();
    let nu1 = base.parameter("nu1").unwrap();
    let nu2 = base.parameter("nu2").unwrap();
    let family = IntegralFamily::new(
        "one-loop-two-leg-li",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        identity_denominators(&base, vec![base.zero(), c1.clone(), c2.clone()]),
        vec![
            vec![s00.clone(), base.zero()],
            vec![base.zero(), s11.clone()],
        ],
        vec![base.parameter("nu0").unwrap(), nu1.clone(), nu2.clone()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
    let ordinary_rows = (0..ordinary_batch.len())
        .map(|ordinal| ordinary_batch.generate(ordinal))
        .collect();
    let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
    let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
    let lorentz_invariance = (0..li_batch.len())
        .map(|ordinal| li_batch.generate(ordinal))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    drop(li_batch);
    let ordinary = ordinary.into_relations();

    assert_eq!(ordinary.len(), 3);
    assert_eq!(lorentz_invariance.len(), 1);
    assert_eq!(
        lorentz_invariance[0].row_id(),
        &RowId::LorentzInvariance {
            first_external: 0,
            second_external: 1,
        }
    );
    let relation = &lorentz_invariance[0];
    assert_eq!(relation.terms().len(), 4);
    let n1 = &base.integer(3) + &nu1;
    let n2 = &base.integer(4) + &nu2;
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0, 1, 0]).unwrap(),
            &[2, 3, 4],
        ),
        &(&(&c2 * &s00) * &n1),
    );
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0, 1, -1]).unwrap(),
            &[2, 3, 4],
        ),
        &(-(&s00 * &n1)),
    );
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0, 0, 1]).unwrap(),
            &[2, 3, 4],
        ),
        &(-(&(&c1 * &s11) * &n2)),
    );
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0, -1, 1]).unwrap(),
            &[2, 3, 4],
        ),
        &(&s11 * &n2),
    );
}
