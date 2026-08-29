use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::row::RowId;

use super::super::ParametricIbpGenerator;
use super::support::{assert_coefficient_eq, indexed_coefficient_for_shift, specialize_for_test};

#[test]
fn one_loop_tadpole_is_a_fully_parametric_recurrence() {
    let base = CoefficientContext::new(["d", "m2", "nu"]);
    let d = base.parameter("d").unwrap();
    let m2 = base.parameter("m2").unwrap();
    let nu = base.parameter("nu").unwrap();
    let family = IntegralFamily::new(
        "one-loop-tadpole-parametric",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        d.clone(),
        vec![AffineDenominator::new(m2.clone(), vec![base.one()])],
        Vec::new(),
        vec![nu.clone()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
    let ordinary_rows = (0..ordinary_batch.len())
        .map(|ordinal| ordinary_batch.generate(ordinal))
        .collect();
    let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
    let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
    assert_eq!(li_batch.len(), 0);
    drop(li_batch);
    let ordinary = ordinary.into_relations();

    assert_eq!(ordinary.len(), 1);
    assert_eq!(
        ordinary[0].row_id(),
        &RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 0,
        }
    );
    let relation = &ordinary[0];
    assert_eq!(relation.terms().len(), 2);
    let shifted_power = &base.integer(3) + &nu;
    let expected_same = &d - &(&base.integer(2) * &shifted_power);
    let expected_raised = &(&base.integer(2) * &m2) * &shifted_power;
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0]).unwrap(),
            &[3],
        ),
        &expected_same,
    );
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[1]).unwrap(),
            &[3],
        ),
        &expected_raised,
    );

    // Sector signs are determined by the raw index, but a power shift is
    // still present in the coefficient at n=0. Raw generation must not use
    // a concrete zero-index shortcut from the discarded vacuum prototype.
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[0]).unwrap(),
            &[0],
        ),
        &(&d - &(&base.integer(2) * &nu)),
    );
    assert_coefficient_eq(
        &specialize_for_test(
            generator.context(),
            indexed_coefficient_for_shift(relation, &[1]).unwrap(),
            &[0],
        ),
        &(&(&base.integer(2) * &m2) * &nu),
    );
}
