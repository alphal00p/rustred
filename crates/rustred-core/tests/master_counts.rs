//! Per-sector master counts from modular critical points of `G = U + F`.
use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::{
    Mask, MasterCount, MasterCountError, MasterCountOptions, MasterCounter, NoVerdictReason,
};

/// Complete family from propagators `(route, mass)`. `route` holds the
/// coefficients of `(k_1, .., k_L, p_1, .., p_E)` in the propagator momentum
/// `q`; the denominator is `q.q - mass` in the bridge's affine coordinates
/// (loop-loop products first, then loop-external products loop-major).
fn family(
    loops: usize,
    c: &CoefficientContext,
    gram: Vec<Vec<Coefficient>>,
    propagators: &[(&[i64], Coefficient)],
) -> IntegralFamily {
    family_with_shifts(
        loops,
        c,
        gram,
        propagators,
        vec![c.zero(); propagators.len()],
    )
}

fn family_with_shifts(
    loops: usize,
    c: &CoefficientContext,
    gram: Vec<Vec<Coefficient>>,
    propagators: &[(&[i64], Coefficient)],
    shifts: Vec<Coefficient>,
) -> IntegralFamily {
    let externals = gram.len();
    let denominators = propagators
        .iter()
        .map(|(route, mass)| {
            let mut constant = -mass.clone();
            for e in 0..externals {
                for f in 0..externals {
                    let weight = c.integer(route[loops + e] * route[loops + f]);
                    constant = &constant + &(&weight * &gram[e][f]);
                }
            }
            let mut coefficients = Vec::new();
            for a in 0..loops {
                for b in a..loops {
                    let symmetry = if a == b { 1 } else { 2 };
                    coefficients.push(c.integer(symmetry * route[a] * route[b]));
                }
            }
            for l in 0..loops {
                for e in 0..externals {
                    coefficients.push(c.integer(2 * route[l] * route[loops + e]));
                }
            }
            AffineDenominator::new(constant, coefficients)
        })
        .collect();
    IntegralFamily::new(
        "master-counts",
        (0..loops).map(|l| format!("k{l}")).collect(),
        (0..externals).map(|e| format!("p{e}")).collect(),
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        gram,
        shifts,
    )
    .unwrap()
}

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d", "m2", "s", "t"]).unwrap()
}

fn p(c: &CoefficientContext, name: &str) -> Coefficient {
    c.parameter(name).unwrap()
}

/// `[k^2 - m1, (k-p)^2 - m2]` with `p^2 = s`.
fn bubble(
    c: &CoefficientContext,
    m1: Coefficient,
    m2: Coefficient,
    s: Coefficient,
) -> IntegralFamily {
    family(1, c, vec![vec![s]], &[(&[1, 0], m1), (&[1, -1], m2)])
}

/// Equal-mass sunrise `[k1^2 - m2, k2^2 - m2, (k1+k2-p)^2 - m2]` with
/// `p^2 = s`, completed by the numerators `(k1-p)^2` and `(k2-p)^2`.
fn sunrise(c: &CoefficientContext, s: Coefficient) -> IntegralFamily {
    let m2 = p(c, "m2");
    family(
        2,
        c,
        vec![vec![s]],
        &[
            (&[1, 0, 0], m2.clone()),
            (&[0, 1, 0], m2.clone()),
            (&[1, 1, -1], m2),
            (&[1, 0, -1], c.zero()),
            (&[0, 1, -1], c.zero()),
        ],
    )
}

fn mask(bits: &str) -> Mask {
    Mask::try_new(bits.chars().map(|bit| bit == '1')).unwrap()
}

fn counts(family: &IntegralFamily, sectors: &[&str]) -> Vec<MasterCount> {
    counts_with(family, MasterCountOptions::default(), sectors)
}

fn counts_with(
    family: &IntegralFamily,
    options: MasterCountOptions,
    sectors: &[&str],
) -> Vec<MasterCount> {
    let mut counter = MasterCounter::try_new(family, options).unwrap();
    sectors
        .iter()
        .map(|sector| counter.count(&mask(sector)).unwrap())
        .collect()
}

fn gram_singular(count: &MasterCount) -> bool {
    matches!(
        count,
        MasterCount::NoVerdict {
            reason: NoVerdictReason::GramSingular,
            ..
        }
    )
}

use MasterCount::{Counted, Zero};

#[test]
fn massive_tadpole_has_one_master() {
    let c = context();
    let tadpole = family(1, &c, Vec::new(), &[(&[1], p(&c, "m2"))]);
    assert_eq!(counts(&tadpole, &["1", "0"]), [Counted(1), Zero]);
}

#[test]
fn massless_bubble_has_one_master_over_scaleless_tadpoles() {
    let c = context();
    let massless = bubble(&c, c.zero(), c.zero(), p(&c, "s"));
    assert_eq!(
        counts(&massless, &["11", "10", "01"]),
        [Counted(1), Zero, Zero]
    );
}

#[test]
fn one_mass_bubble_counts_its_massive_tadpole() {
    let c = context();
    let one_mass = bubble(&c, p(&c, "m2"), c.zero(), p(&c, "s"));
    assert_eq!(
        counts(&one_mass, &["11", "10", "01"]),
        [Counted(1), Counted(1), Zero]
    );
}

#[test]
fn equal_mass_bubble_loses_its_top_master_at_threshold() {
    let c = context();
    let m2 = p(&c, "m2");
    let generic = bubble(&c, m2.clone(), m2.clone(), p(&c, "s"));
    assert_eq!(
        counts(&generic, &["11", "10", "01"]),
        [Counted(1), Counted(1), Counted(1)]
    );
    let threshold = bubble(&c, m2.clone(), m2.clone(), &c.integer(4) * &m2);
    assert_eq!(
        counts(&threshold, &["11", "10", "01"]),
        [Counted(0), Counted(1), Counted(1)]
    );
}

#[test]
fn null_momentum_bubble_has_no_verdict() {
    let c = context();
    let m2 = p(&c, "m2");
    let null = bubble(&c, m2.clone(), m2, c.zero());
    let results = counts(&null, &["11", "10", "01"]);
    assert!(results.iter().all(gram_singular), "{results:?}");
}

#[test]
fn massless_box_has_one_top_master() {
    let c = context();
    let s = p(&c, "s");
    let t = p(&c, "t");
    let half = |value: &Coefficient| value / &c.integer(2);
    let u = -(&s + &t);
    let gram = vec![
        vec![c.zero(), half(&s), half(&u)],
        vec![half(&s), c.zero(), half(&t)],
        vec![half(&u), half(&t), c.zero()],
    ];
    let massless_box = family(
        1,
        &c,
        gram,
        &[
            (&[1, 0, 0, 0], c.zero()),
            (&[1, 1, 0, 0], c.zero()),
            (&[1, 1, 1, 0], c.zero()),
            (&[1, 1, 1, 1], c.zero()),
        ],
    );
    assert_eq!(
        counts(
            &massless_box,
            &["1111", "1110", "1010", "0101", "1100", "1000"]
        ),
        [Counted(1), Counted(0), Counted(1), Counted(1), Zero, Zero]
    );
}

#[test]
fn equal_mass_sunrise_counts_follow_thresholds_and_vacuum_limit() {
    let c = context();
    let m2 = p(&c, "m2");
    let top = "11100";
    let tadpoles = ["11000", "10100", "01100"];
    let generic = counts(
        &sunrise(&c, p(&c, "s")),
        &[top, tadpoles[0], tadpoles[1], tadpoles[2]],
    );
    assert_eq!(generic, [Counted(4), Counted(1), Counted(1), Counted(1)]);
    assert_eq!(
        counts(&sunrise(&c, &c.integer(9) * &m2), &[top]),
        [Counted(3)]
    );
    assert_eq!(counts(&sunrise(&c, m2.clone()), &[top]), [Counted(1)]);

    // Without external momenta the Gram condition is vacuous.
    let vacuum = family(
        2,
        &c,
        Vec::new(),
        &[(&[1, 0], m2.clone()), (&[0, 1], m2.clone()), (&[1, 1], m2)],
    );
    assert_eq!(
        counts(&vacuum, &["111", "110", "100"]),
        [Counted(1), Counted(1), Zero]
    );
}

#[test]
fn null_momentum_sunrise_is_never_counted() {
    let c = context();
    let results = counts(&sunrise(&c, c.zero()), &["11100", "11000"]);
    assert!(results.iter().all(gram_singular), "{results:?}");
}

#[test]
fn massive_triangle_with_null_legs_has_one_top_master_and_guards_null_bubbles() {
    let c = context();
    let m2 = p(&c, "m2");
    let half_s = &p(&c, "s") / &c.integer(2);
    let triangle = family(
        1,
        &c,
        vec![vec![c.zero(), half_s.clone()], vec![half_s, c.zero()]],
        &[
            (&[1, 0, 0], m2.clone()),
            (&[1, 1, 0], m2.clone()),
            (&[1, 1, 1], m2),
        ],
    );
    let results = counts(&triangle, &["111", "101", "100", "110", "011"]);
    assert_eq!(results[..3], [Counted(1), Counted(1), Counted(1)]);
    // An equal-mass bubble on one null leg has a non-isolated Morse locus
    // (G = U - m2 U^2); the family Gram matrix is regular, but the sector
    // still receives no verdict.
    for degenerate in &results[3..] {
        assert!(
            matches!(
                degenerate,
                MasterCount::NoVerdict {
                    morse: None,
                    reason: NoVerdictReason::MorseNonIsolated,
                    ..
                }
            ),
            "{degenerate:?}"
        );
    }
}

#[test]
fn counts_are_deterministic_and_seed_independent() {
    let c = context();
    let m2 = p(&c, "m2");
    let one_mass = bubble(&c, m2, c.zero(), p(&c, "s"));
    let sectors = ["11", "10", "01"];
    let seeded = |seed| {
        counts_with(
            &one_mass,
            MasterCountOptions {
                seed,
                ..MasterCountOptions::default()
            },
            &sectors,
        )
    };
    let first = seeded(7);
    assert_eq!(first, seeded(7));
    for other in [seeded(0), seeded(0xdead_beef)] {
        assert_eq!(first, other);
    }
    assert!(first.iter().all(|count| matches!(count, Counted(_) | Zero)));

    let mut counter = MasterCounter::try_new(&one_mass, MasterCountOptions::default()).unwrap();
    let again = counter.count(&mask("11")).unwrap();
    assert_eq!(counter.count(&mask("11")).unwrap(), again);
}

#[test]
fn power_shifts_arity_and_sampling_are_explicit_errors() {
    let c = context();
    let half = &c.integer(1) / &c.integer(2);
    let shifted = family_with_shifts(
        1,
        &c,
        vec![vec![p(&c, "s")]],
        &[(&[1, 0], p(&c, "m2")), (&[1, -1], c.zero())],
        vec![c.zero(), half],
    );
    assert_eq!(
        MasterCounter::try_new(&shifted, MasterCountOptions::default()).unwrap_err(),
        MasterCountError::UnsupportedNonzeroPowerShift { denominator: 1 }
    );

    let one_mass = bubble(&c, p(&c, "m2"), c.zero(), p(&c, "s"));
    let mut counter = MasterCounter::try_new(&one_mass, MasterCountOptions::default()).unwrap();
    assert_eq!(
        counter.count(&mask("110")).unwrap_err(),
        MasterCountError::WrongArity {
            expected: 2,
            actual: 3
        }
    );

    let single = MasterCountOptions {
        samples: 1,
        ..MasterCountOptions::default()
    };
    assert_eq!(
        MasterCounter::try_new(&one_mass, single).unwrap_err(),
        MasterCountError::InvalidSampleCount { samples: 1 }
    );
    let excessive = MasterCountOptions {
        samples: rustred::sector::masters::MAX_SAMPLES + 1,
        ..MasterCountOptions::default()
    };
    assert!(matches!(
        MasterCounter::try_new(&one_mass, excessive),
        Err(MasterCountError::InvalidSampleCount { .. })
    ));

    let exhausted = MasterCountOptions {
        max_attempts: 0,
        ..MasterCountOptions::default()
    };
    let mut counter = MasterCounter::try_new(&one_mass, exhausted).unwrap();
    // Scaleless sectors need no sample point.
    assert_eq!(counter.count(&mask("01")).unwrap(), Zero);
    assert_eq!(
        counter.count(&mask("11")).unwrap_err(),
        MasterCountError::AttemptsExhausted {
            sample: 0,
            attempts: 0
        }
    );
}

/// A regular family Gram matrix can still hold a sector whose own momentum is
/// light-like. There the count must agree with what IBP actually reduces.
#[test]
fn null_leg_bubble_inside_a_regular_triangle_matches_the_reduction() {
    use rustred::sector::CutConstraint;
    use rustred::solver::bridge::{DynamicSolveOptions, solve_laporta};

    let c = CoefficientContext::try_new(["d", "M", "m", "s", "b"]).unwrap();
    let (big, small, b) = (p(&c, "M"), p(&c, "m"), p(&c, "b"));
    // p1^2 = 0, p2^2 = b and (p1 + p2)^2 = s.
    let half = &(&p(&c, "s") - &b) / &c.integer(2);
    let triangle = family(
        1,
        &c,
        vec![vec![c.zero(), half.clone()], vec![half, b]],
        &[
            (&[1, 0, 0], big.clone()),
            (&[1, 1, 0], small),
            (&[1, 1, 1], big),
        ],
    );
    let sectors = ["111", "110", "101", "011", "100", "010", "001"];
    let counted = counts(&triangle, &sectors);
    assert_eq!(counted[1], Counted(0), "the null unequal-mass bubble");
    let options = DynamicSolveOptions {
        max_depth: 4,
        until_stable: true,
        ..Default::default()
    };
    let targets = [
        vec![1, 1, 1],
        vec![2, 1, 1],
        vec![1, 1, 0],
        vec![2, 1, 0],
        vec![1, 2, 0],
        vec![1, 0, 1],
        vec![0, 1, 1],
    ];
    let solution = solve_laporta(
        &triangle,
        &CutConstraint::none(3).unwrap(),
        &targets,
        &[],
        options,
    )
    .unwrap();
    assert!(solution.stable_depth.is_some());
    for (sector, count) in sectors.iter().zip(&counted) {
        let residuals = solution
            .residuals
            .iter()
            .filter(|residual| {
                residual
                    .iter()
                    .map(|&power| if power > 0 { '1' } else { '0' })
                    .eq(sector.chars())
            })
            .count();
        if let Counted(masters) = count {
            assert!(
                residuals <= *masters,
                "sector {sector}: {residuals} > {masters}"
            );
        }
    }
}
