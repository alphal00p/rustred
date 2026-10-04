//! Downstream-facing arity dispatch and exact, nonzero high-arity reductions.
use rustred::algebra::CoefficientContext;
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::bridge::{
    DynamicSolution, DynamicSolveOptions, certify_laporta, certify_laporta_for, solve_laporta,
    solve_laporta_for, solve_parametric, solve_parametric_for,
};

// A complete scalar-product basis: one massive propagator plus N-1 independent
// linear auxiliaries k.p_i. The target has no auxiliary powers. These are not
// disconnected zero sectors and do not encode a topology-specific dispatcher.
fn family<const N: usize>() -> (IntegralFamily, CoefficientContext) {
    let c = CoefficientContext::try_new(["d", "m2"]).unwrap();
    let denominators = (0..N)
        .map(|i| {
            AffineDenominator::new(
                if i == 0 {
                    -c.parameter("m2").unwrap()
                } else {
                    c.zero()
                },
                (0..N).map(|j| c.integer(i64::from(i == j))).collect(),
            )
        })
        .collect();
    let gram = (1..N)
        .map(|i| (1..N).map(|j| c.integer(i64::from(i == j))).collect())
        .collect();
    let family = IntegralFamily::new(
        format!("tadpole-with-{}-linear-auxiliaries", N - 1),
        vec!["k".into()],
        (1..N).map(|i| format!("p{i}")).collect(),
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        gram,
        vec![c.zero(); N],
    )
    .unwrap();
    (family, c)
}

fn powers<const N: usize>(first: i16) -> Vec<i16> {
    let mut powers = vec![0; N];
    powers[0] = first;
    powers
}

fn check_nonzero<const N: usize>() {
    let (family, c) = family::<N>();
    let cuts = CutConstraint::none(N).unwrap();
    let target = powers::<N>(2);
    let master = powers::<N>(1);
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let solution = solve_laporta_for::<N>(&family, &cuts, &[target.clone()], &[], options).unwrap();
    assert_eq!(solution.residuals, [master.clone()]);
    let rule = solution
        .rules
        .iter()
        .find(|rule| {
            rule.target
                .iter()
                .map(|p| p.value)
                .eq(target.iter().copied())
        })
        .unwrap();
    assert_eq!(rule.rhs.len(), 1);
    assert!(
        rule.rhs[0]
            .powers
            .iter()
            .map(|p| p.value)
            .eq(master.iter().copied())
    );
    // The Euler IBP gives T(2) = (d-2)/(2*m2) T(1), exactly over Q(d,m2).
    let expected = &(&c.parameter("d").unwrap() - &c.integer(2))
        / &(&c.integer(2) * &c.parameter("m2").unwrap());
    assert!(!expected.is_zero());
    assert!((&rule.rhs[0].coefficient - &expected).is_zero());
    let certificate = certify_laporta_for::<N>(&family, &cuts, &solution, false).unwrap();
    assert!(certificate.replayed_rules > 0 && certificate.identities > 0);

    let sector: Vec<_> = target.iter().map(|n| *n > 0).collect();
    let fixed: Vec<_> = target.iter().copied().map(Some).collect();
    let parametric = solve_parametric_for::<N>(&family, &cuts, &sector, &fixed, options).unwrap();
    assert_eq!(parametric.rules[0].rhs.len(), 1);
    assert!((&parametric.rules[0].rhs[0].coefficient - &expected).is_zero());

    // Keep runtime assertions valid for deliberately reduced custom registries.
    if rustred::compiled_runtime_arities().contains(&N) {
        let runtime = solve_laporta(&family, &cuts, &[target], &[], options).unwrap();
        assert_eq!(runtime.rules.len(), solution.rules.len());
        for (runtime, generic) in runtime.rules.iter().zip(&solution.rules) {
            assert_eq!(runtime.target, generic.target);
            assert_eq!(runtime.sector, generic.sector);
            assert_eq!(runtime.rhs, generic.rhs);
            assert_eq!(runtime.nonzero_conditions, generic.nonzero_conditions);
            assert_eq!(runtime.exceptions, generic.exceptions);
        }
        assert_eq!(runtime.residuals, solution.residuals);
        assert_eq!(
            certify_laporta(&family, &cuts, &runtime, false).unwrap(),
            certificate
        );
        let runtime_parametric =
            solve_parametric(&family, &cuts, &sector, &fixed, options).unwrap();
        assert_eq!(runtime_parametric.rules.len(), parametric.rules.len());
        for (runtime, generic) in runtime_parametric.rules.iter().zip(&parametric.rules) {
            assert_eq!(runtime.target, generic.target);
            assert_eq!(runtime.sector, generic.sector);
            assert_eq!(runtime.rhs, generic.rhs);
            assert_eq!(runtime.nonzero_conditions, generic.nonzero_conditions);
            assert_eq!(runtime.exceptions, generic.exceptions);
        }
    }
}

#[test]
fn exact_nonzero_13_slots() {
    check_nonzero::<13>();
}
#[test]
fn exact_nonzero_14_slots() {
    check_nonzero::<14>();
}
#[test]
fn exact_nonzero_15_slots() {
    check_nonzero::<15>();
}

#[test]
fn exact_generic_17_slots_outside_default_registry() {
    check_nonzero::<17>();
}

#[test]
fn generic_entry_points_reject_bad_n_before_other_invalid_input() {
    let (family, _) = family::<1>();
    let cuts = CutConstraint::none(1).unwrap();
    let options = DynamicSolveOptions::default();
    let solution = DynamicSolution::default();
    for error in [
        solve_laporta_for::<0>(&family, &cuts, &[], &[], options).unwrap_err(),
        solve_laporta_for::<2>(&family, &cuts, &[], &[], options).unwrap_err(),
        solve_parametric_for::<0>(&family, &cuts, &[], &[], options).unwrap_err(),
        solve_parametric_for::<2>(&family, &cuts, &[], &[], options).unwrap_err(),
        certify_laporta_for::<0>(&family, &cuts, &solution, false).unwrap_err(),
        certify_laporta_for::<2>(&family, &cuts, &solution, false).unwrap_err(),
    ] {
        assert!(matches!(error, SolverError::InvalidInput(_)));
        assert!(error.to_string().contains("const-generic arity"));
    }
}

#[test]
fn unsupported_runtime_error_reports_the_actual_compiled_registry() {
    // No N=17 native solver instantiation is needed for this capability check.
    // A custom build which includes 17 legitimately supports this family.
    if rustred::compiled_runtime_arities().contains(&17) {
        return;
    }
    let (family, _) = family::<17>();
    let cuts = CutConstraint::none(17).unwrap();
    let options = DynamicSolveOptions::default();
    for error in [
        solve_laporta(&family, &cuts, &[], &[], options).unwrap_err(),
        solve_parametric(&family, &cuts, &[], &[], options).unwrap_err(),
        certify_laporta(&family, &cuts, &DynamicSolution::default(), false).unwrap_err(),
    ] {
        assert!(matches!(error, SolverError::InvalidInput(_)));
        let message = error.to_string();
        assert!(message.contains(&format!("{:?}", rustred::compiled_runtime_arities())));
        assert!(message.contains("received 17") && message.contains("RUSTRED_RUNTIME_ARITIES"));
    }
}

#[test]
fn high_arity_preserves_cut_preferred_and_stability_options() {
    let (family, c) = family::<13>();
    let cuts = CutConstraint::try_new((0..13).map(|i| i == 0)).unwrap();
    let options = DynamicSolveOptions {
        max_depth: 2,
        until_stable: true,
        ..Default::default()
    };
    let first = powers::<13>(1);
    let second = powers::<13>(2);
    let outside = powers::<13>(0);
    let solution = solve_laporta_for::<13>(
        &family,
        &cuts,
        &[first.clone(), outside.clone()],
        &[second.clone()],
        options,
    )
    .unwrap();
    assert_eq!(solution.residuals, [second]);
    assert!(solution.basis_change.is_some());
    let rule = solution
        .rules
        .iter()
        .find(|r| r.target.iter().map(|p| p.value).eq(first.iter().copied()))
        .unwrap();
    let expected = &(&c.integer(2) * &c.parameter("m2").unwrap())
        / &(&c.parameter("d").unwrap() - &c.integer(2));
    assert_eq!(rule.rhs.len(), 1);
    assert!((&rule.rhs[0].coefficient - &expected).is_zero());
    assert!(
        solution
            .rules
            .iter()
            .find(|r| r.target.iter().map(|p| p.value).eq(outside.iter().copied()))
            .unwrap()
            .rhs
            .is_empty()
    );
    let certificate = certify_laporta_for::<13>(&family, &cuts, &solution, false).unwrap();
    assert!(certificate.zero_rules > 0 && certificate.replayed_rules > 0);
    // Preferred-basis poles are still explicit, not erased by the wrapper.
    assert!(
        solution
            .basis_change
            .unwrap()
            .conditions
            .iter()
            .any(|p| !p.is_constant())
    );
}
