//! Full-kernel conversion of a raised finite-density cut to simple-cut jets.
//!
//! A jet `(k, r, b)` denotes
//! `int_p E^k (partial_E^r H)(E) W_b(mu-E)/(2E)`.
//! `H` contains the complete on-shell vacuum kernel and numerator, including
//! their Euclidean energy phases. Each cut line has its own auxiliary mass:
//! for that line, its mass enters H only through E=sqrt(p^2+m^2). Any other
//! explicit dependence on that mass would require additional derivatives.
//!
//! The simple-pole medium contribution has a minus sign. Raising its original
//! Euclidean propagator to power a applies (-partial_m2)^(a-1)/(a-1)! to the
//! ENTIRE simple-pole expression. This module expands that derivative without
//! evaluating H. It does not enumerate cut topologies, specify the vacuum
//! kernel's continuation prescription, or prove a supplied contour reduction.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::solver::SolverError;

/// (Extra energy power, derivative order of the full kernel, W index).
pub(super) type CutJet = (i64, u32, u32);
pub(super) type CutExpansion = BTreeMap<CutJet, Coefficient>;

/// Express one raised Euclidean medium cut using only a simple shell cut.
///
/// The result includes the medium minus sign and raised-power factorial.
/// The coefficient context is preserved, so expansions of different cut legs
/// can be combined by a tensor product with mixed kernel derivatives.
pub(super) fn raised_cut_expansion(
    context: &CoefficientContext,
    power: u32,
) -> Result<CutExpansion, SolverError> {
    if power == 0 {
        return Err(SolverError::InvalidInput(
            "a medium cut requires a positive original propagator power".into(),
        ));
    }
    let two = context.integer(2);
    let mut terms = BTreeMap::from([((0, 0, 0), context.one())]);
    for _ in 1..power {
        let mut next = BTreeMap::new();
        for ((energy, derivative, b), coefficient) in terms {
            // E' = 1/(2E). The three terms differentiate, respectively,
            // E^k/(2E), H, and the occupation/surface distribution.
            let children = [
                ((energy - 2, derivative, b), energy - 1),
                ((energy - 1, derivative + 1, b), 1),
                (
                    (energy - 1, derivative, b + 1),
                    if b == 0 { -1 } else { i64::from(b) },
                ),
            ];
            for (jet, numerator) in children {
                let contribution = &(&coefficient * &context.integer(numerator)) / &two;
                let previous = next.entry(jet).or_insert_with(|| context.zero());
                *previous = &*previous + &contribution;
            }
        }
        next.retain(|_, coefficient| !coefficient.is_zero());
        terms = next;
    }
    let mut normalization = context.integer(-1);
    for factorial in 1..power {
        normalization = -(&normalization / &context.integer(i64::from(factorial)));
    }
    for coefficient in terms.values_mut() {
        *coefficient = &*coefficient * &normalization;
    }
    Ok(terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent evaluation: integrate the raised mass-shell distribution
    /// over the spatial radius first, followed by its energy moment. Units
    /// are mu=1, and the common angular normalization K_d/2 is removed.
    /// g=E^2-p^2-m^2 and C_a(g)=(-1)^(a-1) delta^(a-1)(g)/(a-1)!.
    fn spatial_first_moment(
        context: &CoefficientContext,
        cut_power: u32,
        energy: &Coefficient,
        b: u32,
    ) -> Coefficient {
        let d = context.parameter("d").unwrap();
        let mut shell = context.one();
        for r in 1..cut_power {
            shell = -(&shell * &(&(&d / &context.integer(2)) - &context.integer(i64::from(r))));
            shell = &shell / &context.integer(i64::from(r));
        }
        let exponent = &(&d + energy) - &context.integer(2 * i64::from(cut_power));
        let moment = if b == 0 {
            &context.one() / &(&exponent + &context.one())
        } else {
            // W_b(1-E)=(-1)^(b-1) delta^(b-1)(1-E)/(b-1)!.
            let mut surface = context.one();
            for r in 1..b {
                surface = -(&surface * &(&exponent - &context.integer(i64::from(r - 1))));
                surface = &surface / &context.integer(i64::from(r));
            }
            surface
        };
        &shell * &moment
    }

    fn falling_factorial(
        context: &CoefficientContext,
        exponent: &Coefficient,
        order: u32,
    ) -> Coefficient {
        (0..order).fold(context.one(), |product, r| {
            &product * &(exponent - &context.integer(i64::from(r)))
        })
    }

    /// Apply a jet to H(E)=E^h, with h left symbolic. This uses the closed
    /// monomial derivative and independently integrated distribution moments.
    fn evaluate_jet(
        context: &CoefficientContext,
        (shift, derivative, b): CutJet,
        exponent: &Coefficient,
    ) -> Coefficient {
        let energy = exponent + &context.integer(shift - i64::from(derivative));
        &falling_factorial(context, exponent, derivative)
            * &spatial_first_moment(context, 1, &energy, b)
    }

    fn evaluate_expansion(
        context: &CoefficientContext,
        expansion: &CutExpansion,
        exponent: &Coefficient,
    ) -> Coefficient {
        expansion
            .iter()
            .fold(context.zero(), |sum, (&jet, weight)| {
                &sum + &(weight * &evaluate_jet(context, jet, exponent))
            })
    }

    fn raised_euclidean_moment(
        context: &CoefficientContext,
        cut_power: u32,
        energy: &Coefficient,
    ) -> Coefficient {
        let moment = spatial_first_moment(context, cut_power, energy, 0);
        if cut_power % 2 == 0 { moment } else { -moment }
    }

    #[test]
    fn complete_kernel_jets_match_independent_raised_cut_moments() {
        let context = CoefficientContext::new(["d", "h"]);
        let h = context.parameter("h").unwrap();
        assert!(raised_cut_expansion(&context, 0).is_err());
        for power in 1..=3 {
            let expansion = raised_cut_expansion(&context, power).unwrap();
            assert!(expansion.keys().all(|(_, r, b)| r + b < power));
            assert_eq!(
                evaluate_expansion(&context, &expansion, &h),
                raised_euclidean_moment(&context, power, &h),
                "cut power {power}; H(E)=E^h with symbolic h",
            );
        }
    }

    #[test]
    fn omitting_kernel_derivatives_or_surface_terms_fails() {
        let context = CoefficientContext::new(["d", "h"]);
        let h = context.parameter("h").unwrap();
        let expansion = raised_cut_expansion(&context, 2).unwrap();
        let exact = raised_euclidean_moment(&context, 2, &h);
        let without_kernel_derivatives = expansion
            .iter()
            .filter(|((_, derivative, _), _)| *derivative == 0)
            .map(|(jet, coefficient)| (*jet, coefficient.clone()))
            .collect();
        let without_surfaces = expansion
            .iter()
            .filter(|((_, _, b), _)| *b == 0)
            .map(|(jet, coefficient)| (*jet, coefficient.clone()))
            .collect();
        assert_ne!(
            evaluate_expansion(&context, &without_kernel_derivatives, &h),
            exact,
        );
        assert_ne!(evaluate_expansion(&context, &without_surfaces, &h), exact,);
        assert!(expansion.keys().any(|(_, derivative, _)| *derivative > 0));
        assert!(expansion.keys().any(|(_, _, b)| *b > 0));
    }

    #[test]
    fn two_cut_legs_retain_mixed_kernel_derivatives_and_surfaces() {
        let context = CoefficientContext::new(["d", "h", "k"]);
        let h = context.parameter("h").unwrap();
        let k = context.parameter("k").unwrap();
        for first_power in 1..=3 {
            for second_power in 1..=3 {
                let first = raised_cut_expansion(&context, first_power).unwrap();
                let second = raised_cut_expansion(&context, second_power).unwrap();
                let mut evaluated = context.zero();
                let mut mixed_derivative = false;
                let mut double_surface = false;
                for (&first_jet, first_weight) in &first {
                    for (&second_jet, second_weight) in &second {
                        // The tensor product acts on H(E1,E2)=E1^h E2^k.
                        // Its key represents partial_E1^r1 partial_E2^r2 H.
                        mixed_derivative |= first_jet.1 > 0 && second_jet.1 > 0;
                        double_surface |= first_jet.2 > 0 && second_jet.2 > 0;
                        let coefficient = first_weight * second_weight;
                        let moment = &evaluate_jet(&context, first_jet, &h)
                            * &evaluate_jet(&context, second_jet, &k);
                        evaluated = &evaluated + &(&coefficient * &moment);
                    }
                }
                let expected = &raised_euclidean_moment(&context, first_power, &h)
                    * &raised_euclidean_moment(&context, second_power, &k);
                assert_eq!(
                    evaluated, expected,
                    "two cut powers ({first_power},{second_power}); symbolic kernel exponents",
                );
                if first_power > 1 && second_power > 1 {
                    assert!(mixed_derivative);
                    assert!(double_surface);
                }
            }
        }
    }

    #[test]
    fn independent_energy_ibps_include_kernel_derivatives_in_bulk_and_surface_sectors() {
        let context = CoefficientContext::new(["d", "h"]);
        let h = context.parameter("h").unwrap();
        // Integrate partial_E [E^k H(E) C_a(g) W_b(1-E)] with g=E²-p².
        // C_a'=-a C_(a+1), while partial_E W_b=t_b W_(b+1).
        // These are weighted distributional IBPs, derived without the mass
        // derivative recurrence used by raised_cut_expansion.
        for cut_power in 1..=3 {
            for b in 0..=2 {
                for energy in [-1, 0, 2] {
                    let full_exponent = &h + &context.integer(energy);
                    let differentiated_energy = &full_exponent - &context.one();
                    let measure = &context.integer(energy)
                        * &spatial_first_moment(&context, cut_power, &differentiated_energy, b);
                    let kernel =
                        &h * &spatial_first_moment(&context, cut_power, &differentiated_energy, b);
                    let shell = &context.integer(-2 * i64::from(cut_power))
                        * &spatial_first_moment(
                            &context,
                            cut_power + 1,
                            &(&full_exponent + &context.one()),
                            b,
                        );
                    let surface = &context.integer(if b == 0 { -1 } else { i64::from(b) })
                        * &spatial_first_moment(&context, cut_power, &full_exponent, b + 1);
                    let without_kernel = &(&measure + &shell) + &surface;
                    assert_eq!(
                        &without_kernel + &kernel,
                        context.zero(),
                        "weighted energy IBP: a={cut_power}, k={energy}, b={b}",
                    );
                    assert!(!without_kernel.is_zero());
                }
            }
        }
    }

    #[test]
    fn coupled_two_energy_kernel_requires_mixed_derivatives() {
        let context = CoefficientContext::new(["d"]);
        let degree = 4_u32;
        let binomial = |n: u32, k: u32| {
            (0..k).fold(1_i64, |value, r| {
                value * i64::from(n - r) / i64::from(r + 1)
            })
        };
        for (first_power, second_power) in [(2, 2), (3, 2), (3, 3)] {
            let first = raised_cut_expansion(&context, first_power).unwrap();
            let second = raised_cut_expansion(&context, second_power).unwrap();
            let mut evaluated = context.zero();
            let mut without_mixed_derivatives = context.zero();
            for (&first_jet, first_weight) in &first {
                for (&second_jet, second_weight) in &second {
                    // For H=(E1+E2)^4 the exact mixed derivative depends on
                    // r1+r2, so H is not split into independent leg kernels.
                    let derivative = first_jet.1 + second_jet.1;
                    if derivative > degree {
                        continue;
                    }
                    let remaining = degree - derivative;
                    let derivative_factor =
                        (0..derivative).fold(1_i64, |value, r| value * i64::from(degree - r));
                    let mut moment = context.zero();
                    for first_energy in 0..=remaining {
                        let coefficient =
                            context.integer(derivative_factor * binomial(remaining, first_energy));
                        let first_moment = spatial_first_moment(
                            &context,
                            1,
                            &context.integer(first_jet.0 + i64::from(first_energy)),
                            first_jet.2,
                        );
                        let second_moment = spatial_first_moment(
                            &context,
                            1,
                            &context.integer(second_jet.0 + i64::from(remaining - first_energy)),
                            second_jet.2,
                        );
                        moment = &moment + &(&coefficient * &(&first_moment * &second_moment));
                    }
                    let contribution = &(first_weight * second_weight) * &moment;
                    evaluated = &evaluated + &contribution;
                    if first_jet.1 == 0 || second_jet.1 == 0 {
                        without_mixed_derivatives = &without_mixed_derivatives + &contribution;
                    }
                }
            }
            // Independently expand H before integrating the two RAISED
            // shell distributions; no jet or mass derivative is used here.
            let mut expected = context.zero();
            for first_energy in 0..=degree {
                let coefficient = context.integer(binomial(degree, first_energy));
                let first_moment = raised_euclidean_moment(
                    &context,
                    first_power,
                    &context.integer(i64::from(first_energy)),
                );
                let second_moment = raised_euclidean_moment(
                    &context,
                    second_power,
                    &context.integer(i64::from(degree - first_energy)),
                );
                expected = &expected + &(&coefficient * &(&first_moment * &second_moment));
            }
            assert_eq!(evaluated, expected);
            assert_ne!(without_mixed_derivatives, expected);
        }
    }

    #[test]
    fn native_spatial_source_matches_independent_weighted_ibp_and_nonzero_surfaces() {
        use crate::solver::instantiate::instantiate_polynomial;
        use crate::solver::{Integral, Seed};

        use super::super::reference_spatial::spatial_sources;

        type WeightedKey = (u32, i64, u32); // (shell power, energy power, W index)
        type ComplexRow = BTreeMap<WeightedKey, (Coefficient, Coefficient)>;

        fn add(
            context: &CoefficientContext,
            row: &mut ComplexRow,
            key: WeightedKey,
            phase: &(Coefficient, Coefficient),
            factor: &Coefficient,
        ) {
            let previous = row
                .entry(key)
                .or_insert_with(|| (context.zero(), context.zero()));
            previous.0 = &previous.0 + &(&phase.0 * factor);
            previous.1 = &previous.1 + &(&phase.1 * factor);
            row.retain(|_, (real, imag)| !real.is_zero() || !imag.is_zero());
        }

        fn phase(
            context: &CoefficientContext,
            energy: i64,
            sigma: i64,
        ) -> (Coefficient, Coefficient) {
            let sign = if sigma == -1 && energy % 2 != 0 {
                -1
            } else {
                1
            };
            match energy.rem_euclid(4) {
                0 => (context.integer(sign), context.zero()),
                1 => (context.zero(), context.integer(sign)),
                2 => (context.integer(-sign), context.zero()),
                3 => (context.zero(), context.integer(-sign)),
                _ => unreachable!(),
            }
        }

        fn integrate(context: &CoefficientContext, row: &ComplexRow) -> (Coefficient, Coefficient) {
            row.iter().fold(
                (context.zero(), context.zero()),
                |(real, imag), (&(cut, energy, b), coefficient)| {
                    let moment = spatial_first_moment(context, cut, &context.integer(energy), b);
                    (
                        &real + &(&coefficient.0 * &moment),
                        &imag + &(&coefficient.1 * &moment),
                    )
                },
            )
        }

        // These are the actual rows independently differentiated in spatial
        // coordinates and checked against the native full-D generator. This
        // test binds that source corpus to the occupied-cut convention. It
        // covers the one-loop source, not full three-loop cut-certificate replay.
        let sources = spatial_sources::<2>(1).unwrap();
        let context = &sources.coefficients;
        let d = context.parameter("d").unwrap();
        for a in [1_i16, 2] {
            for alpha in [0_i16, 1, 2, 4] {
                let seed = Seed {
                    integral: Integral::numeric([a, -alpha]).unwrap(),
                    shifts: [0; 2],
                };
                for sigma in [-1, 1] {
                    let mut converted_shells = ComplexRow::new();
                    let mut simple_shells = ComplexRow::new();
                    for term in &sources.rows[0] {
                        let coefficient = instantiate_polynomial(
                            &term.coefficient,
                            &seed,
                            &sources.index_variables,
                            &[None; 2],
                            None,
                        )
                        .unwrap();
                        let power = u32::try_from(a + term.integral[0].value()).unwrap();
                        let energy = i64::from(alpha - term.integral[1].value());
                        let energy_phase = phase(context, energy, sigma);
                        // Euclidean D_a^alpha = (i*sigma)^alpha (-1)^a J_a.
                        let shell_sign = context.integer(if power % 2 == 0 { 1 } else { -1 });
                        add(
                            context,
                            &mut converted_shells,
                            (power, energy, 0),
                            &energy_phase,
                            &(&coefficient * &shell_sign),
                        );
                        for ((shift, derivative, b), weight) in
                            raised_cut_expansion(context, power).unwrap()
                        {
                            // H=E^energy. Differentiate the entire numerator
                            // before absorbing its residual power into a key.
                            let derivative_factor =
                                falling_factorial(context, &context.integer(energy), derivative);
                            add(
                                context,
                                &mut simple_shells,
                                (1, energy + shift - i64::from(derivative), b),
                                &energy_phase,
                                &(&coefficient * &(&weight * &derivative_factor)),
                            );
                        }
                    }

                    // Independently differentiate the weighted distribution:
                    // partial_p·p[C_a(g) W0] = d C_a W0 + 2a p² C_(a+1) W0.
                    // Here g=E²-p², partial_p C_a=2a p C_(a+1), and
                    // p² C_(a+1)=E² C_(a+1)-C_a. W0 depends on independent
                    // E, hence has no spatial derivative before shell integration.
                    let common_phase = phase(context, i64::from(alpha), sigma);
                    let original_sign = context.integer(if a % 2 == 0 { 1 } else { -1 });
                    let mut direct_weighted = ComplexRow::new();
                    add(
                        context,
                        &mut direct_weighted,
                        (a as u32, i64::from(alpha), 0),
                        &common_phase,
                        &(&original_sign * &(&d - &context.integer(2 * i64::from(a)))),
                    );
                    add(
                        context,
                        &mut direct_weighted,
                        (a as u32 + 1, i64::from(alpha) + 2, 0),
                        &common_phase,
                        &(&original_sign * &context.integer(2 * i64::from(a))),
                    );
                    assert_eq!(converted_shells, direct_weighted);
                    assert!(converted_shells.keys().any(|(power, _, _)| *power > 1));
                    assert_eq!(
                        integrate(context, &direct_weighted),
                        (context.zero(), context.zero())
                    );
                    assert_eq!(
                        integrate(context, &simple_shells),
                        (context.zero(), context.zero())
                    );
                    assert!(simple_shells.keys().any(|(_, _, b)| *b > 0));

                    if a == 1 {
                        // A second independent derivation integrates the
                        // shell first, then differentiates the occupied radial
                        // measure p^(d+alpha-1) theta(1-p). Its surface at p=1
                        // is nonzero; dropping that surface spoils the IBP.
                        let mut direct_radial = ComplexRow::new();
                        add(
                            context,
                            &mut direct_radial,
                            (1, i64::from(alpha), 0),
                            &common_phase,
                            &(-(&d + &context.integer(i64::from(alpha) - 1))),
                        );
                        add(
                            context,
                            &mut direct_radial,
                            (1, i64::from(alpha) + 1, 1),
                            &common_phase,
                            &context.one(),
                        );
                        assert_eq!(simple_shells, direct_radial);
                        let bulk_only = simple_shells
                            .iter()
                            .filter(|((_, _, b), _)| *b == 0)
                            .map(|(key, coefficient)| (*key, coefficient.clone()))
                            .collect();
                        assert_ne!(
                            integrate(context, &bulk_only),
                            (context.zero(), context.zero())
                        );
                    }
                }
            }
        }
    }
}
