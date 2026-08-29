use crate::algebra::{
    Coefficient, CoefficientContext, IndexedAlgebraLimits, IndexedCoefficient,
    IndexedCoefficientContext,
};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::relation::ParametricRelation;

pub(super) fn indexed_coefficient_for_shift<'a>(
    relation: &'a ParametricRelation,
    shift: &[i64],
) -> Option<&'a IndexedCoefficient> {
    relation
        .terms()
        .iter()
        .find_map(|(candidate, coefficient)| (candidate.values() == shift).then_some(coefficient))
}

pub(super) fn specialize_for_test(
    context: &IndexedCoefficientContext,
    coefficient: &IndexedCoefficient,
    assignment: &[i64],
) -> Coefficient {
    context
        .specialize(coefficient, assignment, IndexedAlgebraLimits::default())
        .unwrap()
        .0
}

pub(super) fn assert_coefficient_eq(left: &Coefficient, right: &Coefficient) {
    assert!((left - right).is_zero(), "left={left}, right={right}");
}

pub(super) fn identity_denominators(
    context: &CoefficientContext,
    constants: Vec<Coefficient>,
) -> Vec<AffineDenominator> {
    let size = constants.len();
    constants
        .into_iter()
        .enumerate()
        .map(|(row, constant)| {
            AffineDenominator::new(
                constant,
                (0..size)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn coordinate_family(name: &str, loops: usize, externals: usize) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    let arity = loops * (loops + 1) / 2 + loops * externals;
    let external_gram = (0..externals)
        .map(|row| {
            (0..externals)
                .map(|column| {
                    if row == column {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect()
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..loops).map(|loop_| format!("k{loop_}")).collect(),
        (0..externals)
            .map(|external| format!("p{external}"))
            .collect(),
        context.clone(),
        context.parameter("d").unwrap(),
        identity_denominators(&context, vec![context.integer(-1); arity]),
        external_gram,
        vec![context.zero(); arity],
    )
    .unwrap()
}
