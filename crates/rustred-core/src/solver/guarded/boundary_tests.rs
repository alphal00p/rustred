use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Integral, SearchOptions, Term};
use std::sync::Arc;

fn ge(n: i64) -> IndexBounds {
    IndexBounds::new(Some(n), None).unwrap()
}
fn options() -> SearchOptions {
    SearchOptions {
        max_depth: Some(2),
        ..Default::default()
    }
}

#[test]
fn bulk_to_surface_transition_preserves_theta_and_strict_descent() {
    let c = CoefficientContext::new(["a", "b"]);
    let domain = IndexDomain::new([ge(2), IndexBounds::fixed(0)]).unwrap();
    let source = GuardedSource::new(
        "bulk-boundary",
        vec![
            Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: c.one().numerator,
            },
            Term {
                integral: Integral::symbolic([-1, 1]).unwrap(),
                coefficient: c.integer(-1).numerator,
            },
        ],
        domain.clone(),
    );
    let system = GuardedSourceSystem::new(
        "synthetic-boundary",
        [IndexRole::Ordinary, IndexRole::Occupation],
        [0, 1],
        vec![source],
    )
    .unwrap();
    let solution = system.solve_domain(domain, options()).unwrap();
    assert!(solution.unresolved.is_empty(), "{:?}", solution.unresolved);
    assert_eq!(solution.rules.len(), 1);
    let program = GuardedProgram::new(Arc::new(system), solution.rules, [[1, 1]]).unwrap();
    let application = program.apply(&[2, 0]).unwrap();
    assert!(matches!(
        application.status,
        GuardedApplicationStatus::Applied { .. }
    ));
    assert_eq!(application.terms.get(&[1, 1]), Some(&c.one()));
    assert!(matches!(
        program.apply(&[2, -1]).unwrap().status,
        GuardedApplicationStatus::Unresolved(GuardedApplicationFailure::InvalidOccupation { .. })
    ));
}

#[test]
fn recentered_rule_does_not_cross_the_source_lower_boundary() {
    let c = CoefficientContext::new(["b", "x"]);
    let source = GuardedSource::new(
        "higher-surfaces",
        vec![
            Term {
                integral: Integral::symbolic([0]).unwrap(),
                coefficient: c.one().numerator,
            },
            Term {
                integral: Integral::symbolic([-1]).unwrap(),
                coefficient: c.coefficient_fixture("-x").numerator,
            },
        ],
        IndexDomain::new([ge(2)]).unwrap(),
    );
    let system =
        GuardedSourceSystem::new("synthetic-ray", [IndexRole::Occupation], [0], vec![source])
            .unwrap();
    let solution = system
        .solve_domain(IndexDomain::new([ge(1)]).unwrap(), options())
        .unwrap();
    assert!(
        solution
            .rules
            .iter()
            .any(|rule| rule.domain().contains(&[2]))
    );
    assert!(
        solution
            .rules
            .iter()
            .all(|rule| !rule.domain().contains(&[1]))
    );
    assert!(
        solution
            .unresolved
            .iter()
            .any(|piece| piece.domain.contains(&[1]))
    );
    let program = GuardedProgram::new(Arc::new(system), solution.rules, []).unwrap();
    assert!(matches!(
        program.apply(&[1]).unwrap().status,
        GuardedApplicationStatus::Unresolved(_)
    ));
    assert!(matches!(
        program.apply(&[2]).unwrap().status,
        GuardedApplicationStatus::Applied { .. }
    ));
}

#[test]
fn empty_rhs_still_retains_the_original_source_weight_pole() {
    let c = CoefficientContext::new(["b"]);
    let source = GuardedSource::new(
        "conditional-zero",
        vec![Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient: c.coefficient_fixture("b-1").numerator,
        }],
        IndexDomain::new([ge(1)]).unwrap(),
    );
    let system =
        GuardedSourceSystem::new("synthetic-zero", [IndexRole::Occupation], [0], vec![source])
            .unwrap();
    let solution = system
        .solve_domain(
            IndexDomain::new([ge(1)]).unwrap(),
            SearchOptions {
                max_depth: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!solution.rules.is_empty());
    assert!(solution.rules[0].candidate().rhs.is_empty());
    assert!(
        solution.rules[0]
            .nonzero_conditions()
            .iter()
            .any(|p| *p == c.coefficient_fixture("b-1").numerator)
    );
    assert!(
        solution
            .unresolved
            .iter()
            .any(|piece| piece.domain.contains(&[1]))
    );
    let program = GuardedProgram::new(Arc::new(system), solution.rules, []).unwrap();
    assert!(matches!(
        program.apply(&[1]).unwrap().status,
        GuardedApplicationStatus::Unresolved(_)
    ));
    let applied = program.apply(&[2]).unwrap();
    assert!(matches!(
        applied.status,
        GuardedApplicationStatus::Applied { .. }
    ));
    assert!(applied.terms.is_empty());
}

#[test]
fn surface_boundary_is_discovered_from_bulk_only_source_on_a_ray() {
    let c = CoefficientContext::new(["b"]);
    let source = GuardedSource::new(
        "bulk-to-first-surface",
        vec![
            Term {
                integral: Integral::symbolic([1]).unwrap(),
                coefficient: c.one().numerator,
            },
            Term {
                integral: Integral::symbolic([0]).unwrap(),
                coefficient: c.integer(-1).numerator,
            },
        ],
        IndexDomain::new([IndexBounds::fixed(0)]).unwrap(),
    );
    let system =
        GuardedSourceSystem::new("boundary-only", [IndexRole::Occupation], [0], vec![source])
            .unwrap();
    let found = system
        .solve_domain(IndexDomain::new([ge(1)]).unwrap(), options())
        .unwrap();
    assert!(found.rules.iter().any(|rule| rule.domain().contains(&[1])));
    assert!(
        found
            .unresolved
            .iter()
            .any(|piece| piece.domain.contains(&[3]))
    );
    let program = GuardedProgram::new(Arc::new(system), found.rules, [[0]]).unwrap();
    assert_eq!(program.apply(&[1]).unwrap().terms.get(&[0]), Some(&c.one()));
}

#[test]
fn domain_budget_keeps_a_rule_and_all_unsearched_remainders() {
    let c = CoefficientContext::new(["b", "c"]);
    let source = GuardedSource::new(
        "two-surface-recurrence",
        vec![
            Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: c.one().numerator,
            },
            Term {
                integral: Integral::symbolic([-1, -1]).unwrap(),
                coefficient: c.integer(-1).numerator,
            },
        ],
        IndexDomain::new([ge(2), ge(2)]).unwrap(),
    );
    let system = GuardedSourceSystem::new(
        "two-boundaries",
        [IndexRole::Occupation; 2],
        [0, 1],
        vec![source],
    )
    .unwrap();
    let found = system
        .solve_domains(
            vec![IndexDomain::new([ge(1), ge(1)]).unwrap()],
            options(),
            1,
        )
        .unwrap();
    assert!(
        found
            .rules
            .iter()
            .any(|rule| rule.domain().contains(&[2, 2]))
    );
    for point in [[1, 1], [1, 2], [2, 1]] {
        assert!(
            found
                .unresolved
                .iter()
                .any(|piece| piece.domain.contains(&point)
                    && piece.reason == GuardedUnresolvedReason::DomainBudget)
        );
    }
}

#[test]
fn wide_guard_refinement_keeps_symbolic_rules_and_reports_unsupported_faces() {
    let c = CoefficientContext::new(["b"]);
    let source = GuardedSource::new(
        "wide-surface-recurrence",
        vec![
            Term {
                integral: Integral::symbolic([0]).unwrap(),
                coefficient: c.one().numerator,
            },
            Term {
                integral: Integral::symbolic([-1]).unwrap(),
                coefficient: c.integer(-1).numerator,
            },
        ],
        IndexDomain::new([ge(1000)]).unwrap(),
    );
    let system =
        GuardedSourceSystem::new("wide-boundary", [IndexRole::Occupation], [0], vec![source])
            .unwrap();
    let found = system
        .solve_domain(IndexDomain::new([ge(1)]).unwrap(), options())
        .unwrap();
    assert!(
        found
            .rules
            .iter()
            .any(|rule| rule.domain().contains(&[1002]))
    );
    assert!(
        found
            .unresolved
            .iter()
            .any(|piece| piece.domain.contains(&[1000])
                && piece.reason == GuardedUnresolvedReason::UnsupportedPower)
    );
}
