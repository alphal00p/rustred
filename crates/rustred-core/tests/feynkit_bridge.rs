//! Exact multiscale regressions for the runtime bridge used by community.hep.
use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SourceSystem;
use rustred::solver::bridge::{
    DynamicSolution, DynamicSolveOptions, PreferredStatus, certify_laporta, solve_laporta,
    solve_parametric,
};
use std::collections::BTreeMap;

fn family(vertex: bool) -> IntegralFamily {
    let names = if vertex {
        vec!["d", "m1", "m2", "m3", "s1", "s2", "s3"]
    } else {
        vec!["d", "m1", "m2", "m3", "s"]
    };
    let c = CoefficientContext::try_new(names).unwrap();
    let p = |name| c.parameter(name).unwrap();
    let gram = if vertex {
        let cross = &(&(&p("s3") - &p("s1")) - &p("s2")) / &c.integer(2);
        vec![vec![p("s1"), cross.clone()], vec![cross, p("s2")]]
    } else {
        vec![vec![p("s")]]
    };
    let routes: Vec<(Vec<i64>, &str)> = if vertex {
        vec![
            (vec![1, 0, 0, 0], "m1"),
            (vec![0, 1, 0, 0], "m2"),
            (vec![1, -1, 0, 0], "m3"),
            (vec![1, 0, -1, 0], "m1"),
            (vec![1, 0, -1, -1], "m1"),
            (vec![0, 1, -1, 0], "m2"),
            (vec![0, 1, -1, -1], "m2"),
        ]
    } else {
        vec![
            (vec![1, 0, 0], "m1"),
            (vec![0, 1, 0], "m2"),
            (vec![1, -1, 0], "m3"),
            (vec![1, 0, -1], "m1"),
            (vec![0, 1, -1], "m2"),
        ]
    };
    let denominators = routes
        .iter()
        .map(|(route, mass)| {
            let mut constant = -p(mass);
            for a in 0..gram.len() {
                for b in 0..gram.len() {
                    constant = &constant + &(&c.integer(route[2 + a] * route[2 + b]) * &gram[a][b]);
                }
            }
            let mut coefficients = vec![
                c.integer(route[0] * route[0]),
                c.integer(2 * route[0] * route[1]),
                c.integer(route[1] * route[1]),
            ];
            for k in 0..2 {
                for e in 0..gram.len() {
                    coefficients.push(c.integer(2 * route[k] * route[2 + e]));
                }
            }
            AffineDenominator::new(constant, coefficients)
        })
        .collect();
    IntegralFamily::new(
        "multiscale",
        vec!["k1".into(), "k2".into()],
        (0..gram.len()).map(|e| format!("p{e}")).collect(),
        c.clone(),
        p("d"),
        denominators,
        gram,
        vec![c.zero(); routes.len()],
    )
    .unwrap()
}

fn uncut(family: &IntegralFamily) -> CutConstraint {
    CutConstraint::none(family.denominator_count()).unwrap()
}

fn contains_parameter(value: &Coefficient, name: &str) -> bool {
    value
        .numerator
        .variables()
        .iter()
        .enumerate()
        .any(|(index, variable)| {
            variable.to_string().ends_with(name)
                && (value.numerator.contains(index) || value.denominator.contains(index))
        })
}

#[test]
fn two_loop_multiscale_propagator_parametric_and_laporta() {
    check(false);
}

#[test]
fn two_loop_multiscale_vertex_parametric_and_laporta() {
    check(true);
}

fn check(vertex: bool) {
    let family = family(vertex);
    if vertex {
        check_euler_identity::<7>(&family, &[(3, "s1"), (4, "s3")]);
    } else {
        check_euler_identity::<5>(&family, &[(3, "s")]);
    }
    let n = family.denominator_count();
    let options = DynamicSolveOptions {
        max_depth: 1,
        include_lorentz: false,
        ..Default::default()
    };
    let parametric = solve_parametric(
        &family,
        &uncut(&family),
        &vec![true; n],
        &vec![None; n],
        options,
    )
    .unwrap();
    assert_eq!(parametric.rules.len(), 1);
    let rule = &parametric.rules[0];
    assert!(!rule.rhs.is_empty());
    assert!(
        rule.target
            .iter()
            .all(|power| power.symbolic && power.value == 0)
    );
    assert!(
        !rule.nonzero_conditions.is_empty(),
        "generic pivot divisions must retain their guards"
    );
    assert!(
        rule.rhs
            .iter()
            .any(|term| contains_parameter(&term.coefficient, "m1")
                || contains_parameter(&term.coefficient, "m2")
                || contains_parameter(&term.coefficient, "m3"))
    );

    // A pinched graph is a product of two independent massive tadpoles. Its
    // exact two-scale recurrence follows separately by radial integration:
    // I(2,1,0,...) = (d-2)/(2*m1) I(1,1,0,...).
    let mut target = vec![0; n];
    target[0] = 2;
    target[1] = 1;
    let mut master = target.clone();
    master[0] = 1;
    let result = solve_laporta(
        &family,
        &uncut(&family),
        &[target.clone(), master.clone()],
        &[],
        options,
    )
    .unwrap();
    let reduced = result
        .rules
        .iter()
        .find(|rule| {
            rule.target
                .iter()
                .map(|p| p.value)
                .eq(target.iter().copied())
        })
        .unwrap();
    assert_eq!(reduced.rhs.len(), 1);
    assert!(
        reduced.rhs[0]
            .powers
            .iter()
            .map(|p| p.value)
            .eq(master.iter().copied())
    );
    let c = family.coefficient_context();
    let expected = &(&c.parameter("d").unwrap() - &c.integer(2))
        / &(&c.integer(2) * &c.parameter("m1").unwrap());
    // Coefficient maps include zero-degree index slots on the bridge side;
    // native arithmetic unifies them before the exact zero comparison.
    assert!((&reduced.rhs[0].coefficient - &expected).is_zero());
    assert_eq!(result.residuals, vec![master]);

    // A dotted integral of the connected graph exercises elimination with
    // every physical line present and kinematics kept as independent scales.
    let mut connected = vec![1; n];
    connected[0] = 2;
    // A two-loop vertex has six physical lines. Its auxiliary (k2-p1)^2-m2
    // appears once in the numerator; ∂k1·k2 supplies a direct reduction of
    // this numerator with the fourth physical propagator doubled.
    if vertex {
        connected[0] = 1;
        connected[3] = 2;
        connected[5] = -1;
    }
    let mut corner = vec![1; n];
    if vertex {
        corner[5] = 0;
    }
    let connected_result = solve_laporta(
        &family,
        &uncut(&family),
        &[corner, connected.clone()],
        &[],
        DynamicSolveOptions {
            max_depth: 0,
            max_targets: 1024,
            ..options
        },
    )
    .unwrap();
    let connected_rule = connected_result
        .rules
        .iter()
        .find(|rule| {
            rule.target
                .iter()
                .map(|power| power.value)
                .eq(connected.iter().copied())
        })
        .expect("connected dotted target must reduce");
    assert!(!connected_rule.rhs.is_empty());
    assert!(
        connected_rule
            .rhs
            .iter()
            .any(|term| contains_parameter(&term.coefficient, if vertex { "s3" } else { "s" }))
    );
    assert!(
        connected_rule
            .rhs
            .iter()
            .any(|term| contains_parameter(&term.coefficient, "m1"))
    );
}

/// Independent momentum-space oracle for ∂/∂k1 · k1. In particular,
/// k1·∂D3 = D1-D2+D3+m1-m2+m3 and
/// k1·∂(k1-P)^2 = D1+D_P+2*m1-P². This checks all shifts,
/// dimension signs, mass terms and external invariants before solving.
fn check_euler_identity<const N: usize>(family: &IntegralFamily, shifted: &[(usize, &str)]) {
    let sources = SourceSystem::<N>::from_family_with_lorentz(family, false).unwrap();
    let template = &sources.rows()[0][0].coefficient;
    let a = |i: usize| -> Coefficient {
        template
            .variable(&template.variables()[sources.index_variables()[i]])
            .unwrap()
            .into()
    };
    let c = family.coefficient_context();
    let p = |name| c.parameter(name).unwrap();
    let mut expected = BTreeMap::<Vec<i16>, Coefficient>::new();
    let mut add = |shift: Vec<i16>, value: Coefficient| {
        expected
            .entry(shift)
            .and_modify(|old| *old = &*old + &value)
            .or_insert(value);
    };
    let shift = |raised: usize, lowered: Option<usize>| {
        let mut values = vec![0; N];
        values[raised] += 1;
        if let Some(lowered) = lowered {
            values[lowered] -= 1;
        }
        values
    };
    let diagonal = &(&p("d") - &(&c.integer(2) * &a(0))) - &a(2);
    let diagonal = shifted
        .iter()
        .fold(diagonal, |total, (i, _)| &total - &a(*i));
    add(vec![0; N], diagonal);
    add(shift(0, None), -(&(&c.integer(2) * &p("m1")) * &a(0)));
    add(shift(2, Some(0)), -a(2));
    add(shift(2, Some(1)), a(2));
    add(
        shift(2, None),
        -(&(&(&p("m1") - &p("m2")) + &p("m3")) * &a(2)),
    );
    for &(i, invariant) in shifted {
        add(shift(i, Some(0)), -a(i));
        add(
            shift(i, None),
            -(&(&(&c.integer(2) * &p("m1")) - &p(invariant)) * &a(i)),
        );
    }
    // SourceSystem visits external contractions first, then k1 and k2.
    let actual: BTreeMap<_, Coefficient> = sources.rows()[family.external_count()]
        .iter()
        .map(|term| {
            (
                term.integral
                    .powers()
                    .iter()
                    .map(|power| power.value())
                    .collect::<Vec<_>>(),
                term.coefficient.clone().into(),
            )
        })
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (shift, value) in expected {
        assert!(
            (&actual[&shift] - &value).is_zero(),
            "Euler IBP differs at {shift:?}"
        );
    }
}

#[test]
fn zero_sectors_duplicates_and_invalid_inputs_are_explicit() {
    let family = family(false);
    let zero = vec![1, 0, 0, 0, 0];
    let result = solve_laporta(
        &family,
        &uncut(&family),
        &[zero.clone(), zero],
        &[],
        DynamicSolveOptions::default(),
    )
    .unwrap();
    assert_eq!(result.rules.len(), 1);
    assert!(result.rules.iter().all(|rule| rule.rhs.is_empty()));
    assert!(result.residuals.is_empty());
    assert!(
        solve_laporta(
            &family,
            &uncut(&family),
            &[vec![1, 1]],
            &[],
            DynamicSolveOptions::default()
        )
        .is_err()
    );
    assert!(
        solve_parametric(
            &family,
            &uncut(&family),
            &[true; 5],
            &[Some(0); 5],
            DynamicSolveOptions::default()
        )
        .is_err()
    );
    assert!(
        solve_laporta(
            &family,
            &uncut(&family),
            &[vec![2, 1, 0, 0, 0]],
            &[],
            DynamicSolveOptions {
                max_targets: 1,
                ..Default::default()
            }
        )
        .is_err(),
        "RHS closure must fail explicitly when its integral budget is exhausted"
    );
}

#[test]
fn equal_mass_sunset_matches_feyncalc_kira_reductions() {
    let c = CoefficientContext::try_new(["d", "M"]).unwrap();
    let mass = c.parameter("M").unwrap();
    let family = IntegralFamily::new(
        "equal-mass-sunset",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(-mass.clone(), vec![c.one(), c.zero(), c.zero()]),
            AffineDenominator::new(-mass.clone(), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(-mass.clone(), vec![c.one(), c.integer(2), c.one()]),
        ],
        Vec::new(),
        vec![c.zero(); 3],
    )
    .unwrap();
    let targets = vec![vec![2, 1, 0], vec![1, 1, 1], vec![1, 1, 2], vec![1, 1, 3]];
    let result = solve_laporta(
        &family,
        &uncut(&family),
        &targets,
        &[],
        DynamicSolveOptions::default(),
    )
    .unwrap();
    let d = c.parameter("d").unwrap();
    let d2 = &d - &c.integer(2);
    let d3 = &d - &c.integer(3);
    let d8 = &d - &c.integer(8);
    let mass2 = &mass * &mass;
    let mass3 = &mass2 * &mass;
    let expected = [
        (&d2 / &(&c.integer(2) * &mass), c.zero()),
        (c.zero(), &d3 / &(&c.integer(3) * &mass)),
        (
            &(&d2 * &d2) / &(&c.integer(12) * &mass3),
            &(&d8 * &d3) / &(&c.integer(18) * &mass2),
        ),
    ];
    for (target, (bubble, sunset)) in [targets[0].clone(), targets[2].clone(), targets[3].clone()]
        .into_iter()
        .zip(expected)
    {
        let rule = result
            .rules
            .iter()
            .find(|rule| {
                rule.target
                    .iter()
                    .map(|p| p.value)
                    .eq(target.iter().copied())
            })
            .unwrap();
        let mut coefficients = BTreeMap::<Vec<i16>, Coefficient>::new();
        for term in &rule.rhs {
            // Equal masses admit all line permutations, independently by
            // unimodular reroutings of the two integration momenta.
            let mut powers: Vec<_> = term.powers.iter().map(|p| p.value).collect();
            powers.sort();
            coefficients
                .entry(powers)
                .and_modify(|old| *old = &*old + &term.coefficient)
                .or_insert(term.coefficient.clone());
        }
        let actual_bubble = coefficients
            .remove(&vec![0, 1, 1])
            .unwrap_or_else(|| c.zero());
        let actual_sunset = coefficients
            .remove(&vec![1, 1, 1])
            .unwrap_or_else(|| c.zero());
        assert!(
            coefficients.is_empty(),
            "unexpected sunset residuals {coefficients:?}"
        );
        assert!(
            (&actual_bubble - &bubble).is_zero(),
            "bubble coefficient differs for {target:?}"
        );
        assert!(
            (&actual_sunset - &sunset).is_zero(),
            "sunset coefficient differs for {target:?}"
        );
    }
}

/// Bubble `[k.k - m2, (k-p).(k-p)]` with `p.p = s` in the bridge's own
/// coordinates; `massive = false` sets `m2 = 0` and `on_shell` sets `s = 0`, so
/// the massless on-shell bubble is scaleless.
fn bubble(massive: bool, on_shell: bool) -> (IntegralFamily, CoefficientContext) {
    let c = CoefficientContext::try_new(vec!["d", "m2", "s"]).unwrap();
    let p = |name| c.parameter(name).unwrap();
    let s = if on_shell { c.zero() } else { p("s") };
    let mass = if massive { -p("m2") } else { c.zero() };
    let family = IntegralFamily::new(
        "bubble",
        vec!["k".into()],
        vec!["p".into()],
        c.clone(),
        p("d"),
        vec![
            AffineDenominator::new(mass, vec![c.integer(1), c.integer(0)]),
            AffineDenominator::new(s.clone(), vec![c.integer(1), c.integer(-2)]),
        ],
        vec![vec![s]],
        vec![c.zero(); 2],
    )
    .unwrap();
    (family, c)
}

fn outside_cut(powers: &[i16], cut: &[bool]) -> bool {
    powers
        .iter()
        .zip(cut)
        .any(|(&power, &cut)| cut && power <= 0)
}

fn reductions(solution: &DynamicSolution) -> BTreeMap<Vec<i16>, BTreeMap<Vec<i16>, Coefficient>> {
    solution
        .rules
        .iter()
        .map(|rule| {
            let target = rule.target.iter().map(|power| power.value).collect();
            let rhs = rule
                .rhs
                .iter()
                .map(|term| {
                    let powers = term.powers.iter().map(|power| power.value).collect();
                    (powers, term.coefficient.clone())
                })
                .collect();
            (target, rhs)
        })
        .collect()
}

fn assert_same_terms(
    actual: &BTreeMap<Vec<i16>, Coefficient>,
    expected: &BTreeMap<Vec<i16>, Coefficient>,
    label: &str,
) {
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "{label}"
    );
    for (powers, coefficient) in actual {
        assert!(
            (coefficient - &expected[powers]).is_zero(),
            "{label}: coefficient of {powers:?}"
        );
    }
}

/// A cut keeps the uncut integral order, so its reduction is the uncut one
/// with every integral outside the cut removed, and so is its residual basis.
fn assert_cut_projects_uncut(
    family: &IntegralFamily,
    cut: &[bool],
    targets: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> DynamicSolution {
    let uncut_solution = solve_laporta(family, &uncut(family), targets, &[], options).unwrap();
    let cuts = CutConstraint::try_new(cut.iter().copied()).unwrap();
    let solution = solve_laporta(family, &cuts, targets, &[], options).unwrap();
    let uncut_rules = reductions(&uncut_solution);
    let rules = reductions(&solution);
    for target in targets {
        assert!(
            rules.contains_key(target) || solution.residuals.contains(target),
            "requested {target:?} is neither solved nor a residual with cut {cut:?}"
        );
    }
    for (target, rhs) in rules {
        let projected = if outside_cut(&target, cut) {
            BTreeMap::new()
        } else {
            uncut_rules[&target]
                .iter()
                .filter(|(powers, _)| !outside_cut(powers, cut))
                .map(|(powers, coefficient)| (powers.clone(), coefficient.clone()))
                .collect()
        };
        assert_same_terms(&rhs, &projected, &format!("{target:?} with cut {cut:?}"));
    }
    let residuals: Vec<_> = uncut_solution
        .residuals
        .into_iter()
        .filter(|powers| !outside_cut(powers, cut))
        .collect();
    assert_eq!(solution.residuals, residuals);
    solution
}

#[test]
fn reverse_unitarity_cuts_project_uncut_reductions() {
    let (massive, c) = bubble(true, false);
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let targets = [vec![1, 1], vec![2, 1], vec![1, 0]];
    let solution = assert_cut_projects_uncut(&massive, &[true, true], &targets, options);
    // I(2,1) = (d-3)/(m2-s) I(1,1); the cut removes the tadpole I(1,0).
    let p = |name| c.parameter(name).unwrap();
    let master = &(&p("d") - &c.integer(3)) / &(&p("m2") - &p("s"));
    let rules = reductions(&solution);
    assert_same_terms(
        &rules[&vec![2, 1]],
        &BTreeMap::from([(vec![1, 1], master)]),
        "two-cut bubble",
    );
    assert_eq!(solution.residuals, [vec![1, 1]]);
    let tadpole = solution
        .rules
        .iter()
        .find(|rule| rule.target.iter().map(|power| power.value).eq([1, 0]))
        .unwrap();
    assert!(tadpole.rhs.is_empty() && tadpole.nonzero_conditions.is_empty());
    // The rule does not depend on requesting the integral outside the cut.
    let alone = solve_laporta(
        &massive,
        &CutConstraint::try_new([true, true]).unwrap(),
        &[vec![2, 1]],
        &[],
        options,
    )
    .unwrap();
    assert_same_terms(
        &reductions(&alone)[&vec![2, 1]],
        &rules[&vec![2, 1]],
        "single target",
    );
    // A single cut on the massless line also removes the tadpole.
    assert_cut_projects_uncut(&massive, &[false, true], &targets, options);

    let two_loop = family(false);
    let targets = [
        vec![2, 1, 1, 0, 0],
        vec![1, 1, 1, -1, 0],
        vec![1, 2, 1, 0, -1],
    ];
    for include_lorentz in [false, true] {
        let options = DynamicSolveOptions {
            max_depth: 2,
            include_lorentz,
            ..Default::default()
        };
        let solution = assert_cut_projects_uncut(
            &two_loop,
            &[true, true, true, false, false],
            &targets,
            options,
        );
        assert_eq!(solution.residuals, [vec![1, 1, 1, 0, 0]]);
    }

    // Scaleless proofs still apply in sectors that contain every cut.
    let (scaleless, _) = bubble(false, true);
    let solution = solve_laporta(
        &scaleless,
        &CutConstraint::try_new([true, true]).unwrap(),
        &[vec![1, 1], vec![2, 1]],
        &[],
        options,
    )
    .unwrap();
    assert!(solution.rules.iter().all(|rule| rule.rhs.is_empty()));
    assert!(solution.residuals.is_empty());
}

#[test]
fn reverse_unitarity_parametric_rules_and_input_guards() {
    let (massive, c) = bubble(true, false);
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let cut = CutConstraint::try_new([true, true]).unwrap();
    let excluded =
        solve_parametric(&massive, &cut, &[true, false], &[None, None], options).unwrap();
    assert_eq!(excluded.rules.len(), 1);
    assert!(excluded.rules[0].rhs.is_empty() && excluded.rules[0].nonzero_conditions.is_empty());

    // Fixing the cut index drops its boundary term I(n1,0) during discovery.
    let fixed = [None, Some(1)];
    let plain =
        solve_parametric(&massive, &uncut(&massive), &[true, true], &fixed, options).unwrap();
    let with_cut = solve_parametric(&massive, &cut, &[true, true], &fixed, options).unwrap();
    let boundary = |term: &rustred::solver::bridge::DynamicTerm| {
        !term.powers[1].symbolic && term.powers[1].value <= 0
    };
    assert!(plain.rules[0].rhs.iter().any(boundary));
    assert!(!with_cut.rules[0].rhs.iter().any(boundary));
    let projected: Vec<_> = plain.rules[0]
        .rhs
        .iter()
        .filter(|term| !boundary(term))
        .cloned()
        .collect();
    assert_eq!(with_cut.rules[0].rhs, projected);
    // A symbolic cut index cannot be pruned: the rule is the uncut one.
    let symbolic = [None, None];
    let plain = solve_parametric(
        &massive,
        &uncut(&massive),
        &[true, true],
        &symbolic,
        options,
    )
    .unwrap();
    let with_cut = solve_parametric(&massive, &cut, &[true, true], &symbolic, options).unwrap();
    assert_eq!(with_cut.rules[0].rhs, plain.rules[0].rhs);

    let wrong_arity = CutConstraint::none(3).unwrap();
    assert!(solve_laporta(&massive, &wrong_arity, &[vec![1, 1]], &[], options).is_err());
    assert!(solve_parametric(&massive, &wrong_arity, &[true, true], &symbolic, options).is_err());

    // Like the solver's cut preparation, cuts are not combined with power shifts.
    let half = &c.integer(1) / &c.integer(2);
    let shifted = IntegralFamily::new(
        "shifted-bubble",
        vec!["k".into()],
        vec!["p".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        massive.denominators().to_vec(),
        vec![vec![c.parameter("s").unwrap()]],
        vec![c.zero(), half],
    )
    .unwrap();
    let only_first = CutConstraint::try_new([true, false]).unwrap();
    // The shift alone is still supported.
    solve_laporta(&shifted, &uncut(&shifted), &[vec![2, 1]], &[], options).unwrap();
    for error in [
        solve_laporta(&shifted, &only_first, &[vec![1, 1]], &[], options).unwrap_err(),
        solve_parametric(&shifted, &only_first, &[true, true], &symbolic, options).unwrap_err(),
    ] {
        assert!(error.to_string().contains("power shifts"), "{error}");
    }
}

/// Massive tadpole `k.k - m2`.
fn tadpole() -> (IntegralFamily, CoefficientContext) {
    let c = CoefficientContext::try_new(vec!["d", "m2"]).unwrap();
    let family = IntegralFamily::new(
        "tadpole",
        vec!["k".into()],
        Vec::new(),
        c.clone(),
        c.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            -c.parameter("m2").unwrap(),
            vec![c.integer(1)],
        )],
        Vec::new(),
        vec![c.zero()],
    )
    .unwrap();
    (family, c)
}

/// Massless triangle `[k.k, (k-p1).(k-p1), (k-p1-p2).(k-p1-p2)]` with light-like
/// legs and `(p1+p2)^2 = s`; its top sector reduces to the `s` bubble.
fn light_like_triangle() -> IntegralFamily {
    let c = CoefficientContext::try_new(vec!["d", "s"]).unwrap();
    let s = c.parameter("s").unwrap();
    let half = &s / &c.integer(2);
    let integer = |values: [i64; 3]| values.map(|value| c.integer(value)).to_vec();
    IntegralFamily::new(
        "triangle",
        vec!["k".into()],
        vec!["p1".into(), "p2".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(c.zero(), integer([1, 0, 0])),
            AffineDenominator::new(c.zero(), integer([1, -2, 0])),
            AffineDenominator::new(s.clone(), integer([1, -2, -2])),
        ],
        vec![vec![c.zero(), half.clone()], vec![half, c.zero()]],
        vec![c.zero(); 3],
    )
    .unwrap()
}

#[test]
fn preferred_masters_change_the_basis_exactly() {
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let (tadpole, c) = tadpole();
    let solution = solve_laporta(
        &tadpole,
        &uncut(&tadpole),
        &[vec![1], vec![3]],
        &[vec![2]],
        options,
    )
    .unwrap();
    assert_eq!(solution.residuals, [vec![2]]);
    // T(1) = 2 m2/(d-2) T(2), divided by the pivot d-2.
    let p = |name| c.parameter(name).unwrap();
    let d2 = &p("d") - &c.integer(2);
    assert_same_terms(
        &reductions(&solution)[&vec![1]],
        &BTreeMap::from([(vec![2], &(&c.integer(2) * &p("m2")) / &d2)]),
        "tadpole",
    );
    let change = solution.basis_change.as_ref().unwrap();
    assert_eq!(change.replaced, [vec![1]]);
    assert_eq!(change.preferred[0].status, PreferredStatus::Replaced);
    assert!(change.conditions.iter().any(|condition| {
        (&Coefficient::from(condition.clone()) - &d2).is_zero()
            || (&Coefficient::from(condition.clone()) + &d2).is_zero()
    }));
    let one_rule = solution
        .rules
        .iter()
        .find(|rule| rule.target[0].value == 1)
        .unwrap();
    assert!(
        one_rule
            .nonzero_conditions
            .iter()
            .any(|condition| !condition.is_constant() && change.conditions.contains(condition))
    );

    // Independently map the preferred master back through the default search.
    let (bubble, _) = bubble(true, false);
    let targets = [vec![1, 1], vec![2, 1], vec![1, 2], vec![1, 0]];
    let preferred = [vec![2, 1]];
    let plain = solve_laporta(
        &bubble,
        &uncut(&bubble),
        &[&targets[..], &preferred].concat(),
        &[],
        options,
    )
    .unwrap();
    let changed = solve_laporta(&bubble, &uncut(&bubble), &targets, &preferred, options).unwrap();
    assert!(changed.residuals.contains(&vec![2, 1]) && !changed.residuals.contains(&vec![1, 1]));
    let plain_rules = reductions(&plain);
    let back = &plain_rules[&vec![2, 1]];
    for (target, rhs) in reductions(&changed) {
        let mut mapped = BTreeMap::<Vec<i16>, Coefficient>::new();
        for (powers, coefficient) in rhs {
            let terms = if powers == preferred[0] {
                back.clone()
            } else {
                BTreeMap::from([(powers, c.integer(1))])
            };
            for (inner, factor) in terms {
                let value = &coefficient * &factor;
                let sum = match mapped.remove(&inner) {
                    Some(old) => &old + &value,
                    None => value,
                };
                if !sum.is_zero() {
                    mapped.insert(inner, sum);
                }
            }
        }
        let expected = plain_rules
            .get(&target)
            .cloned()
            .unwrap_or_else(|| BTreeMap::from([(target.clone(), c.integer(1))]));
        assert_same_terms(&mapped, &expected, &format!("{target:?} mapped back"));
    }

    // A preferred residual of the search is kept as is.
    let kept = solve_laporta(&bubble, &uncut(&bubble), &targets, &[vec![1, 1]], options).unwrap();
    assert_eq!(
        kept.basis_change.unwrap().preferred[0].status,
        PreferredStatus::Residual
    );

    let error = |family: &IntegralFamily, targets: &[Vec<i16>], preferred: &[Vec<i16>]| {
        solve_laporta(family, &uncut(family), targets, preferred, options)
            .unwrap_err()
            .to_string()
    };
    assert!(error(&tadpole, &[vec![1]], &[vec![2], vec![2]]).contains("listed twice"));
    assert!(error(&tadpole, &[vec![1]], &[vec![0]]).contains("zero sector"));
    assert!(error(&tadpole, &[vec![1]], &[vec![1, 1]]).contains("coordinates"));
    assert!(
        error(&bubble, &targets, &[vec![2, 1], vec![1, 2]]).contains("depends on preferred master")
    );
    let triangle = light_like_triangle();
    let deeper = DynamicSolveOptions {
        max_depth: 2,
        ..Default::default()
    };
    let lower = solve_laporta(
        &triangle,
        &uncut(&triangle),
        &[vec![1, 1, 1]],
        &[vec![1, 1, 1]],
        deeper,
    );
    let message = lower.unwrap_err().to_string();
    assert!(message.contains("lower-sector integrals"), "{message}");
    let cut = CutConstraint::try_new([true, true]).unwrap();
    let outside = solve_laporta(&bubble, &cut, &targets, &[vec![1, 0]], options).unwrap_err();
    assert!(outside.to_string().contains("outside the cut"));
}

#[test]
fn laporta_certificate_replays_and_rejects_tampering() {
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let (bubble, c) = bubble(true, false);
    let targets = [vec![1, 1], vec![2, 1], vec![1, 2], vec![1, 0], vec![2, 0]];
    let plain = solve_laporta(&bubble, &uncut(&bubble), &targets, &[], options).unwrap();
    let certificate = certify_laporta(&bubble, &uncut(&bubble), &plain, false).unwrap();
    assert!(certificate.replayed_rules > 0 && certificate.identities > 0);
    assert_eq!(certificate.returned_rules, plain.rules.len());

    let preferred =
        solve_laporta(&bubble, &uncut(&bubble), &targets, &[vec![2, 1]], options).unwrap();
    certify_laporta(&bubble, &uncut(&bubble), &preferred, false).unwrap();
    let cut = CutConstraint::try_new([true, true]).unwrap();
    let cut_solution = solve_laporta(&bubble, &cut, &targets, &[], options).unwrap();
    certify_laporta(&bubble, &cut, &cut_solution, false).unwrap();
    // A certificate must use the solution's own cut.
    assert!(certify_laporta(&bubble, &uncut(&bubble), &cut_solution, false).is_err());

    let two_loop = family(false);
    let lorentz = DynamicSolveOptions {
        max_depth: 1,
        include_lorentz: true,
        ..Default::default()
    };
    let solution = solve_laporta(
        &two_loop,
        &uncut(&two_loop),
        &[vec![2, 1, 1, 0, 0]],
        &[],
        lorentz,
    )
    .unwrap();
    certify_laporta(&two_loop, &uncut(&two_loop), &solution, true).unwrap();

    let doubled = |coefficient: &Coefficient| coefficient * &c.integer(2);
    let mut returned = plain.clone();
    let rule = returned
        .rules
        .iter_mut()
        .find(|rule| !rule.rhs.is_empty())
        .unwrap();
    rule.rhs[0].coefficient = doubled(&rule.rhs[0].coefficient);
    assert!(certify_laporta(&bubble, &uncut(&bubble), &returned, false).is_err());
    let mut derived = plain.clone();
    let rule = derived
        .derivation
        .iter_mut()
        .find(|derived| !derived.rule.rhs.is_empty())
        .unwrap();
    rule.rule.rhs[0].coefficient = doubled(&rule.rule.rhs[0].coefficient);
    assert!(certify_laporta(&bubble, &uncut(&bubble), &derived, false).is_err());
    let mut missing = plain.clone();
    missing
        .derivation
        .retain(|derived| derived.rule.rhs.is_empty());
    assert!(certify_laporta(&bubble, &uncut(&bubble), &missing, false).is_err());
}

#[test]
fn laporta_certificate_rejects_non_numeric_and_malformed_public_keys() {
    let (family, _) = bubble(true, false);
    let cuts = uncut(&family);
    let solution = solve_laporta(
        &family,
        &cuts,
        &[vec![2, 1], vec![1, 2]],
        &[],
        DynamicSolveOptions {
            max_depth: 1,
            ..Default::default()
        },
    )
    .unwrap();
    certify_laporta(&family, &cuts, &solution, false).unwrap();
    let mutations: &[fn(&mut DynamicSolution)] = &[
        |s| s.rules[0].target[0].symbolic = true,
        |s| {
            s.rules.iter_mut().find(|r| !r.rhs.is_empty()).unwrap().rhs[0].powers[0].symbolic = true
        },
        |s| {
            s.rules[0].target.pop();
        },
        |s| {
            s.rules.iter_mut().find(|r| !r.rhs.is_empty()).unwrap().rhs[0]
                .powers
                .pop();
        },
        |s| s.rules[0].target[0].value = 64,
        |s| s.rules[0].sector[0] = !s.rules[0].sector[0],
        |s| {
            s.rules[0].sector.pop();
        },
        |s| s.derivation[0].rule.target[0].symbolic = true,
        |s| {
            s.residuals[0].pop();
        },
        |s| s.residuals[0][0] = -65,
        |s| {
            s.requested[0].pop();
        },
        |s| {
            let seeds = s
                .derivation
                .iter_mut()
                .find_map(|d| match &mut d.origin {
                    rustred::solver::bridge::RuleOrigin::Identities { seeds } => Some(seeds),
                    _ => None,
                })
                .unwrap();
            seeds[0].pop();
        },
        |s| {
            let seeds = s
                .derivation
                .iter_mut()
                .find_map(|d| match &mut d.origin {
                    rustred::solver::bridge::RuleOrigin::Identities { seeds } => Some(seeds),
                    _ => None,
                })
                .unwrap();
            // This seed itself is compact, but its original IBP has a +1
            // endpoint. Refuse it before the order's compact-key constructor.
            seeds[0][0] = 63;
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut tampered = solution.clone();
        mutate(&mut tampered);
        assert!(
            matches!(
                certify_laporta(&family, &cuts, &tampered, false),
                Err(rustred::solver::SolverError::Certification(_))
            ),
            "mutation {index}"
        );
    }
}

#[test]
fn laporta_certificate_validates_all_basis_change_keys() {
    let (family, _) = bubble(true, false);
    let cuts = uncut(&family);
    let solution = solve_laporta(
        &family,
        &cuts,
        &[vec![2, 1], vec![1, 2]],
        &[vec![2, 1]],
        DynamicSolveOptions {
            max_depth: 1,
            ..Default::default()
        },
    )
    .unwrap();
    certify_laporta(&family, &cuts, &solution, false).unwrap();
    assert!(!solution.basis_change.as_ref().unwrap().original.is_empty());
    let mutations: &[fn(&mut rustred::solver::bridge::BasisChange)] = &[
        |c| c.original[0].target[0].symbolic = true,
        |c| {
            c.original[0].rhs[0].powers.pop();
        },
        |c| c.original[0].sector[0] = !c.original[0].sector[0],
        |c| {
            c.replaced[0].pop();
        },
        |c| {
            c.preferred[0].integral.pop();
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut tampered = solution.clone();
        mutate(tampered.basis_change.as_mut().unwrap());
        assert!(
            matches!(
                certify_laporta(&family, &cuts, &tampered, false),
                Err(rustred::solver::SolverError::Certification(_))
            ),
            "basis mutation {index}"
        );
    }
}

#[test]
fn until_stable_reports_the_first_reproduced_depth() {
    let (bubble, _) = bubble(false, false);
    let options = |max_depth| DynamicSolveOptions {
        max_depth,
        until_stable: true,
        ..Default::default()
    };
    let shallow = solve_laporta(&bubble, &uncut(&bubble), &[vec![3, 1]], &[], options(0)).unwrap();
    assert_eq!(
        (shallow.residuals.clone(), shallow.stable_depth),
        (vec![vec![3, 1]], None)
    );
    let stable = solve_laporta(&bubble, &uncut(&bubble), &[vec![3, 1]], &[], options(6)).unwrap();
    assert_eq!(stable.residuals, [vec![1, 1]]);
    assert_eq!((stable.stable_depth, stable.depth), (Some(1), 3));
    let short = solve_laporta(&bubble, &uncut(&bubble), &[vec![3, 1]], &[], options(2)).unwrap();
    assert_eq!((short.stable_depth, short.depth), (None, 2));
}

#[test]
fn certificates_accept_integrals_proved_zero_by_ibp() {
    // [k.k - m2, k.p] with p.p = s: the numerator integral I(1,-1) is zero.
    let c = CoefficientContext::try_new(vec!["d", "m2", "s"]).unwrap();
    let family = IntegralFamily::new(
        "tadpole-numerator",
        vec!["k".into()],
        vec!["p".into()],
        c.clone(),
        c.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(-c.parameter("m2").unwrap(), vec![c.integer(1), c.zero()]),
            AffineDenominator::new(c.zero(), vec![c.zero(), c.integer(1)]),
        ],
        vec![vec![c.parameter("s").unwrap()]],
        vec![c.zero(); 2],
    )
    .unwrap();
    // Odd powers of k.p vanish; I(1,-2) does not.
    for (targets, depth) in [(vec![vec![1, -1]], 1), (vec![vec![2, -1], vec![1, -2]], 2)] {
        let options = DynamicSolveOptions {
            max_depth: depth,
            ..Default::default()
        };
        let solution = solve_laporta(&family, &uncut(&family), &targets, &[], options).unwrap();
        let rules = reductions(&solution);
        assert!(rules[&targets[0]].is_empty());
        certify_laporta(&family, &uncut(&family), &solution, false).unwrap();
    }

    // A returned rule must use residuals only.
    let (bubble, _) = bubble(true, false);
    let options = DynamicSolveOptions {
        max_depth: 1,
        ..Default::default()
    };
    let targets = [vec![1, 1], vec![2, 1], vec![1, 2]];
    let mut solution = solve_laporta(&bubble, &uncut(&bubble), &targets, &[], options).unwrap();
    let rule = solution
        .rules
        .iter_mut()
        .find(|rule| rule.target.iter().map(|power| power.value).eq([2, 1]))
        .unwrap();
    rule.rhs[0].powers = vec![
        rustred::solver::bridge::DynamicPower {
            symbolic: false,
            value: 1,
        },
        rustred::solver::bridge::DynamicPower {
            symbolic: false,
            value: 2,
        },
    ];
    let error = certify_laporta(&bubble, &uncut(&bubble), &solution, false).unwrap_err();
    assert!(error.to_string().contains("not a residual"), "{error}");
}
