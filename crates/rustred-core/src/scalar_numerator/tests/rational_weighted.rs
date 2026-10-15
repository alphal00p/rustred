//! Exact differential controls for the optional rational coefficient lane.
//! Both sides share input validation, guards, denominator translation and final
//! term admission. Only native polynomial coefficient arithmetic differs.
use super::*;
use crate::scalar_numerator::weighted::polynomial::testing::observed;

fn compare(
    service: &FamilyScalarNumeratorService<'_>,
    numerator: &Atom,
    base: &IntegralKey,
    mass: &Atom,
    rational: bool,
) -> WeightedScalarNumeratorLowering {
    let (fast, rational_calls, generic_calls) =
        observed(false, || service.lower_weighted(numerator, base, mass));
    let (ordinary, forced_rational, forced_generic) =
        observed(true, || service.lower_weighted(numerator, base, mass));
    let fast = fast.unwrap();
    let ordinary = ordinary.unwrap();
    assert_eq!(
        (rational_calls, generic_calls),
        (usize::from(rational), usize::from(!rational))
    );
    assert_eq!((forced_rational, forced_generic), (0, 1));
    assert_eq!(fast.family_fingerprint(), ordinary.family_fingerprint());
    assert_eq!(fast.terms(), ordinary.terms());
    assert_eq!(fast.nonzero_conditions(), ordinary.nonzero_conditions());
    fast
}

#[test]
fn rational_weighted_mixed_denominator_powers_match_generic_in_two_three_and_five_loops() {
    for loops in [2, 3, 5] {
        let (family, momenta) = super::whole_weighted::family(loops);
        let labels = (0..loops)
            .map(|index| symbol(&format!("rational_mixed_{loops}_{index}")).to_atom())
            .collect::<Vec<_>>();
        let head = dot_head();
        let service = FamilyScalarNumeratorService::try_new(
            &family,
            head,
            labels.clone(),
            ScalarNumeratorLimits {
                max_lowered_terms: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let first = 1;
        let second = 6.min(momenta.len() - 1);
        let mut powers = vec![1; momenta.len()];
        powers[first] += 2;
        powers[second] += 3;
        let base = IntegralKey::try_new(powers).unwrap();
        for mass in [Atom::one(), Atom::num(3) / Atom::num(2)] {
            let denominator = |slot: usize| {
                let mut square = Atom::Zero;
                for (i, left) in labels.iter().enumerate() {
                    for (j, right) in labels.iter().enumerate() {
                        square += Atom::num(momenta[slot][i] * momenta[slot][j])
                            * dot(head, left.clone(), right.clone());
                    }
                }
                square - &mass
            };
            // At five loops this is P2² P7³, scalar degree5 / momentum degree10.
            let factored = denominator(first).pow(2) * denominator(second).pow(3);
            for numerator in [factored.clone(), factored.expand()] {
                let result = compare(&service, &numerator, &base, &mass, true);
                assert_eq!(result.terms().len(), 1);
                assert_eq!(
                    result.terms()[0].integral().powers(),
                    vec![1; momenta.len()]
                );
                assert_eq!(
                    result.terms()[0].coefficient(),
                    &family.coefficient_context().one()
                );
                assert_eq!(result.terms()[0].scalar_spectator(), &Atom::one());
                assert!(result.nonzero_conditions().is_empty());
            }
        }
    }
}

#[test]
fn rational_weighted_opaque_complex_and_parameter_coefficients_keep_generic_lane() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let k = symbol("rational_fallback_k").to_atom();
    let head = dot_head();
    let square = dot(head, k.clone(), k.clone());
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let d = artifact
        .coefficient_context()
        .parameter("d")
        .unwrap()
        .to_expression();
    let external = symbol("rational_opaque_x").to_atom();
    let spectators = [
        external.clone(),
        symbol("rational_opaque_f").call(external.clone()),
        d.clone() + Atom::one(),
        Atom::one() / (&d - Atom::one()),
        Atom::i(),
        Atom::num(2).sqrt(),
    ];
    let base = IntegralKey::try_new([4]).unwrap();
    for spectator in spectators {
        let numerator = (spectator * (&square - Atom::one()).pow(3)).expand();
        let result = compare(&service, &numerator, &base, &Atom::one(), false);
        assert_eq!(result.terms().len(), 1);
        assert_eq!(result.terms()[0].integral().powers(), [1]);
    }
    let mass = symbol("rational_symbolic_mass").to_atom();
    let result = compare(&service, &square, &base, &mass, false);
    assert_eq!(result.terms().len(), 2);
    assert!(result.nonzero_conditions().contains(&mass));
}

#[test]
fn rational_weighted_zero_cancellation_and_original_poles_are_identical() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let k = symbol("rational_zero_k").to_atom();
    let head = dot_head();
    let square = dot(head, k.clone(), k.clone());
    let service =
        FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k], Default::default())
            .unwrap();
    let base = IntegralKey::try_new([2]).unwrap();
    for numerator in [
        Atom::Zero,
        (&square + Atom::one()).pow(2)
            - square.clone().pow(2)
            - Atom::num(2) * &square
            - Atom::one(),
    ] {
        let result = compare(&service, &numerator, &base, &Atom::one(), true);
        assert!(result.is_zero());
        assert!(result.nonzero_conditions().is_empty());
    }
    let d = artifact
        .coefficient_context()
        .parameter("d")
        .unwrap()
        .to_expression();
    let pole = &d - Atom::one();
    let numerator = (d.clone().pow(2) - Atom::one()) / &pole - &d - Atom::one();
    let result = compare(&service, &numerator, &base, &Atom::one(), false);
    assert!(result.is_zero());
    assert!(result.nonzero_conditions().contains(&pole));
}

#[test]
fn rational_weighted_resource_and_input_errors_match_generic() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let k = symbol("rational_errors_k").to_atom();
    let head = dot_head();
    let square = dot(head, k.clone(), k.clone());
    let base = IntegralKey::try_new([2]).unwrap();
    let mut cases = vec![
        // Inexact coefficients are rejected by the existing initial guard
        // scan, before either polynomial lane; they are not a new fallback.
        (
            Atom::num(0.5) * &square,
            base.clone(),
            Atom::one(),
            ScalarNumeratorLimits::default(),
        ),
        (
            square.clone(),
            base.clone(),
            Atom::Zero,
            ScalarNumeratorLimits::default(),
        ),
        (
            square.clone(),
            base.clone(),
            square.clone(),
            ScalarNumeratorLimits::default(),
        ),
        (
            square.clone().pow(-1),
            base.clone(),
            Atom::one(),
            ScalarNumeratorLimits::default(),
        ),
        (
            square.clone(),
            IntegralKey::try_new([1, 1]).unwrap(),
            Atom::one(),
            ScalarNumeratorLimits::default(),
        ),
        (
            (&square + Atom::one()).pow(3),
            base.clone(),
            Atom::one(),
            ScalarNumeratorLimits {
                max_polynomial_terms: 1,
                ..Default::default()
            },
        ),
        (
            square.clone(),
            base.clone(),
            Atom::one(),
            ScalarNumeratorLimits {
                max_lowered_terms: 1,
                ..Default::default()
            },
        ),
    ];
    for work in 0..32 {
        let mut limits = ScalarNumeratorLimits::default();
        limits.exact_algebra.max_term_operations = work;
        cases.push((square.clone(), base.clone(), Atom::one(), limits));
    }
    let mut failures = 0;
    for (numerator, base, mass, limits) in cases {
        let service =
            FamilyScalarNumeratorService::try_new(artifact.family(), head, vec![k.clone()], limits)
                .unwrap();
        let (actual, _, _) = observed(false, || service.lower_weighted(&numerator, &base, &mass));
        let (reference, _, _) = observed(true, || service.lower_weighted(&numerator, &base, &mass));
        match (actual, reference) {
            (Ok(actual), Ok(reference)) => {
                assert_eq!(actual.terms(), reference.terms());
                assert_eq!(actual.nonzero_conditions(), reference.nonzero_conditions());
            }
            (Err(actual), Err(reference)) => {
                failures += 1;
                assert_eq!(
                    std::mem::discriminant(&actual),
                    std::mem::discriminant(&reference)
                );
                assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
            }
            other => panic!("weighted policy changed: {other:?}"),
        }
    }
    assert!(failures >= 6);
}
