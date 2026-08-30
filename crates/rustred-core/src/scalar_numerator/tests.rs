use std::collections::BTreeMap;

use symbolica::atom::{
    Atom, AtomCore, FunctionBuilder, NamespacedSymbol, Symbol, SymbolAttribute, SymbolBuilder,
};

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::reduction::Reducer;

use super::*;

fn symbol(name: &str) -> Symbol {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse(format!("rustred_scalar_numerator_tests::{name}"))
            .expect("test symbols are namespaced"),
    )
    .build()
    .expect("test symbol registration must be stable")
}

fn dot_head() -> Symbol {
    SymbolBuilder::new(
        NamespacedSymbol::try_parse("rustred_scalar_numerator_tests::dot")
            .expect("test dot is namespaced"),
    )
    .with_attributes(vec![SymbolAttribute::Symmetric, SymbolAttribute::Linear])
    .build()
    .expect("test dot registration must be stable")
}

fn dot(head: Symbol, left: impl Into<Atom>, right: impl Into<Atom>) -> Atom {
    FunctionBuilder::new(head)
        .add_arg(left.into())
        .add_arg(right.into())
        .finish()
}

fn coefficient(term: &LoweredScalarNumeratorTerm) -> String {
    term.coefficient().to_expression().to_canonical_string()
}

#[test]
fn one_loop_dot_square_lowers_with_physical_mass_bookkeeping() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("k1").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let numerator = dot(head, loop_momentum.clone(), loop_momentum).pow(Atom::num(2));
    let lowering = service
        .lower(&numerator, &IntegralKey::try_new([3]).unwrap())
        .unwrap();

    let observed = lowering
        .terms()
        .iter()
        .map(|term| {
            (
                term.integral().powers().to_vec(),
                coefficient(term),
                term.common_mass_squared_power(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            (vec![1], "1".to_owned(), 0),
            (vec![2], "2".to_owned(), 1),
            (vec![3], "1".to_owned(), 2),
        ]
    );
}

#[test]
fn two_loop_cross_product_uses_the_complete_artifact_basis() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let head = dot_head();
    let k1 = symbol("cross_k1").to_atom();
    let k2 = symbol("cross_k2").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![k1.clone(), k2.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let base = IntegralKey::try_new([1, 1, 1]).unwrap();
    let lowering = service
        .lower(&dot(head, k1.clone(), k2.clone()), &base)
        .unwrap();

    let observed = lowering
        .terms()
        .iter()
        .map(|term| {
            (
                term.integral().powers().to_vec(),
                coefficient(term),
                term.common_mass_squared_power(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            (vec![0, 1, 1], "-1/2".to_owned(), 0),
            (vec![1, 0, 1], "-1/2".to_owned(), 0),
            (vec![1, 1, 0], "1/2".to_owned(), 0),
            (vec![1, 1, 1], "-1/2".to_owned(), 1),
        ]
    );
    assert_eq!(
        service.lower(&dot(head, k2, k1), &base).unwrap().terms(),
        lowering.terms()
    );
    assert_eq!(lowering.family_fingerprint(), artifact.family_fingerprint());
}

#[test]
fn scalar_product_head_must_canonicalize_reversed_coordinates() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = SymbolBuilder::new(
        NamespacedSymbol::try_parse("rustred_scalar_numerator_tests::attribute_less_dot").unwrap(),
    )
    .build()
    .unwrap();
    let error = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![symbol("attribute_less_k").to_atom()],
        ScalarNumeratorLimits::default(),
    )
    .err()
    .expect("an attribute-less head cannot canonicalize symmetric coordinates");
    assert!(matches!(
        error,
        ScalarNumeratorError::InvalidScalarProductHead {
            violation: ScalarProductHeadViolation::Attributes,
        }
    ));
}

#[test]
fn pinched_cross_product_lowers_and_cancels_after_exact_reduction() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let head = dot_head();
    let k1 = symbol("pinch_k1").to_atom();
    let k2 = symbol("pinch_k2").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![k1.clone(), k2.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let base = IntegralKey::try_new([1, 1, 0]).unwrap();
    let lowering = service.lower(&dot(head, k1, k2), &base).unwrap();
    assert!(lowering.terms().iter().any(|term| {
        term.integral().powers() == [1, 1, -1]
            && coefficient(term) == "1/2"
            && term.common_mass_squared_power() == 0
    }));
    assert_eq!(
        service
            .lower(&dot(head, symbol("pinch_k1"), symbol("pinch_k2")), &base)
            .unwrap()
            .terms(),
        lowering.terms()
    );

    let context = artifact.coefficient_context();
    let limits = ScalarNumeratorLimits::default().exact_algebra;
    let mut reducer = Reducer::new(&artifact).unwrap();
    let mut collected = BTreeMap::new();
    for term in lowering.terms() {
        let decomposition = reducer
            .reduce_with_common_mass_homogeneity(term.integral())
            .unwrap();
        for (master, reduced) in decomposition.terms() {
            let total_mass_power =
                reduced.common_mass_squared_power() + i128::from(term.common_mass_squared_power());
            assert_eq!(total_mass_power, 1);
            let contribution = context
                .try_mul(term.coefficient(), reduced.unit_mass_coefficient(), limits)
                .unwrap();
            let key = (master.powers().to_vec(), total_mass_power);
            let combined = match collected.remove(&key) {
                Some(existing) => context.try_add(&existing, &contribution, limits).unwrap(),
                None => contribution,
            };
            if !combined.is_zero() {
                collected.insert(key, combined);
            }
        }
    }
    assert!(collected.is_empty());
}

#[test]
fn spectators_remain_in_symbolica_and_zero_stays_zero() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("spectator_k").to_atom();
    let external_left = symbol("p1").to_atom();
    let external_right = symbol("p2").to_atom();
    let scalar = symbol("c").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let loop_square = dot(head, loop_momentum.clone(), loop_momentum);
    let external_dot = dot(head, external_left, external_right);
    let numerator = scalar.clone() * loop_square + external_dot.clone();
    let lowering = service
        .lower(&numerator, &IntegralKey::try_new([2]).unwrap())
        .unwrap();

    assert!(
        lowering
            .terms()
            .iter()
            .any(|term| { term.scalar_spectator() == &scalar && term.integral().powers() == [1] })
    );
    assert!(lowering.terms().iter().any(|term| {
        term.scalar_spectator() == &external_dot && term.integral().powers() == [2]
    }));
    assert!(
        service
            .lower(&Atom::Zero, &IntegralKey::try_new([2]).unwrap())
            .unwrap()
            .is_zero()
    );
}

#[test]
fn mass_increment_combines_with_reducer_homogeneity() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("homogeneous_k").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let lowering = service
        .lower(
            &dot(head, loop_momentum.clone(), loop_momentum),
            &IntegralKey::try_new([3]).unwrap(),
        )
        .unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    let total_mass_powers = lowering
        .terms()
        .iter()
        .flat_map(|term| {
            reducer
                .reduce_with_common_mass_homogeneity(term.integral())
                .unwrap()
                .terms()
                .values()
                .map(|coefficient| {
                    coefficient.common_mass_squared_power()
                        + i128::from(term.common_mass_squared_power())
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(total_mass_powers, vec![-1, -1]);
}

#[test]
fn residual_loop_dependence_and_nonpolynomial_forms_fail_closed() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("rejected_k").to_atom();
    let external = symbol("rejected_p").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits::default(),
    )
    .unwrap();
    let base = IntegralKey::try_new([1]).unwrap();

    assert!(matches!(
        service.lower(&dot(head, loop_momentum.clone(), external), &base),
        Err(ScalarNumeratorError::MixedLoopScalarProduct { .. })
    ));
    assert!(matches!(
        service.lower(&loop_momentum.clone(), &base),
        Err(ScalarNumeratorError::LoopMomentumOutsideScalarProduct { .. })
    ));
    assert!(matches!(
        service.lower(
            &dot(head, loop_momentum.clone(), loop_momentum.clone()).pow(Atom::num(-1)),
            &base,
        ),
        Err(ScalarNumeratorError::NonPolynomialScalarProducts { .. })
    ));
    let unexpanded =
        (dot(head, loop_momentum.clone(), loop_momentum) + Atom::num(1)).pow(Atom::num(2));
    assert!(matches!(
        service.lower(&unexpanded, &base),
        Err(ScalarNumeratorError::NonPolynomialScalarProducts { .. })
    ));
}

#[test]
fn scalar_degree_limit_is_explicit_not_a_rank_specialization() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("limited_k").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits {
            max_scalar_product_degree: 3,
            ..ScalarNumeratorLimits::default()
        },
    )
    .unwrap();
    let numerator = dot(head, loop_momentum.clone(), loop_momentum).pow(Atom::num(4));
    assert!(matches!(
        service.lower(&numerator, &IntegralKey::try_new([5]).unwrap()),
        Err(ScalarNumeratorError::ResourceLimit {
            resource: "scalar-product degree",
            requested: 4,
            limit: 3,
        })
    ));
}

#[test]
fn symbolica_exponent_domain_is_an_unconditional_boundary() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("cas_exponent_k").to_atom();
    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum.clone()],
        ScalarNumeratorLimits {
            max_scalar_product_degree: usize::MAX,
            ..ScalarNumeratorLimits::default()
        },
    )
    .unwrap();
    let numerator =
        dot(head, loop_momentum.clone(), loop_momentum).pow(Atom::num(i64::from(i32::MAX)));
    assert!(matches!(
        service.lower(&numerator, &IntegralKey::try_new([1]).unwrap()),
        Err(ScalarNumeratorError::ScalarProductExponentOverflow)
    ));
}

#[test]
fn momentum_labels_and_exact_comparisons_are_bounded() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let loop_momentum = symbol("bounded_label_k").to_atom();
    assert!(matches!(
        ScalarNumeratorService::try_new(
            &artifact,
            head,
            vec![loop_momentum.clone()],
            ScalarNumeratorLimits {
                max_momentum_label_nodes: 0,
                ..ScalarNumeratorLimits::default()
            },
        ),
        Err(ScalarNumeratorError::ResourceLimit {
            resource: "scalar-numerator momentum label nodes",
            requested: 1,
            limit: 0,
        })
    ));

    let service = ScalarNumeratorService::try_new(
        &artifact,
        head,
        vec![loop_momentum],
        ScalarNumeratorLimits {
            max_loop_momentum_label_checks: 0,
            ..ScalarNumeratorLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        service.lower(&Atom::num(1), &IntegralKey::try_new([1]).unwrap()),
        Err(ScalarNumeratorError::ResourceLimit {
            resource: "loop-momentum exact subtree checks",
            requested: 1,
            limit: 0,
        })
    ));
}
