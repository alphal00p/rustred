use symbolica::atom::{
    Atom, FunctionBuilder, NamespacedSymbol, Symbol, SymbolAttribute, SymbolBuilder,
};

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::presentation::{
    CommonMassScale, DenominatorRole, FamilyConventions, FamilyPresentation, MetricConvention,
    MomentumCombination, MomentumRouting, PhysicalPropagator, PropagatorConvention,
};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey, ScalarProductCoordinate};

use super::*;

mod auxiliary;

fn symbol(name: &str) -> Symbol {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse(format!("rustred_tensor_tests::{name}"))
            .expect("test symbols are namespaced"),
    )
    .build()
    .expect("test symbol registration must be stable")
}

fn attributed_symbol(name: &str, attributes: Vec<SymbolAttribute>) -> Symbol {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse(format!("rustred_tensor_tests::{name}"))
            .expect("test symbols are namespaced"),
    )
    .with_attributes(attributes)
    .build()
    .expect("test attributed-symbol registration must be stable")
}

fn heads_with_linear_dot() -> TensorHeads {
    TensorHeads::try_new(
        symbol("loop_vector"),
        symbol("external_vector"),
        attributed_symbol("metric", vec![SymbolAttribute::Symmetric]),
        attributed_symbol(
            "linear_dot",
            vec![SymbolAttribute::Symmetric, SymbolAttribute::Linear],
        ),
    )
    .unwrap()
}

fn one_loop_presentation() -> (FamilyPresentation, CoefficientContext) {
    one_loop_presentation_with_dimension(None)
}

fn one_loop_presentation_with_dimension(
    constant_dimension: Option<i64>,
) -> (FamilyPresentation, CoefficientContext) {
    let context = CoefficientContext::new(["d", "m2"]);
    let mass = context.parameter("m2").unwrap();
    let family = IntegralFamily::new(
        "tensor-one-loop-vacuum",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        constant_dimension
            .map(|value| context.integer(value))
            .unwrap_or_else(|| context.parameter("d").unwrap()),
        vec![AffineDenominator::new(mass.clone(), vec![context.one()])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    let presentation = FamilyPresentation::try_new(
        family,
        vec![DenominatorRole::Physical(PhysicalPropagator::new(
            "D".to_owned(),
            MomentumCombination::new(vec![context.one()], Vec::new()),
            mass.clone(),
        ))],
        MomentumRouting::new(
            vec!["source-k".into()],
            Vec::new(),
            vec![vec![context.one()]],
            vec![Vec::new()],
            Vec::new(),
        ),
        FamilyConventions::new(
            MetricConvention::Euclidean,
            PropagatorConvention::MOMENTUM_SQUARED_PLUS_MASS_SQUARED,
        ),
        Some(CommonMassScale::new(mass)),
    )
    .unwrap();
    (presentation, context)
}

fn call(head: Symbol, left: impl Into<Atom>, right: impl Into<Atom>) -> Atom {
    FunctionBuilder::new(head)
        .add_arg(left.into())
        .add_arg(right.into())
        .finish()
}

#[test]
fn custom_heads_reduce_the_rank_two_single_scale_sentinel() {
    let (presentation, context) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("loop_id").to_atom();
    let external_id = symbol("external_id").to_atom();
    let mu = symbol("mu").to_atom();
    let opaque = FunctionBuilder::new(symbol("opaque"))
        .add_arg(symbol("z"))
        .finish();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], vec![external_id.clone()]),
        TensorLimits::default(),
    )
    .unwrap();
    let numerator = Atom::mul_many(vec![
        call(heads.dot(), loop_id.clone(), external_id.clone()),
        call(heads.loop_vector(), loop_id, mu.clone()),
        opaque.clone(),
    ]);
    let base = IntegralKey::try_new([1]).unwrap();

    let projection = service.project(&numerator, &base).unwrap();
    assert_eq!(projection.lane(), ResolvedTensorLane::SingleScaleVacuum);
    assert_eq!(projection.terms().len(), 1);
    assert_eq!(
        projection.terms()[0].scalar_products(),
        &[ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }]
    );
    assert_eq!(projection.terms()[0].scalar_spectator(), &opaque);
    assert_eq!(
        projection.terms()[0].outside_tensor(),
        &call(heads.external_vector(), external_id, mu)
    );
    assert_eq!(projection.guards().len(), 1);
    assert_eq!(
        projection.guards()[0].origin(),
        TensorGuardOrigin::RankTwoProjectorDimension
    );
    assert_eq!(
        projection.guards()[0].polynomial(),
        &context.parameter("d").unwrap().numerator
    );

    let lowered = service.lower_scalar_products(&projection, &base).unwrap();
    assert_eq!(lowered.terms().len(), 2);
    assert_eq!(lowered.terms()[0].integral().powers(), &[0]);
    assert_eq!(lowered.terms()[1].integral().powers(), &[1]);
    let dimension = context.parameter("d").unwrap();
    let key_zero_coefficient = context
        .try_mul(
            lowered.terms()[0].coefficient(),
            &dimension,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(key_zero_coefficient, context.one());
    let key_one_coefficient = context
        .try_mul(
            lowered.terms()[1].coefficient(),
            &dimension,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    let minus_mass = context
        .try_neg(
            &context.parameter("m2").unwrap(),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(key_one_coefficient, minus_mass);
    assert_eq!(lowered.guards(), projection.guards());

    let composed = service.reduce(&numerator, &base).unwrap();
    assert_eq!(composed.terms(), lowered.terms());
    assert_eq!(
        composed.family_fingerprint(),
        presentation.family().fingerprint()
    );
}

#[test]
fn scalar_spectators_survive_and_odd_rank_is_exact_zero() {
    let (presentation, context) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("scalar_odd_loop_id").to_atom();
    let external_id = symbol("scalar_odd_external_id").to_atom();
    let mu = FunctionBuilder::new(symbol("decorated_index"))
        .add_arg(symbol("mu_atom"))
        .finish();
    let spectator = FunctionBuilder::new(symbol("scalar_spectator"))
        .add_arg(symbol("regulator"))
        .finish();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::SingleScaleVacuum,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], vec![external_id]),
        TensorLimits::default(),
    )
    .unwrap();
    let base = IntegralKey::try_new([1]).unwrap();

    let scalar = service.reduce(&spectator, &base).unwrap();
    assert_eq!(scalar.terms().len(), 1);
    assert_eq!(scalar.terms()[0].scalar_spectator(), &spectator);
    assert_eq!(scalar.terms()[0].outside_tensor(), &Atom::num(1));
    assert_eq!(scalar.terms()[0].coefficient(), &context.one());
    assert!(scalar.guards().is_empty());

    let odd = call(heads.loop_vector(), loop_id, mu);
    let projected = service.project(&odd, &base).unwrap();
    assert!(projected.is_zero());
    assert!(projected.guards().is_empty());
    assert!(service.reduce(&odd, &base).unwrap().is_zero());
}

#[test]
fn opaque_scalars_cannot_hide_loop_momentum_dependence() {
    let (presentation, _) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("hidden_loop_id").to_atom();
    let mu = symbol("hidden_mu").to_atom();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], Vec::new()),
        TensorLimits::default(),
    )
    .unwrap();
    let base = IntegralKey::try_new([1]).unwrap();
    let hidden = FunctionBuilder::new(symbol("hidden_loop_dependence"))
        .add_arg(loop_id.clone())
        .finish();
    let numerator = Atom::mul_many(vec![hidden, call(heads.loop_vector(), loop_id, mu)]);
    assert!(matches!(
        service.project(&numerator, &base),
        Err(TensorError::LoopMomentumInOpaqueScalar { .. })
    ));
    let hidden_sum = Atom::add_many(vec![
        FunctionBuilder::new(symbol("hidden_sum_dependence"))
            .add_arg(symbol("hidden_loop_id"))
            .finish(),
        Atom::num(1),
    ]);
    let nested = Atom::mul_many(vec![
        hidden_sum,
        call(
            heads.loop_vector(),
            symbol("hidden_loop_id"),
            symbol("hidden_sum_mu"),
        ),
    ]);
    assert!(matches!(
        service.project(&nested, &base),
        Err(TensorError::LoopMomentumInOpaqueScalar { .. })
    ));

    let numeric_service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![Atom::num(1)], Vec::new()),
        TensorLimits::default(),
    )
    .unwrap();
    let numeric_spectator = FunctionBuilder::new(symbol("numeric_id_spectator"))
        .add_arg(Atom::num(1))
        .finish();
    assert_eq!(
        numeric_service
            .project(&numeric_spectator, &base)
            .unwrap()
            .terms()
            .len(),
        1
    );
}

#[test]
fn heads_and_reserved_ingress_are_strictly_authenticated() {
    let linear_dot = attributed_symbol(
        "accepted_vakint_dot",
        vec![SymbolAttribute::Symmetric, SymbolAttribute::Linear],
    );
    assert!(
        TensorHeads::try_new(
            symbol("accepted_k"),
            symbol("accepted_p"),
            attributed_symbol("accepted_metric", vec![SymbolAttribute::Symmetric]),
            linear_dot,
        )
        .is_ok()
    );
    let duplicate = symbol("duplicate_head");
    assert!(matches!(
        TensorHeads::try_new(
            duplicate,
            duplicate,
            symbol("duplicate_g"),
            symbol("duplicate_dot")
        ),
        Err(TensorHeadError::Duplicate {
            first: TensorHeadKind::LoopVector,
            second: TensorHeadKind::ExternalVector,
        })
    ));

    let (presentation, _) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("malformed_loop_id").to_atom();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], Vec::new()),
        TensorLimits::default(),
    )
    .unwrap();
    let wrong_arity = FunctionBuilder::new(heads.loop_vector())
        .add_arg(loop_id)
        .finish();
    let base = IntegralKey::try_new([1]).unwrap();
    assert!(matches!(
        service.project(&wrong_arity, &base),
        Err(TensorError::MalformedReservedHead {
            head: TensorHeadKind::LoopVector,
            expected_arity: 2,
            actual_arity: Some(1),
        })
    ));
    assert!(matches!(
        service.project(&heads.dot().to_atom(), &base),
        Err(TensorError::MalformedReservedHead {
            head: TensorHeadKind::Dot,
            actual_arity: None,
            ..
        })
    ));
}

#[test]
fn unsupported_and_resource_frontiers_are_typed() {
    let (presentation, _) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("frontier_loop_id").to_atom();
    let momenta = TensorMomenta::new(vec![loop_id], Vec::new());
    assert!(matches!(
        TensorService::try_new(
            &presentation,
            TensorLane::Generic,
            heads,
            momenta.clone(),
            TensorLimits::default(),
        ),
        Err(TensorError::UnsupportedGenericKinematics)
    ));
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        momenta,
        TensorLimits {
            max_input_terms: 0,
            ..TensorLimits::default()
        },
    )
    .unwrap();
    let base = IntegralKey::try_new([1]).unwrap();
    assert!(matches!(
        service.project(&Atom::num(1), &base),
        Err(TensorError::ResourceLimit {
            resource: "tensor numerator terms",
            requested: 1,
            limit: 0,
        })
    ));

    let comparison_limited = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![symbol("bounded_loop_id").to_atom()], Vec::new()),
        TensorLimits {
            max_loop_momentum_label_checks: 0,
            ..TensorLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        comparison_limited.project(&Atom::num(1), &base),
        Err(TensorError::ResourceLimit {
            resource: "opaque-scalar loop-momentum label checks",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn retained_tensor_index_contractions_are_rejected_until_canonicalized() {
    let (presentation, _) = one_loop_presentation();
    let heads = heads_with_linear_dot();
    let loop_id = symbol("collision_loop_id").to_atom();
    let external_id = symbol("collision_external_id").to_atom();
    let mu = symbol("collision_mu").to_atom();
    let nu = symbol("collision_nu").to_atom();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], vec![external_id.clone()]),
        TensorLimits::default(),
    )
    .unwrap();
    let k_mu = call(heads.loop_vector(), loop_id.clone(), mu.clone());
    let k_nu = call(heads.loop_vector(), loop_id, nu.clone());
    let base = IntegralKey::try_new([1]).unwrap();

    let metric_contraction = Atom::mul_many(vec![
        call(heads.metric(), mu.clone(), nu.clone()),
        k_mu.clone(),
        k_nu.clone(),
    ]);
    assert!(matches!(
        service.project(&metric_contraction, &base),
        Err(TensorError::UnsupportedLorentzIndexContraction { .. })
    ));
    let external_contraction = Atom::mul_many(vec![
        call(heads.external_vector(), external_id, mu.clone()),
        k_mu.clone(),
        k_nu,
    ]);
    assert!(matches!(
        service.project(&external_contraction, &base),
        Err(TensorError::UnsupportedLorentzIndexContraction { .. })
    ));
    let repeated_free_index = Atom::mul_many(vec![k_mu.clone(), k_mu]);
    assert!(matches!(
        service.project(&repeated_free_index, &base),
        Err(TensorError::ReservedHeadInUnsupportedPosition {
            head: TensorHeadKind::LoopVector,
        })
    ));
}

#[test]
fn nonzero_constant_dimension_needs_no_exceptional_guard() {
    let (presentation, _) = one_loop_presentation_with_dimension(Some(4));
    let heads = heads_with_linear_dot();
    let loop_id = symbol("constant_d_loop_id").to_atom();
    let external_id = symbol("constant_d_external_id").to_atom();
    let mu = symbol("constant_d_mu").to_atom();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], vec![external_id.clone()]),
        TensorLimits::default(),
    )
    .unwrap();
    let numerator = Atom::mul_many(vec![
        call(heads.dot(), loop_id.clone(), external_id),
        call(heads.loop_vector(), loop_id, mu),
    ]);
    let projection = service
        .project(&numerator, &IntegralKey::try_new([1]).unwrap())
        .unwrap();
    assert_eq!(projection.terms().len(), 1);
    assert!(projection.guards().is_empty());
}

#[test]
fn identically_zero_dimension_is_rejected_before_exact_division() {
    let (presentation, _) = one_loop_presentation_with_dimension(Some(0));
    let heads = heads_with_linear_dot();
    let loop_id = symbol("zero_d_loop_id").to_atom();
    let external_id = symbol("zero_d_external_id").to_atom();
    let mu = symbol("zero_d_mu").to_atom();
    let service = TensorService::try_new(
        &presentation,
        TensorLane::Auto,
        heads,
        TensorMomenta::new(vec![loop_id.clone()], vec![external_id.clone()]),
        TensorLimits::default(),
    )
    .unwrap();
    let numerator = Atom::mul_many(vec![
        call(heads.dot(), loop_id.clone(), external_id),
        call(heads.loop_vector(), loop_id, mu),
    ]);
    assert!(matches!(
        service.project(&numerator, &IntegralKey::try_new([1]).unwrap()),
        Err(TensorError::SingularDimension)
    ));
}
