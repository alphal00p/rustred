use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{ParametricIbpGenerator, ParametricRelation};

pub(super) fn guarded_tadpole_family() -> (CoefficientContext, IntegralFamily) {
    let base = CoefficientContext::new(["d", "m2", "x"]);
    let x = base.parameter("x").unwrap();
    let shifted_power = base.try_div(&base.one(), &x, Default::default()).unwrap();
    let family = IntegralFamily::new(
        "parametric-foundry-guarded-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            base.parameter("m2").unwrap(),
            vec![base.one()],
        )],
        Vec::new(),
        vec![shifted_power],
    )
    .unwrap();
    (base, family)
}

pub(super) fn sole_ordinary_relation(generator: &ParametricIbpGenerator<'_>) -> ParametricRelation {
    let batch = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(batch.len(), 1);
    let row = batch.generate(0);
    batch
        .complete(vec![row])
        .unwrap()
        .into_relations()
        .pop()
        .unwrap()
}

pub(super) fn tadpole_sources() -> (
    CoefficientContext,
    IndexedCoefficientContext,
    Vec<ParametricRelation>,
    String,
) {
    let base = CoefficientContext::new(["d", "m2"]);
    let family = IntegralFamily::new(
        "parametric-foundry-tadpole",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            base.parameter("m2").unwrap(),
            vec![base.one()],
        )],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let family_fingerprint = family.fingerprint().to_owned();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    (
        base,
        generator.context().clone(),
        relations,
        family_fingerprint,
    )
}

pub(crate) fn sunset_sources() -> (
    CoefficientContext,
    IndexedCoefficientContext,
    Vec<ParametricRelation>,
) {
    let base = CoefficientContext::new(["d", "s"]);
    let zero = base.zero();
    let one = base.one();
    let minus_s = base
        .try_neg(&base.parameter("s").unwrap(), Default::default())
        .unwrap();
    let family = IntegralFamily::new(
        "parametric-foundry-equal-mass-sunset",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_s.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_s.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_s, vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let relations = batch.complete(rows).unwrap().into_relations();
    (base, generator.context().clone(), relations)
}

pub(super) fn two_source_ibp_li_sources() -> (
    CoefficientContext,
    IndexedCoefficientContext,
    Vec<ParametricRelation>,
) {
    let base = CoefficientContext::new(["d"]);
    let zero = base.zero();
    let one = base.one();
    let family = IntegralFamily::new(
        "parametric-foundry-two-source-ibp-li",
        vec!["k".into()],
        vec!["p0".into(), "p1".into(), "p2".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                zero.clone(),
                vec![one.clone(), zero.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                zero.clone(),
                vec![zero.clone(), one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                zero.clone(),
                vec![zero.clone(), zero.clone(), one.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                zero.clone(),
                vec![zero.clone(), zero.clone(), zero.clone(), one.clone()],
            ),
        ],
        vec![
            vec![zero.clone(), one.clone(), one.clone()],
            vec![one.clone(), one.clone(), one.clone()],
            vec![one.clone(), one.clone(), base.integer(2)],
        ],
        vec![zero.clone(), zero.clone(), zero.clone(), zero],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary_batch = generator.prepare_ordinary_ibp().unwrap();
    let ordinary_rows = (0..ordinary_batch.len())
        .map(|ordinal| ordinary_batch.generate(ordinal))
        .collect();
    let ordinary = ordinary_batch.complete(ordinary_rows).unwrap();
    let li_batch = generator.prepare_lorentz_invariance(&ordinary).unwrap();
    let li = li_batch.generate(0).unwrap();
    drop(li_batch);
    let ordinary = ordinary.into_relations().into_iter().next().unwrap();
    (base, generator.context().clone(), vec![ordinary, li])
}
