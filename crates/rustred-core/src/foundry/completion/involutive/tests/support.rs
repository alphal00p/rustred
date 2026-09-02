use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::CompletionGeometryLimits;
use super::super::*;

pub(super) fn context(arity: usize) -> IndexedCoefficientContext {
    let base = CoefficientContext::new(std::iter::empty::<&str>());
    IndexedCoefficientContext::try_new(&base, "involutive-synthetic-tests", arity).unwrap()
}

pub(super) fn active_ordering(arity: usize, limits: InvolutiveLimits) -> OreOrderingAdapter {
    OreOrderingAdapter::try_new(
        OrderingPolicy::default(),
        Mask::try_new(std::iter::repeat_n(true, arity)).unwrap(),
        limits,
    )
    .unwrap()
}

pub(super) fn shift(values: &[u64], limits: InvolutiveLimits) -> ForwardShift {
    ForwardShift::try_new(values.iter().copied(), limits).unwrap()
}

pub(super) fn monomial_consequence(
    source_ordinal: usize,
    powers: &[u64],
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
) -> OreConsequence {
    let row = OreRow::try_new(
        ordering,
        [(shift(powers, limits), context.one())],
        context,
        limits,
    )
    .unwrap();
    OreConsequence::try_from_source(source_ordinal, row, ordering, context, limits).unwrap()
}

pub(super) fn epoch(
    leaders: &[&[u64]],
    context: &IndexedCoefficientContext,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
) -> JanetBasisEpoch {
    let consequences = leaders
        .iter()
        .enumerate()
        .map(|(ordinal, powers)| monomial_consequence(ordinal, powers, ordering, context, limits));
    JanetBasisEpoch::try_initial(
        consequences,
        ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap()
}

pub(super) fn two_step_nonconstant_fixture(
    context: &IndexedCoefficientContext,
    ordering: &OreOrderingAdapter,
    limits: InvolutiveLimits,
) -> (JanetBasisEpoch, OreConsequence) {
    let zero = shift(&[0], limits);
    let e = shift(&[1], limits);
    let e2 = shift(&[2], limits);
    let n = context.index(0).unwrap();
    let n_plus_one = context.add(&n, &context.one()).unwrap();
    let divisor = OreConsequence::try_from_source(
        0,
        OreRow::try_new(
            ordering,
            [(e.clone(), n), (zero.clone(), context.one())],
            context,
            limits,
        )
        .unwrap(),
        ordering,
        context,
        limits,
    )
    .unwrap();
    let basis = JanetBasisEpoch::try_initial(
        [divisor],
        ordering,
        context,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let subject = OreConsequence::try_from_source(
        1,
        OreRow::try_new(
            ordering,
            [
                (e2, n_plus_one.clone()),
                (e, n_plus_one),
                (zero, context.one()),
            ],
            context,
            limits,
        )
        .unwrap(),
        ordering,
        context,
        limits,
    )
    .unwrap();
    (basis, subject)
}
