//! Physics controls for the guarded identity interface.
//!
//! These tests check bounded weighted IBP identities, not full master-integral
//! reductions. Phase-space conventions follow arXiv:1609.04339, sections 2, 5;
//! the connected four-loop C cut is arXiv:2501.17921v2, equations (31)--(34).
//! The one-loop moments below are independently integrated distributionally.

use std::sync::Arc;

use symbolica::domains::integer::Z;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::solver::{Integral, PolynomialRow, SearchOptions, Term};

use super::*;

fn bounds(lower: Option<i64>, upper: Option<i64>) -> IndexBounds {
    IndexBounds::new(lower, upper).unwrap()
}

fn polynomial_row<const N: usize>(
    context: &CoefficientContext,
    terms: &[([i16; N], &str)],
) -> PolynomialRow<N> {
    terms
        .iter()
        .map(|(shift, value)| Term {
            integral: Integral::symbolic(*shift).unwrap(),
            coefficient: context.coefficient_fixture(value).numerator,
        })
        .collect()
}

fn bounded_search() -> SearchOptions {
    SearchOptions {
        max_depth: Some(2),
        ..Default::default()
    }
}

fn one_loop_sources() -> (CoefficientContext, GuardedSourceSystem<3>) {
    // a: massless cut power; n: inverse energy power (n <= 0 is a
    // polynomial numerator); b: occupation/surface order. D = d+1.
    let context = CoefficientContext::new(["d", "mu", "a", "n", "b"]);
    let domain = |support| {
        IndexDomain::new([bounds(Some(1), None), bounds(None, Some(0)), support]).unwrap()
    };
    let sources = vec![
        GuardedSource::new(
            "phase-space:euler:bulk",
            polynomial_row(&context, &[([0, 0, 0], "d+1-2*a-n"), ([0, 0, 1], "-mu")]),
            domain(IndexBounds::fixed(0)),
        ),
        GuardedSource::new(
            "phase-space:euler:surface",
            polynomial_row(&context, &[([0, 0, 0], "d+1-2*a-n-b"), ([0, 0, 1], "mu*b")]),
            domain(bounds(Some(1), None)),
        ),
        GuardedSource::new(
            "phase-space:support:delta",
            polynomial_row(&context, &[([0, 0, 0], "mu"), ([0, -1, 0], "-1")]),
            domain(IndexBounds::fixed(1)),
        ),
        GuardedSource::new(
            "phase-space:support:derivative",
            polynomial_row(
                &context,
                &[([0, 0, 0], "mu"), ([0, -1, 0], "-1"), ([0, 0, -1], "-1")],
            ),
            domain(bounds(Some(2), None)),
        ),
    ];
    let system = GuardedSourceSystem::new(
        "massless-positive-energy-phase-space:W-v1",
        [
            IndexRole::RequiredCut,
            IndexRole::Ordinary,
            IndexRole::Occupation,
        ],
        [2, 3, 4],
        sources,
    )
    .unwrap();
    (context, system)
}

/// Integral of E^q W_b(1-E) over E >= 0, analytically continued from
/// Re(q)>-1. Derivatives are evaluated directly on E^q, independently of IBP.
fn radial_moment(context: &CoefficientContext, q: &Coefficient, b: i64) -> Coefficient {
    assert!(b >= 0);
    if b == 0 {
        return &context.one() / &(q + &context.one());
    }
    let mut value = context.one();
    for derivative in 0..b - 1 {
        value = -(&value * &(q - &context.integer(derivative)));
        value = &value / &context.integer(derivative + 1);
    }
    value
}

/// J(a,n;b) = int dE d^d p theta(E) C_a(E^2-p^2) E^(-n) W_b(1-E).
/// Spatial integration first leaves (Omega_d/2) c_a E^(d-2a-n).
/// We divide out only the common nonzero Omega_d/2 normalization.
fn one_loop_moment(context: &CoefficientContext, point: &[i64; 3]) -> Coefficient {
    let [a, n, b] = *point;
    assert!(a > 0 && n <= 0 && b >= 0);
    let d = context.parameter("d").unwrap();
    let half_d = &d / &context.integer(2);
    let mut cut = context.one();
    for derivative in 0..a - 1 {
        cut = -(&cut * &(&half_d - &context.integer(derivative + 1)));
        cut = &cut / &context.integer(derivative + 1);
    }
    let q = &d - &context.integer(2 * a + n);
    &cut * &radial_moment(context, &q, b)
}

fn at_mu_one(coefficient: &Coefficient) -> Coefficient {
    Coefficient::from_num_den(
        coefficient.numerator.replace(1, &1.into()),
        coefficient.denominator.replace(1, &1.into()),
        &Z,
        true,
    )
}

#[test]
fn raised_mass_shell_cuts_and_energy_numerators_match_independent_moments() {
    let (context, system) = one_loop_sources();
    for a in 1..=3 {
        for n in -3..=0 {
            for b in 0..=4 {
                let point = [a, n, b];
                for (row, metadata) in system.native_sources().rows().iter().zip(system.sources()) {
                    if !metadata.domain.contains(&point) {
                        continue;
                    }
                    let mut residual = context.zero();
                    for term in row {
                        let mut coefficient = term.coefficient.clone();
                        coefficient = coefficient.replace(1, &1.into());
                        for (axis, variable) in [2, 3, 4].into_iter().enumerate() {
                            coefficient = coefficient.replace(variable, &point[axis].into());
                        }
                        let successor = std::array::from_fn(|axis| {
                            point[axis] + i64::from(term.integral[axis].value())
                        });
                        residual = &residual
                            + &(&Coefficient::from(coefficient)
                                * &one_loop_moment(&context, &successor));
                    }
                    assert!(
                        residual.is_zero(),
                        "{}, point={point:?}: {residual}",
                        metadata.id
                    );
                }
            }
        }
    }
    assert!(!one_loop_moment(&context, &[1, 0, 0]).is_zero());
    assert!(!one_loop_moment(&context, &[2, -2, 1]).is_zero());
    assert!(
        !system.is_zero(&[1, 0, 0]),
        "theta index zero is a bulk integral"
    );
    assert!(
        system.is_zero(&[0, 0, 0]),
        "a missing required mass-shell cut is zero"
    );
    assert!(
        !system.valid_indices(&[1, 0, -1]),
        "negative occupation order is undefined"
    );
}

#[test]
fn native_discovery_replay_and_application_preserve_nonzero_fermi_surfaces() {
    let (context, system) = one_loop_sources();
    let requested = (1..=3)
        .map(|b| {
            IndexDomain::new([
                bounds(Some(1), None),
                bounds(None, Some(0)),
                IndexBounds::fixed(b),
            ])
            .unwrap()
        })
        .collect();
    let solution = system
        .solve_domains(requested, bounded_search(), 48)
        .unwrap();
    assert!(!solution.rules.is_empty(), "{:#?}", solution.unresolved);
    let generated_rules = solution.rules.len();
    let unresolved_domains = solution.unresolved.len();
    let program = GuardedProgram::new(Arc::new(system), solution.rules, []).unwrap();
    let mut applied = 0;
    let mut tested_targets = 0;
    for a in 1..=3 {
        for n in -3..=0 {
            for b in 1..=3 {
                let point = [a, n, b];
                tested_targets += 1;
                let result = program.apply(&point).unwrap();
                if !matches!(result.status, GuardedApplicationStatus::Applied { .. }) {
                    continue;
                }
                applied += 1;
                let actual =
                    result
                        .terms
                        .iter()
                        .fold(context.zero(), |sum, (child, coefficient)| {
                            &sum + &(&at_mu_one(coefficient) * &one_loop_moment(&context, child))
                        });
                assert_eq!(actual, one_loop_moment(&context, &point), "point={point:?}");
            }
        }
    }
    assert!(
        applied >= 12,
        "rules must have reusable raised-cut/numerator coverage, got {applied}"
    );
    eprintln!(
        "weighted IBP pilot: loops=1 generated_rules={generated_rules} unresolved_domains={unresolved_domains} tested_targets={tested_targets} applied={applied}"
    );
    let undefined = program.apply(&[1, 0, -1]).unwrap();
    assert!(matches!(
        undefined.status,
        GuardedApplicationStatus::Unresolved(_)
    ));
    let absent_cut = program.apply(&[0, 0, 0]).unwrap();
    assert!(matches!(absent_cut.status, GuardedApplicationStatus::Zero));
    let bulk = program.apply(&[1, 0, 0]).unwrap();
    assert!(!matches!(bulk.status, GuardedApplicationStatus::Zero));
}

fn connected_cut_sources<const N: usize>(
    loops: usize,
) -> (CoefficientContext, GuardedSourceSystem<N>) {
    // Two independent positive-energy cuts, an exchanged momentum L=P-Q,
    // and zero, one, or two vacuum loops in its connected self energy:
    // 2 loops: 1/L^2 (the doubly cut sunset of arXiv:1609.04339 eq.31).
    // 3 loops: 1/L^2 int_R 1/[R^2 (R+L)^2] (bubble insertion control).
    // 4 loops: 1/L^2 int_RS 1/[R^2 S^2 (R+S+L)^2], the published C cut.
    // The 3-loop graph is a derived connected topology control, not a claim
    // to reproduce the published full QCD Mercedes numerator/counterterms.
    let denominators = N - 3;
    assert_eq!(
        denominators,
        match loops {
            2 => 3,
            3 => 5,
            4 => 6,
            _ => unreachable!(),
        }
    );
    let mut names = vec!["d".to_owned(), "mu".to_owned()];
    names.extend((0..denominators).map(|i| format!("a{i}")));
    names.extend(["n".to_owned(), "b0".to_owned(), "b1".to_owned()]);
    let context = CoefficientContext::new(names);
    let mut sources = Vec::new();
    for surface in 0..4 {
        let mut diagonal = format!("{loops}*(d+1)-n");
        for i in 0..denominators {
            diagonal.push_str(&format!("-2*a{i}"));
        }
        let mut terms = Vec::new();
        let mut domain = [bounds(Some(1), None); N];
        domain[denominators] = bounds(None, Some(0));
        for support in 0..2 {
            let axis = denominators + 1 + support;
            let mut shift = [0; N];
            shift[axis] = 1;
            let coefficient = if surface & (1 << support) == 0 {
                domain[axis] = IndexBounds::fixed(0);
                context.coefficient_fixture("-mu")
            } else {
                domain[axis] = bounds(Some(1), None);
                diagonal.push_str(&format!("-b{support}"));
                context.coefficient_fixture(&format!("mu*b{support}"))
            };
            terms.push(Term {
                integral: Integral::symbolic(shift).unwrap(),
                coefficient: coefficient.numerator,
            });
        }
        terms.push(Term {
            integral: Integral::symbolic([0; N]).unwrap(),
            coefficient: context.coefficient_fixture(&diagonal).numerator,
        });
        sources.push(GuardedSource::new(
            format!("{loops}-loop:connected-cut:euler:{surface}"),
            terms,
            IndexDomain::new(domain).unwrap(),
        ));
    }
    let roles = std::array::from_fn(|axis| {
        if axis < 2 {
            IndexRole::RequiredCut
        } else if axis > denominators {
            IndexRole::Occupation
        } else {
            IndexRole::Ordinary
        }
    });
    let system = GuardedSourceSystem::new(
        format!("connected-massless-{loops}-loop-two-cut:W-v1"),
        roles,
        std::array::from_fn(|axis| axis + 2),
        sources,
    )
    .unwrap();
    (context, system)
}

/// Vacuum subloops give (L^2)^(-c), with c=sum(a_uncut)-(loops-2)D/2.
/// With P,Q on shell, L^2=2pq(1-cos(theta)); angular and vacuum prefactors
/// are common to every support order and cancel in these IBP checks. The
/// remaining independently integrated radial moments are exact functions of d.
fn connected_cut_moment<const N: usize>(
    context: &CoefficientContext,
    loops: usize,
    point: &[i64; N],
) -> Coefficient {
    let denominators = N - 3;
    assert_eq!(point[0], 1);
    assert_eq!(point[1], 1);
    let d = context.parameter("d").unwrap();
    let dimensions = &d + &context.one();
    let subloop_degree =
        &(&context.integer((loops - 2) as i64) * &dimensions) / &context.integer(2);
    let uncut_powers: i64 = point[2..denominators].iter().sum();
    let c = &context.integer(uncut_powers) - &subloop_degree;
    let q = &(&d - &context.integer(2)) - &c;
    let first = &q - &context.integer(point[denominators]);
    &radial_moment(context, &first, point[N - 2]) * &radial_moment(context, &q, point[N - 1])
}

fn check_connected_cut<const N: usize>(loops: usize) {
    let (context, system) = connected_cut_sources::<N>(loops);
    let mut domains = Vec::new();
    for first in 0..=2 {
        for second in 0..=2 {
            if first == 0 && second == 0 {
                continue;
            }
            let mut domain = [IndexBounds::fixed(1); N];
            domain[N - 3] = bounds(None, Some(0));
            domain[N - 2] = IndexBounds::fixed(first);
            domain[N - 1] = IndexBounds::fixed(second);
            domains.push(IndexDomain::new(domain).unwrap());
        }
    }
    let solution = system.solve_domains(domains, bounded_search(), 96).unwrap();
    assert!(
        !solution.rules.is_empty(),
        "{loops} loops: {:#?}",
        solution.unresolved
    );
    let generated_rules = solution.rules.len();
    let unresolved_domains = solution.unresolved.len();
    let program = GuardedProgram::new(Arc::new(system), solution.rules, []).unwrap();
    let mut applied = 0;
    let mut tested_targets = 0;
    for numerator in -2..=0 {
        for first in 0..=2 {
            for second in 0..=2 {
                let mut point = [1; N];
                point[N - 3] = numerator;
                point[N - 2] = first;
                point[N - 1] = second;
                tested_targets += 1;
                let expected = connected_cut_moment(&context, loops, &point);
                assert!(
                    !expected.is_zero(),
                    "nonzero weighted control: {loops} {point:?}"
                );
                let result = program.apply(&point).unwrap();
                if !matches!(result.status, GuardedApplicationStatus::Applied { .. }) {
                    continue;
                }
                applied += 1;
                let actual =
                    result
                        .terms
                        .iter()
                        .fold(context.zero(), |sum, (child, coefficient)| {
                            &sum + &(&at_mu_one(coefficient)
                                * &connected_cut_moment(&context, loops, child))
                        });
                assert_eq!(actual, expected, "{loops} loops, point={point:?}");
            }
        }
    }
    assert!(
        applied >= 3,
        "{loops}-loop bounded pilot produced only {applied} applications"
    );
    eprintln!(
        "weighted IBP pilot: loops={loops} generated_rules={generated_rules} unresolved_domains={unresolved_domains} tested_targets={tested_targets} applied={applied}"
    );
}

#[test]
fn connected_two_loop_weighted_sunset_matches_independent_radial_integrals() {
    check_connected_cut::<6>(2);
}

#[test]
fn connected_three_loop_weighted_bubble_insertion_matches_independent_radial_integrals() {
    check_connected_cut::<8>(3);
}

#[test]
fn published_four_loop_c_cut_has_exact_nonzero_surface_pilot() {
    check_connected_cut::<9>(4);
}
