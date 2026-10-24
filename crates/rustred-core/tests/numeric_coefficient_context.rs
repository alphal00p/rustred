//! Families whose coefficient context has NO parameters (rational dimension,
//! rational deformation) must solve exactly instead of panicking.
//!
//! Regression: lifting a base coefficient into the index-extended context
//! iterated Symbolica exponent chunks of width zero ("chunk size must be
//! non-zero") on the `solve_laporta -> SourceSystem::from_family ->
//! ParametricIbpGenerator::prepare_ordinary_ibp` path.
use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::bridge::{DynamicSolveOptions, certify_laporta_for, solve_laporta_for};

fn rational(context: &CoefficientContext, numerator: i64, denominator: i64) -> Coefficient {
    context
        .try_div(
            &context.integer(numerator),
            &context.integer(denominator),
            ExactAlgebraLimits::default(),
        )
        .unwrap()
}

/// `d = 4 - 2 eps` at the sampled `eps = 1/2700`, as RustFlow substitutes it.
fn sampled_dimension(context: &CoefficientContext) -> Coefficient {
    &context.integer(4) - &rational(context, 2, 2700)
}

#[test]
fn parameter_free_tadpole_reduces_exactly() {
    let context = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
    let dimension = sampled_dimension(&context);
    let family = IntegralFamily::new(
        "numeric-tadpole",
        vec!["k".into()],
        vec![],
        context.clone(),
        dimension.clone(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        vec![],
        vec![context.zero()],
    )
    .unwrap();
    let cuts = CutConstraint::none(1).unwrap();
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let solution = solve_laporta_for::<1>(&family, &cuts, &[vec![2]], &[], options).unwrap();
    assert_eq!(solution.residuals, [vec![1]]);
    let rule = solution
        .rules
        .iter()
        .find(|rule| rule.target.iter().map(|p| p.value).eq([2]))
        .unwrap();
    assert_eq!(rule.rhs.len(), 1);
    // T(2) = (d - 2)/2 T(1) = 2699/2700 T(1) at the sampled dimension.
    assert!((&rule.rhs[0].coefficient - &rational(&context, 2699, 2700)).is_zero());
    let certificate = certify_laporta_for::<1>(&family, &cuts, &solution, false).unwrap();
    assert!(certificate.replayed_rules > 0);
}

#[test]
fn parameter_free_deformed_sunrise_matches_the_symbolic_deformation() {
    // D0 = k1^2 - 1 - eta, D1 = k2^2 - 1, D2 = (k1 + k2)^2 - 1, with the
    // rational eta = 1/3 and sampled d: no context parameter remains.
    let build = |context: &CoefficientContext, eta: Coefficient, dimension: Coefficient| {
        let row = |values: [i64; 3]| values.map(|value| context.integer(value)).to_vec();
        IntegralFamily::new(
            "numeric-deformed-sunrise",
            vec!["k1".into(), "k2".into()],
            vec![],
            context.clone(),
            dimension,
            vec![
                AffineDenominator::new(&context.integer(-1) - &eta, row([1, 0, 0])),
                AffineDenominator::new(context.integer(-1), row([0, 0, 1])),
                AffineDenominator::new(context.integer(-1), row([1, 2, 1])),
            ],
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap()
    };
    let numeric = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
    let family = build(
        &numeric,
        rational(&numeric, 1, 3),
        sampled_dimension(&numeric),
    );
    let cuts = CutConstraint::none(3).unwrap();
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let targets = [vec![2, 1, 1], vec![1, 2, 1]];
    let solution = solve_laporta_for::<3>(&family, &cuts, &targets, &[], options).unwrap();
    assert!(!solution.residuals.is_empty());
    certify_laporta_for::<3>(&family, &cuts, &solution, false).unwrap();

    // The same search with a symbolic eta (the previously working lane) must
    // leave the same residual set.
    let symbolic = CoefficientContext::try_new(["eta"]).unwrap();
    let parametric = build(
        &symbolic,
        symbolic.parameter("eta").unwrap(),
        sampled_dimension(&symbolic),
    );
    let reference = solve_laporta_for::<3>(&parametric, &cuts, &targets, &[], options).unwrap();
    assert_eq!(solution.residuals, reference.residuals);
}

/// RustFlow's native P14 wheel (see `sector_symmetry_census.rs`): physical
/// lines `D0..D7`, ISPs `D8 = k0.k1`, `D9 = k0.k2`, over
/// (k0k0, k0k1, k0k2, k0k3, k1k1, k1k2, k1k3, k2k2, k2k3, k3k3).
const P14_SCALAR_PRODUCTS: [[i64; 10]; 10] = [
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [1, -2, 0, -2, 1, 0, 2, 0, 0, 1],
    [0, 0, 0, 0, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [1, -2, -2, -2, 1, 2, 2, 1, 2, 1],
    [0, 0, 0, 0, 1, 2, 0, 1, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
];
const P14_CONSTANTS: [i64; 10] = [-1, -1, -1, -1, -1, -1, -1, -1, 0, 0];

/// Rim-deformed P14 (`D0 -> D0 - eta`) in the given context.
fn p14_rim(
    context: &CoefficientContext,
    eta: Coefficient,
    dimension: Coefficient,
) -> IntegralFamily {
    let denominators = P14_SCALAR_PRODUCTS
        .iter()
        .zip(P14_CONSTANTS)
        .enumerate()
        .map(|(slot, (row, constant))| {
            let mut constant = context.integer(constant);
            if slot == 0 {
                constant = &constant - &eta;
            }
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    IntegralFamily::new(
        "p14-rim",
        (0..4).map(|index| format!("k{index}")).collect(),
        vec![],
        context.clone(),
        dimension,
        denominators,
        vec![],
        vec![context.zero(); 10],
    )
    .unwrap()
}

/// Non-trivial evidence for the zero-parameter fix on a real family: a P14
/// rim Laporta solve of a rank-1 numerator target at depth 3 gives the same
/// non-trivial rule and residual structure in a parameter-free context
/// (eps = 1/2700, eta = 1/3), with symbolic eta at sampled d, and with
/// symbolic (d, eta); every solution replays exactly.
#[test]
fn parameter_free_p14_rim_laporta_matches_symbolic_contexts() {
    let target = vec![1, 1, 1, 1, 1, 0, 0, 0, -1, 0];
    let cuts = CutConstraint::none(10).unwrap();
    let options = DynamicSolveOptions {
        max_depth: 3,
        ..Default::default()
    };
    let numeric = CoefficientContext::try_new(Vec::<String>::new()).unwrap();
    let eta_only = CoefficientContext::try_new(["eta"]).unwrap();
    let symbolic = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let families = [
        p14_rim(
            &numeric,
            rational(&numeric, 1, 3),
            sampled_dimension(&numeric),
        ),
        p14_rim(
            &eta_only,
            eta_only.parameter("eta").unwrap(),
            sampled_dimension(&eta_only),
        ),
        p14_rim(
            &symbolic,
            symbolic.parameter("eta").unwrap(),
            symbolic.parameter("d").unwrap(),
        ),
    ];
    let structure = |solution: &rustred::solver::bridge::DynamicSolution| {
        let mut rules: Vec<_> = solution
            .rules
            .iter()
            .map(|rule| {
                let mut rhs: Vec<_> = rule.rhs.iter().map(|term| term.powers.clone()).collect();
                rhs.sort();
                (rule.target.clone(), rhs)
            })
            .collect();
        rules.sort();
        (rules, solution.residuals.clone())
    };
    let mut reference = None;
    for family in &families {
        let solution =
            solve_laporta_for::<10>(family, &cuts, std::slice::from_ref(&target), &[], options)
                .unwrap();
        assert!(solution.rules.len() >= 50, "{}", solution.rules.len());
        assert!(solution.residuals.len() >= 2);
        let certificate = certify_laporta_for::<10>(family, &cuts, &solution, false).unwrap();
        assert!(certificate.replayed_rules > 0);
        let structure = structure(&solution);
        match &reference {
            None => reference = Some(structure),
            Some(reference) => assert_eq!(&structure, reference),
        }
    }
}
