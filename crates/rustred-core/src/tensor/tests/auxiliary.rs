use symbolica::atom::{Atom, NamespacedSymbol, Symbol, SymbolBuilder};

use crate::algebra::CoefficientContext;
use crate::family::presentation::{
    AuxiliaryDenominator, CommonMassScale, DenominatorRole, FamilyConventions, FamilyPresentation,
    MetricConvention, MomentumCombination, MomentumRouting, PhysicalPropagator,
    PropagatorConvention,
};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};

use super::super::{
    TensorError, TensorHeads, TensorLane, TensorLimits, TensorMomenta, TensorService,
};

fn symbol(name: &str) -> Symbol {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse(format!("rustred_tensor_aux_tests::{name}"))
            .expect("test symbols are namespaced"),
    )
    .build()
    .expect("test symbol registration must be stable")
}

fn presentation(auxiliary_power_shift: bool) -> FamilyPresentation {
    let context = CoefficientContext::new(["d", "m2"]);
    let mass = context.parameter("m2").unwrap();
    let row = |constant, active| {
        AffineDenominator::new(
            constant,
            (0..3)
                .map(|coordinate| {
                    if coordinate == active {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect(),
        )
    };
    let family = IntegralFamily::new(
        "tensor-two-loop-auxiliary",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            row(mass.clone(), 0),
            row(mass.clone(), 2),
            row(context.zero(), 1),
        ],
        Vec::new(),
        vec![
            context.zero(),
            context.zero(),
            if auxiliary_power_shift {
                context.one()
            } else {
                context.zero()
            },
        ],
    )
    .unwrap();
    FamilyPresentation::try_new(
        family,
        vec![
            DenominatorRole::Physical(PhysicalPropagator::new(
                "D0".to_owned(),
                MomentumCombination::new(vec![context.one(), context.zero()], Vec::new()),
                mass.clone(),
            )),
            DenominatorRole::Physical(PhysicalPropagator::new(
                "D1".to_owned(),
                MomentumCombination::new(vec![context.zero(), context.one()], Vec::new()),
                mass.clone(),
            )),
            DenominatorRole::Auxiliary(AuxiliaryDenominator::new("ISP01".to_owned())),
        ],
        MomentumRouting::new(
            vec!["source-k0".into(), "source-k1".into()],
            Vec::new(),
            vec![
                vec![context.one(), context.zero()],
                vec![context.zero(), context.one()],
            ],
            vec![Vec::new(), Vec::new()],
            Vec::new(),
        ),
        FamilyConventions::new(
            MetricConvention::Euclidean,
            PropagatorConvention::MOMENTUM_SQUARED_PLUS_MASS_SQUARED,
        ),
        Some(CommonMassScale::new(mass)),
    )
    .unwrap()
}

fn service(presentation: &FamilyPresentation) -> TensorService<'_> {
    TensorService::try_new(
        presentation,
        TensorLane::Auto,
        TensorHeads::try_new(
            symbol("k_head"),
            symbol("p_head"),
            symbol("g_head"),
            symbol("dot_head"),
        )
        .unwrap(),
        TensorMomenta::new(
            vec![symbol("k0_id").to_atom(), symbol("k1_id").to_atom()],
            Vec::new(),
        ),
        TensorLimits::default(),
    )
    .unwrap()
}

#[test]
fn lower_and_reduce_reject_invisible_auxiliary_integral_content() {
    let presentation = presentation(false);
    let service = service(&presentation);
    let eligible = IntegralKey::try_new([1, 1, 0]).unwrap();
    let projection = service.project(&Atom::num(1), &eligible).unwrap();
    let negative_auxiliary = IntegralKey::try_new([1, 1, -1]).unwrap();
    assert!(matches!(
        service.project(&Atom::num(1), &negative_auxiliary),
        Err(TensorError::UnsupportedAuxiliaryIntegral {
            denominator: 2,
            power: -1,
        })
    ));
    assert!(matches!(
        service.lower_scalar_products(&projection, &negative_auxiliary),
        Err(TensorError::UnsupportedAuxiliaryIntegral {
            denominator: 2,
            power: -1,
        })
    ));
    let positive_auxiliary = IntegralKey::try_new([1, 1, 1]).unwrap();
    assert!(matches!(
        service.reduce(&Atom::num(1), &positive_auxiliary),
        Err(TensorError::UnsupportedAuxiliaryIntegral {
            denominator: 2,
            power: 1,
        })
    ));
    assert!(
        service
            .reduce(&Atom::num(1), &IntegralKey::try_new([1, 1, 0]).unwrap())
            .is_ok()
    );
}

#[test]
fn nonzero_auxiliary_power_shift_is_a_typed_boundary() {
    let presentation = presentation(true);
    let service = service(&presentation);
    assert!(matches!(
        service.reduce(&Atom::num(1), &IntegralKey::try_new([1, 1, 0]).unwrap()),
        Err(TensorError::UnsupportedAuxiliaryPowerShift { denominator: 2 })
    ));
}
