//! Independent adversarial tests for exact terminal aliases. These fixtures
//! exercise changes of integration variables, not oracle-derived relations.

use std::collections::BTreeSet;

use symbolica::domains::SelfRing;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::{DenominatorAction, Jacobian};

use super::{ProductSkipReason, TerminalAliasError, TerminalAliasPlan};

fn key<const N: usize>(powers: [i64; N]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn family(name: &str, momenta: &[&[i64]], masses: &[i64], shifts: &[i64]) -> IntegralFamily {
    let loops = momenta[0].len();
    let context = CoefficientContext::new(["d"]);
    let denominators = momenta
        .iter()
        .zip(masses)
        .map(|(momentum, mass)| {
            let row = (0..loops)
                .flat_map(|left| {
                    (left..loops).map(move |right| {
                        momentum[left] * momentum[right] * if left == right { 1 } else { 2 }
                    })
                })
                .map(|value| context.integer(value))
                .collect();
            AffineDenominator::new(context.integer(-mass), row)
        })
        .collect();
    IntegralFamily::new(
        name,
        (0..loops).map(|axis| format!("k{axis}")).collect(),
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        vec![],
        shifts.iter().map(|&shift| context.integer(shift)).collect(),
    )
    .unwrap()
}

fn prepare(family: &IntegralFamily, raw: &BTreeSet<IntegralKey>) -> TerminalAliasPlan {
    TerminalAliasPlan::independent_tadpole_products(family, raw, OrderingPolicy::SpiredUncutV1)
        .unwrap()
}

#[test]
fn primitive_integer_shears_alias_but_nonunit_jacobians_and_numerators_do_not() {
    let family = family(
        "audit-primitive-shear",
        &[&[1, 0], &[0, 1], &[2, 1]],
        &[1; 3],
        &[0; 3],
    );
    let raw = BTreeSet::from([
        key([1, 1, 0]),
        key([1, 0, 1]),
        key([0, 1, 1]),
        key([1, 1, -1]),
        key([2, 1, 0]),
        key([1, 0, 2]),
        key([1, 2, 0]),
    ]);
    let plan = prepare(&family, &raw);
    assert_eq!(plan.raw_terminals(), &raw);
    assert_eq!(plan.statistics().eligible_products, 5);
    assert_eq!(plan.aliases().len(), 3);
    assert_eq!(plan.canonical_terminals().len(), 4);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NonUnimodularMomentumBasis],
        1
    );
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NumeratorPowers],
        1
    );
    assert_eq!(
        plan.representative(&family, &key([0, 1, 1])).unwrap(),
        &key([0, 1, 1])
    );
    assert_eq!(
        plan.representative(&family, &key([1, 1, -1])).unwrap(),
        &key([1, 1, -1])
    );
    assert_eq!(
        plan.representative(&family, &key([1, 1, 0])).unwrap(),
        plan.representative(&family, &key([1, 0, 1])).unwrap()
    );
    for source in [key([2, 1, 0]), key([1, 0, 2]), key([1, 2, 0])] {
        assert_eq!(
            plan.representative(&family, &source).unwrap(),
            plan.representative(&family, &key([2, 1, 0])).unwrap()
        );
    }
    for (source, alias) in plan.aliases() {
        assert!(matches!(alias.witness().jacobian(), Jacobian::Unit { .. }));
        assert_eq!(
            OrderingPolicy::SpiredUncutV1
                .compare(alias.representative().powers(), source.powers())
                .unwrap(),
            std::cmp::Ordering::Less
        );
        assert!(!plan.aliases().contains_key(alias.representative()));
        assert!(raw.contains(alias.representative()));
        for (slot, &power) in source
            .powers()
            .iter()
            .enumerate()
            .filter(|(_, power)| **power > 0)
        {
            let DenominatorAction::Monomial { target, scale } =
                &alias.witness().row_actions()[slot]
            else {
                panic!("active denominator not monomial");
            };
            assert!(scale.is_one());
            assert_eq!(power, alias.representative().powers()[*target]);
        }
    }
}

#[test]
fn epsilon_fg_products_match_without_an_isp_permutation_requirement() {
    let family = family(
        "audit-fg-products",
        &[
            &[1, 0, 0, 0],
            &[0, 1, 0, 0],
            &[0, 0, 1, 0],
            &[1, 0, -1, 0],
            &[0, 0, 0, 1],
            &[0, 1, -1, 0],
            &[1, 0, -1, 1],
            &[1, -1, 0, 0],
            &[0, 1, 0, -1],
            &[0, 0, 1, -1],
        ],
        &[1; 10],
        &[0; 10],
    );
    let first = key([0, 0, 1, 0, 0, 1, 1, 1, 0, 0]);
    let second = key([0, 0, 1, 0, 1, 0, 1, 1, 0, 0]);
    let plan = prepare(&family, &BTreeSet::from([first.clone(), second.clone()]));
    assert_eq!(plan.aliases().len(), 1);
    assert_eq!(plan.canonical_terminals().len(), 1);
    assert_eq!(
        plan.representative(&family, &first).unwrap(),
        plan.representative(&family, &second).unwrap()
    );
    let witness = plan.aliases().values().next().unwrap().witness();
    assert!(matches!(witness.jacobian(), Jacobian::Unit { .. }));
    assert!(
        witness
            .row_actions()
            .iter()
            .any(|row| matches!(row, DenominatorAction::Affine))
    );
}

#[test]
fn unequal_dotted_power_multisets_are_not_identified() {
    let family = family("audit-dots", &[&[1, 0], &[0, 1], &[1, 1]], &[1; 3], &[0; 3]);
    let first = key([2, 1, 0]);
    let second = key([3, 1, 0]);
    let raw = BTreeSet::from([first.clone(), second.clone()]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn unit_mass_and_zero_analytic_offsets_are_required() {
    let raw = BTreeSet::from([key([1, 1, 0]), key([1, 0, 1])]);
    let massive = family(
        "audit-masses",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1, 1, 2],
        &[0; 3],
    );
    let mass_plan = prepare(&massive, &raw);
    assert!(mass_plan.aliases().is_empty());
    assert_eq!(mass_plan.canonical_terminals(), &raw);
    assert_eq!(
        mass_plan.statistics().skipped[&ProductSkipReason::NonUnitMass],
        1
    );
    let shifted = family(
        "audit-shift",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1; 3],
        &[0, 0, 1],
    );
    let shift_plan = prepare(&shifted, &raw);
    assert!(shift_plan.aliases().is_empty());
    assert_eq!(shift_plan.canonical_terminals(), &raw);
    assert_eq!(
        shift_plan.statistics().skipped[&ProductSkipReason::AnalyticPowerShifts],
        raw.len()
    );
}

#[test]
fn singular_product_preserves_declared_key_without_fabricating_a_terminal() {
    let family = family(
        "audit-singular-product",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[0, 0, 1],
            &[1, 1, 0],
            &[1, 0, 1],
            &[0, 1, 1],
        ],
        &[1; 6],
        &[0; 6],
    );
    let raw = BTreeSet::from([key([1, 1, 1, 0, 0, 0]), key([1, 1, 0, 1, 0, 0])]);
    let plan = prepare(&family, &raw);
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::SingularMomentumBasis],
        1
    );
}

#[test]
fn plans_are_bound_to_family_arity_and_declared_keys() {
    let first = family(
        "audit-owner",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1; 3],
        &[0; 3],
    );
    let other = family(
        "audit-other-owner",
        &[&[1, 0], &[0, 1], &[1, 1]],
        &[1; 3],
        &[0; 3],
    );
    let raw = BTreeSet::from([key([1, 1, 0]), key([1, 0, 1])]);
    let plan = prepare(&first, &raw);
    assert_eq!(
        plan.representative(&other, &key([1, 1, 0])).unwrap_err(),
        TerminalAliasError::WrongFamily
    );
    assert_eq!(
        plan.representative(&first, &key([1, 1])).unwrap_err(),
        TerminalAliasError::WrongArity {
            expected: 3,
            actual: 2
        }
    );
    assert_eq!(
        plan.representative(&first, &key([4, 1, 0])).unwrap_err(),
        TerminalAliasError::UndeclaredTerminal
    );
    assert!(matches!(
        TerminalAliasPlan::independent_tadpole_products(
            &first,
            &BTreeSet::from([key([1, 1])]),
            OrderingPolicy::SpiredUncutV1
        ),
        Err(TerminalAliasError::WrongArity { .. })
    ));
    let empty = prepare(&first, &BTreeSet::new());
    assert!(empty.canonical_terminals().is_empty());
    assert!(empty.aliases().is_empty());
}

#[test]
fn parameter_dependent_auxiliary_maps_are_conservatively_left_unchanged() {
    let base = family(
        "audit-conditional-base",
        &[
            &[1, 0, 0],
            &[0, 1, 0],
            &[0, 0, 1],
            &[1, 1, 0],
            &[1, 0, 1],
            &[0, 1, 1],
        ],
        &[1; 6],
        &[0; 6],
    );
    let context = base.coefficient_context();
    let scale = context.parameter("d").unwrap();
    let mut denominators = base.denominators().to_vec();
    let auxiliary = &denominators[5];
    denominators[5] = AffineDenominator::new(
        context
            .try_mul(auxiliary.constant(), &scale, Default::default())
            .unwrap(),
        auxiliary
            .coefficients()
            .iter()
            .map(|value| context.try_mul(value, &scale, Default::default()).unwrap())
            .collect(),
    );
    let family = IntegralFamily::new(
        "audit-conditional-auxiliary",
        base.loop_momenta().to_vec(),
        vec![],
        context.clone(),
        base.dimension().clone(),
        denominators,
        vec![],
        base.power_shifts().to_vec(),
    )
    .unwrap();
    // Swapping k1 and k2 identifies these active products. However, the full
    // auxiliary-coordinate map also contains scales d and 1/d. This first
    // service deliberately retains such guarded proposals rather than
    // silently erasing their conditions, even if family admission implies d≠0.
    assert!(
        family
            .domain()
            .conditions()
            .any(|condition| !condition.polynomial().is_constant())
    );
    let raw = BTreeSet::from([key([1, 0, 1, 1, 0, 0]), key([0, 1, 1, 1, 0, 0])]);
    let plan = prepare(&family, &raw);
    assert_eq!(plan.statistics().eligible_products, 2);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::ConditionalMomentumMap],
        1
    );
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}

#[test]
fn rational_loop_basis_is_not_admitted_merely_because_its_determinant_is_one() {
    let base = family(
        "audit-rational-base",
        &[&[1, 0], &[0, 2], &[1, 1]],
        &[1; 3],
        &[0; 3],
    );
    let context = base.coefficient_context();
    let mut denominators = base.denominators().to_vec();
    let quarter = context
        .try_div(&context.one(), &context.integer(4), Default::default())
        .unwrap();
    denominators[0] = AffineDenominator::new(
        context.integer(-1),
        vec![quarter, context.zero(), context.zero()],
    );
    let family = IntegralFamily::new(
        "audit-rational-unit-determinant",
        base.loop_momenta().to_vec(),
        vec![],
        context.clone(),
        base.dimension().clone(),
        denominators,
        vec![],
        base.power_shifts().to_vec(),
    )
    .unwrap();
    // For the first key U=diag(1/2,2), hence det(U)=1, but U is not an
    // integer unimodular transformation. The initial lane must not admit it.
    let raw = BTreeSet::from([key([1, 1, 0]), key([0, 1, 1])]);
    let plan = prepare(&family, &raw);
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NonIntegerQuadratic],
        1
    );
    assert_eq!(
        plan.statistics().skipped[&ProductSkipReason::NonUnimodularMomentumBasis],
        1
    );
    assert!(plan.aliases().is_empty());
    assert_eq!(plan.canonical_terminals(), &raw);
}
