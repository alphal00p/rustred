use super::weighted::weights;
use super::*;
use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use symbolica::{
    atom::AtomView,
    id::{Pattern, Replacement},
    parser::ParseSettings,
};

pub(super) fn family(loops: usize) -> (IntegralFamily, Vec<Vec<i64>>) {
    let c = CoefficientContext::try_new(["d"]).unwrap();
    let coordinates = (0..loops)
        .flat_map(|i| (i..loops).map(move |j| (i, j)))
        .collect::<Vec<_>>();
    let mut momenta = (0..loops)
        .map(|i| {
            let mut v = vec![0; loops];
            v[i] = 1;
            v
        })
        .collect::<Vec<_>>();
    for i in 0..loops {
        for j in i + 1..loops {
            let mut v = vec![0; loops];
            v[i] = 1;
            v[j] = -1;
            momenta.push(v);
        }
    }
    momenta.reverse(); // The inverse change of basis is not idempotent.
    let denominators = momenta
        .iter()
        .map(|v| {
            AffineDenominator::new(
                c.integer(-1),
                coordinates
                    .iter()
                    .map(|&(i, j)| c.integer(v[i] * v[j] * if i == j { 1 } else { 2 }))
                    .collect(),
            )
        })
        .collect();
    let result = IntegralFamily::new(
        "whole_weighted",
        (0..loops).map(|i| format!("q{i}")).collect(),
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![],
        vec![c.zero(); coordinates.len()],
    )
    .unwrap();
    (result, momenta)
}

#[test]
fn whole_denominator_powers_cancel_in_two_and_three_loops_before_output_cap() {
    for loops in [2, 3] {
        let (family, momenta) = family(loops);
        let head = dot_head();
        let labels = (0..loops)
            .map(|i| symbol(&format!("whole_{loops}_{i}")).to_atom())
            .collect::<Vec<_>>();
        let service = FamilyScalarNumeratorService::try_new(
            &family,
            head,
            labels.clone(),
            ScalarNumeratorLimits {
                diagnostics_enabled: true,
                max_lowered_terms: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let base = IntegralKey::try_new(vec![2; momenta.len()]).unwrap();
        for mass in [
            Atom::one(),
            Atom::num(4),
            symbol("whole_mass_squared").to_atom(),
        ] {
            for (slot, v) in momenta.iter().enumerate() {
                let mut square = Atom::Zero;
                for (i, left) in labels.iter().enumerate() {
                    for (j, right) in labels.iter().enumerate() {
                        square += Atom::num(v[i] * v[j]) * dot(head, left.clone(), right.clone());
                    }
                }
                let factored = (square - &mass).pow(11);
                let expected_key = base
                    .powers()
                    .iter()
                    .enumerate()
                    .map(|(i, &power)| power - if i == slot { 11 } else { 0 })
                    .collect::<Vec<_>>();
                let first = service.lower_weighted(&factored, &base, &mass).unwrap();
                let expanded = service
                    .lower_weighted(&factored.expand(), &base, &mass)
                    .unwrap();
                assert_eq!(
                    weights(&first),
                    BTreeMap::from([(expected_key, Atom::one())])
                );
                assert_eq!(weights(&first), weights(&expanded));
                assert_eq!(first.nonzero_conditions(), expanded.nonzero_conditions());
            }
        }
        let two = (dot(head, labels[0].clone(), labels[0].clone())
            + dot(head, labels[1].clone(), labels[1].clone()))
        .pow(11);
        assert!(matches!(
            service.lower_weighted(&two, &base, &Atom::one()),
            Err(ScalarNumeratorError::ResourceLimit { .. })
        ));
    }
}

#[test]
fn denominator_coordinate_substitution_is_simultaneous_and_literal() {
    let x = symbol("literal_swap_x_").to_atom();
    let y = symbol("literal_swap_y").to_atom();
    let source = symbol("literal_swap_f").call((&x, &y));
    let mapped = source.replace_multiple([
        Replacement::new(
            Pattern::Literal(x.clone()),
            Pattern::Literal(&y + Atom::one()),
        ),
        Replacement::new(
            Pattern::Literal(y.clone()),
            Pattern::Literal(Atom::num(2) * &x),
        ),
    ]);
    assert_eq!(
        mapped,
        symbol("literal_swap_f").call((&y + Atom::one(), Atom::num(2) * &x))
    );

    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let external = symbol("literal_external").to_atom();
    let spectator = dot(head, external.clone(), external);
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![x.clone()],
        Default::default(),
    )
    .unwrap();
    let input = (dot(head, x.clone(), x) - Atom::one()).pow(3) * &spectator;
    let output = service
        .lower_weighted(&input, &IntegralKey::try_new([2]).unwrap(), &Atom::one())
        .unwrap();
    assert_eq!(weights(&output), BTreeMap::from([(vec![-1], spectator)]));
}

#[test]
fn numeric_affine_cancellation_precedes_power_support_admission() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let head = dot_head();
    let k = symbol("numeric_collection_loop").to_atom();
    let square = dot(head, k.clone(), k.clone());
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![k],
        ScalarNumeratorLimits {
            diagnostics_enabled: true,
            max_polynomial_terms: 1,
            max_lowered_terms: 1,
            ..Default::default()
        },
    )
    .unwrap();
    for mass in [Atom::one(), Atom::num(4), symbol("numeric_mass").to_atom()] {
        let base = Atom::num(3) * &square - Atom::num(2) * (&square + &mass) + &mass;
        let input = base.pow(11);
        assert_eq!(input.expand_num(), (&square - &mass).pow(11));
        assert!(matches!(input.expand_num().as_view(), AtomView::Pow(_)));
        let result = service
            .lower_weighted(&input, &IntegralKey::try_new([2]).unwrap(), &mass)
            .unwrap();
        assert_eq!(weights(&result), BTreeMap::from([(vec![-9], Atom::one())]));
        assert!(matches!(
            service.lower_weighted(
                &(&square + &mass).pow(2),
                &IntegralKey::try_new([2]).unwrap(),
                &mass,
            ),
            Err(ScalarNumeratorError::ResourceLimit { .. })
        ));
    }
}

#[test]
fn actual_cancelled_rational_prefactor_keeps_all_original_poles() {
    let input = Atom::parse(
        include_str!("cancelled_prefactor.symbolica"),
        "weighted_regression",
        ParseSettings::default(),
    )
    .unwrap();
    assert!(!input.is_zero());
    assert!(input.collect_factors().is_zero());
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let c = artifact.coefficient_context();
    let d = c.parameter("d").unwrap().to_expression();
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        dot_head(),
        vec![symbol("prefactor_loop").to_atom()],
        Default::default(),
    )
    .unwrap();
    let output = service
        .lower_weighted(&input, &IntegralKey::try_new([1]).unwrap(), &Atom::one())
        .unwrap();
    assert!(output.is_zero());
    for condition in [d.clone(), &d - Atom::one(), &d + Atom::num(2)] {
        assert!(output.nonzero_conditions().contains(&condition));
    }
}

#[test]
fn collection_does_not_admit_a_genuinely_large_polynomial_or_erase_cancelled_poles() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let head = dot_head();
    let a = symbol("whole_bound_a").to_atom();
    let b = symbol("whole_bound_b").to_atom();
    let service = FamilyScalarNumeratorService::try_new(
        artifact.family(),
        head,
        vec![a.clone(), b.clone()],
        ScalarNumeratorLimits {
            diagnostics_enabled: true,
            max_polynomial_terms: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let source = (dot(head, a.clone(), a) + dot(head, b.clone(), b)).pow(11);
    assert!(matches!(
        service.lower_weighted(
            &source,
            &IntegralKey::try_new([1, 1, 1]).unwrap(),
            &Atom::one()
        ),
        Err(ScalarNumeratorError::ResourceLimit { .. })
    ));
}
