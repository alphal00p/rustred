//! Conversion controls for the supplied Euclidean D5 and D4 relations.
//!
//! P0 is the shifted Euclidean integration variable. For a contour at
//! Im(P0)=sigma*mu, the simple-pole medium term is
//! -i^alpha sigma^alpha int_p E^(alpha-1) theta(mu-E)/2. Raised powers are
//! (-d/dm^2)^(a-1)/(a-1)! of the entire expression, including theta.
//! This is the prescription of arXiv:0912.1856, section II.1 and equation
//! (29), and arXiv:1609.04339, sections 2 and 5. No pasted reduction is used
//! as a source identity here. Spatial dimension is d, not spacetime D=d+1.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::solver::{Integral, SearchOptions, Term};

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ComplexCoefficient {
    real: Coefficient,
    imag: Coefficient,
}

impl ComplexCoefficient {
    fn phase_times(
        context: &CoefficientContext,
        sigma: i64,
        alpha: i64,
        value: &Coefficient,
    ) -> Self {
        assert!(matches!(sigma, -1 | 1));
        let value = if sigma == -1 && alpha % 2 != 0 {
            -value.clone()
        } else {
            value.clone()
        };
        match alpha.rem_euclid(4) {
            0 => Self {
                real: value,
                imag: context.zero(),
            },
            1 => Self {
                real: context.zero(),
                imag: value,
            },
            2 => Self {
                real: -value,
                imag: context.zero(),
            },
            3 => Self {
                real: context.zero(),
                imag: -value,
            },
            _ => unreachable!(),
        }
    }

    fn square_times(&self, factor: &Coefficient) -> Self {
        Self {
            real: &(&(&self.real * &self.real) - &(&self.imag * &self.imag)) * factor,
            imag: &(&(&self.real * &self.imag) + &(&self.imag * &self.real)) * factor,
        }
    }
}

/// Divide every one-loop expression by K_d/2, where
/// K_d = Omega_(d-1)/(2*pi)^d, and set mu=1. The common normalization and
/// homogeneous power of mu are restored explicitly in the accompanying docs.
/// Integrating the cut distribution over spatial momentum first gives
/// c_a(d) int_0^infinity dE E^(d-2a+energy) W_b(1-E).
fn spatial_first_cut(context: &CoefficientContext, a: i64, energy: i64, b: i64) -> Coefficient {
    assert!(a >= 1 && b >= 0);
    let d = context.parameter("d").unwrap();
    let mut cut = context.one();
    for r in 1..a {
        cut = -(&cut * &(&(&d / &context.integer(2)) - &context.integer(r)));
        cut = &cut / &context.integer(r);
    }
    let q = &d + &context.integer(energy - 2 * a);
    let radial = if b == 0 {
        &context.one() / &(&q + &context.one())
    } else {
        let mut radial = context.one();
        for r in 1..b {
            radial = -(&radial * &(&q - &context.integer(r - 1)));
            radial = &radial / &context.integer(r);
        }
        radial
    };
    &cut * &radial
}

/// A different integration order: first take the simple contour residue,
/// then differentiate its full occupied phase-space measure with respect
/// to m^2. A key is (energy power, W index) of a SIMPLE mass-shell cut.
/// d_m2 J_1(k,b) = (k-1)/2 J_1(k-2,b) + t_b/2 J_1(k-1,b+1),
/// with t_0=-1 and t_b=b for b>=1. This follows from E'=1/(2E).
fn differentiated_residue(
    context: &CoefficientContext,
    a: i64,
    alpha: i64,
) -> BTreeMap<(i64, i64), Coefficient> {
    let mut terms = BTreeMap::from([((alpha, 0), context.one())]);
    for _ in 1..a {
        let mut next = BTreeMap::new();
        for ((energy, b), coefficient) in terms {
            let children = [
                ((energy - 2, b), energy - 1),
                ((energy - 1, b + 1), if b == 0 { -1 } else { b }),
            ];
            for (point, numerator) in children {
                let weight = &(&coefficient * &context.integer(numerator)) / &context.integer(2);
                let old = next.entry(point).or_insert_with(|| context.zero());
                *old = &*old + &weight;
            }
        }
        next.retain(|_, coefficient| !coefficient.is_zero());
        terms = next;
    }
    // The original simple contour residue supplies a minus sign, followed
    // by (-d_m2)^(a-1)/(a-1)! for the Euclidean propagator power.
    let mut normalization = context.integer(-1);
    for r in 1..a {
        normalization = -(&normalization / &context.integer(r));
    }
    for coefficient in terms.values_mut() {
        *coefficient = &*coefficient * &normalization;
    }
    terms
}

fn evaluate_residue(
    context: &CoefficientContext,
    terms: &BTreeMap<(i64, i64), Coefficient>,
) -> Coefficient {
    terms
        .iter()
        .fold(context.zero(), |sum, ((energy, b), weight)| {
            &sum + &(weight * &spatial_first_cut(context, 1, *energy, *b))
        })
}

#[test]
fn euclidean_energy_moments_keep_phases_and_differentiated_fermi_surfaces() {
    let started = Instant::now();
    let context = CoefficientContext::new(["d"]);
    let mut checked = 0;
    for a in 1..=3 {
        for alpha in 0..=4 {
            let terms = differentiated_residue(&context, a, alpha);
            assert!(terms.keys().all(|(_, b)| *b <= a - 1));
            let residue = evaluate_residue(&context, &terms);
            let cut = spatial_first_cut(&context, a, alpha, 0);
            // C_a(g)=(-1)^(a-1) delta^(a-1)(g)/(a-1)! and
            // g=E^2-p^2-m^2, so Euclidean D5=(-1)^a(i*sigma)^alpha J_a.
            let signed_cut = if a % 2 == 0 { cut } else { -cut };
            for sigma in [-1, 1] {
                let from_contour =
                    ComplexCoefficient::phase_times(&context, sigma, alpha, &residue);
                let from_cut = ComplexCoefficient::phase_times(&context, sigma, alpha, &signed_cut);
                assert_eq!(
                    from_contour, from_cut,
                    "a={a}, alpha={alpha}, sigma={sigma}"
                );
                checked += 1;
            }
            let plus = ComplexCoefficient::phase_times(&context, 1, alpha, &residue);
            let minus = ComplexCoefficient::phase_times(&context, -1, alpha, &residue);
            if alpha % 2 == 0 {
                assert_eq!(plus, minus);
                assert!(plus.imag.is_zero());
            } else {
                assert_eq!(plus.imag, -minus.imag.clone());
                assert!(plus.real.is_zero());
            }
        }
    }
    let full = differentiated_residue(&context, 2, 1);
    assert_eq!(full.len(), 1);
    assert_eq!(
        full.get(&(0, 1)),
        Some(&context.coefficient_fixture("-1/2"))
    );
    let without_surfaces = full
        .iter()
        .filter(|((_, b), _)| *b == 0)
        .map(|(point, coefficient)| (*point, coefficient.clone()))
        .collect();
    assert!(evaluate_residue(&context, &without_surfaces).is_zero());
    assert!(!evaluate_residue(&context, &full).is_zero());
    eprintln!(
        "pasted D5 conversion: max_cut_power=3 max_energy_power=4 sigma_cases={checked} exact_check_us={}",
        started.elapsed().as_micros()
    );
}

fn conversion_sources(context: &CoefficientContext) -> GuardedSourceSystem<3> {
    let bounds = |lower, upper| IndexBounds::new(lower, upper).unwrap();
    let mut sources = Vec::new();
    for bulk in [true, false] {
        // Integral coordinates are (cut a, inverse energy n, occupation b).
        // This is int dE d/dE [E^(-n-1) C_(a-1) W_b]=0, recentered
        // so the raised cut is the candidate head. It is not pasted R1.
        let terms = [
            ([-1, 2, 0], "-n-1"),
            ([0, 0, 0], "-2*(a-1)"),
            ([-1, 1, 1], if bulk { "-1" } else { "b" }),
        ]
        .into_iter()
        .map(|(shift, coefficient)| Term {
            integral: Integral::symbolic(shift).unwrap(),
            coefficient: context.coefficient_fixture(coefficient).numerator,
        })
        .collect();
        sources.push(GuardedSource::new(
            if bulk {
                "energy-total-derivative:bulk"
            } else {
                "energy-total-derivative:surface"
            },
            terms,
            IndexDomain::new([
                bounds(Some(2), None),
                bounds(None, Some(-1)),
                if bulk {
                    IndexBounds::fixed(0)
                } else {
                    bounds(Some(1), None)
                },
            ])
            .unwrap(),
        ));
    }
    GuardedSourceSystem::new(
        "pasted-D5:positive-energy-cut:W-v1",
        [
            IndexRole::RequiredCut,
            IndexRole::Ordinary,
            IndexRole::Occupation,
        ],
        [2, 3, 4],
        sources,
    )
    .unwrap()
}

#[test]
fn pasted_sunset_relation_matches_independent_double_cut_and_native_surface_conversion() {
    let started = Instant::now();
    let context = CoefficientContext::new(["d", "mu", "a", "n", "b"]);
    let system = conversion_sources(&context);
    let build_us = started.elapsed().as_micros();
    let requested = IndexDomain::new([
        IndexBounds::new(Some(2), None).unwrap(),
        IndexBounds::fixed(-1),
        IndexBounds::fixed(0),
    ])
    .unwrap();
    let discovery_started = Instant::now();
    let solution = system
        .solve_domain(
            requested,
            SearchOptions {
                max_depth: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let discovery_us = discovery_started.elapsed().as_micros();
    let rules = solution.rules.len();
    let unresolved = solution.unresolved.len();
    assert!(rules > 0, "{:#?}", solution.unresolved);
    let replay_started = Instant::now();
    let program = GuardedProgram::new(Arc::new(system), solution.rules, []).unwrap();
    let replay_us = replay_started.elapsed().as_micros();
    let application_started = Instant::now();
    let application = program.apply(&[2, -1, 0]).unwrap();
    assert!(matches!(
        application.status,
        GuardedApplicationStatus::Applied { .. }
    ));
    assert_eq!(application.terms.len(), 1);
    assert_eq!(
        application.terms.get(&[1, 0, 1]),
        Some(&context.coefficient_fixture("-1/2"))
    );
    let converted = application
        .terms
        .iter()
        .fold(context.zero(), |sum, (point, weight)| {
            &sum + &(weight * &spatial_first_cut(&context, point[0], -point[1], point[2]))
        });
    let raised_application = program.apply(&[3, -1, 0]).unwrap();
    assert!(matches!(
        raised_application.status,
        GuardedApplicationStatus::Applied { .. }
    ));
    let raised_value =
        raised_application
            .terms
            .iter()
            .fold(context.zero(), |sum, (point, weight)| {
                &sum + &(weight * &spatial_first_cut(&context, point[0], -point[1], point[2]))
            });
    assert_eq!(raised_value, spatial_first_cut(&context, 3, 1, 0));
    let application_us = application_started.elapsed().as_micros();
    let d5 = ComplexCoefficient::phase_times(&context, 1, 1, &converted);

    // For the massless ++B sunset the zero/single-cut terms are scaleless.
    // The surviving double cut has kernel 1/[2*p*q*(1-cos(theta))].
    // The normalized d-dimensional angular beta integral is
    // <1/(1-cos(theta))>=(d-2)/(d-3); the two independent radial
    // integrals are int_0^1 dp p^(d-3)=1/(d-2). Thus the sunset divided
    // by (K_d/2)^2 is the following independently integrated quantity.
    let angular = context.coefficient_fixture("(d-2)/(d-3)");
    let radial = context.coefficient_fixture("1/(d-2)");
    let sunset = &(&angular * &(&radial * &radial)) / &context.integer(2);
    let pasted_rhs = d5.square_times(&context.coefficient_fixture("-2/((d-2)*(d-3))"));
    assert!(pasted_rhs.imag.is_zero());
    assert_eq!(sunset, pasted_rhs.real);
    // Losing the Euclidean i phase produces the opposite nonzero answer.
    let phase_dropped_rhs =
        &(&converted * &converted) * &context.coefficient_fixture("-2/((d-2)*(d-3))");
    assert_eq!(phase_dropped_rhs, -sunset.clone());
    assert!(!sunset.is_zero());
    eprintln!(
        "pasted D4/R1: build_us={build_us} discovery_us={discovery_us} replay_us={replay_us} apply_us={application_us} generated_rules={rules} unresolved_domains={unresolved} tested_targets=2 applied=2"
    );
}
