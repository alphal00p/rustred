//! The Insertion, SeriesParallel and Carpet kernels (owner: M2-T4 rr-kernel):
//! closed G-forms, Forcer's `reduceG`, the transverse oracle of the architecture
//! doc (section 3.2), nested Euclidean G forms through convention C1, and exact
//! agreement with reductions of the P0 lane (kite and 3L carpet sectors).
//!
//! Presentations are hand-built from the Forcer graphs of each node (notation of
//! `crates/rustred-topology/tests/fixtures/forcer/forcer_dag_68a30bf.tsv`).
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::carpet::CarpetStep;
use rustred::propagator::census::{PCensus, PClassRecord, SignedAffineLoopMap};
use rustred::propagator::class::OwnerClassKey;
use rustred::propagator::error::PError;
use rustred::propagator::insertion::InsertionStep;
use rustred::propagator::kernel::bubble::{
    BubbleKernel, LinePower, box_pairings, ghat, relative_tag,
};
use rustred::propagator::kernel::gamma::{FactoredRational, GammaProduct};
use rustred::propagator::lane::{BubbleRoles, CarpetRoles, LaneRoles, OwnerLane};
use rustred::propagator::library::{OwnerProgram, PLibrary, POwnerSpec};
use rustred::propagator::presentation::{
    EdgeKind, EdgeMap, Presentation, PresentationKind, Slot, SlotIsp, SlotLine,
    check_carpet_contract, check_insertion_contract,
};
use rustred::propagator::series_parallel::SeriesParallelStep;
use rustred::propagator::step::{PKeyView, PLaneStep, PStage, PStepContext, PStepOut};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::{P_CARPET_KERNEL_VERSION, P_INSERTION_KERNEL_VERSION, PresentationId};
use rustred::reduction::ReductionLimits;
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use symbolica::domains::rational::{Q, Rational};
use symbolica::poly::factor::Factorize;

use crate::support::p_integrals::{GValue, PFamily, Power, collect, g, rational};

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn algebra() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}

fn limits() -> ReductionLimits {
    ReductionLimits::default()
}

fn line(momentum: &[i64]) -> Slot {
    eps_line(momentum, 0)
}

fn eps_line(momentum: &[i64], eps_units: u8) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units,
    })
}

fn isp(left: &[i64], right: &[i64]) -> Slot {
    Slot::Isp(SlotIsp {
        left: left.to_vec(),
        right: right.to_vec(),
    })
}

fn presentation(id: u32, loops: u8, slots: Vec<Slot>) -> Arc<Presentation> {
    Arc::new(
        Presentation::from_slots(
            PresentationId(id),
            PresentationKind::EpsNode,
            &format!("hand_built_{id}"),
            loops,
            slots,
            &context(),
            None,
        )
        .unwrap(),
    )
}

/// `k_in = Sum_j linear[i][j] k_out[j] + external[i] q_out`, `q_in = sign q_out`.
fn loop_map(linear: &[i64], external: &[i64], sign: i8) -> SignedAffineLoopMap {
    let size = external.len() as u8;
    SignedAffineLoopMap {
        loops_in: size,
        loops_out: size,
        loop_linear: linear.into(),
        loop_external: external.into(),
        external_sign: sign,
    }
}

fn insertion_edge(
    parent: &Presentation,
    child: &Presentation,
    integrated_loop: u8,
    outer_map: SignedAffineLoopMap,
    line_slots: &[Option<u16>],
    merged_slot: Option<u16>,
) -> (EdgeMap, Arc<Presentation>) {
    let edge = EdgeMap {
        parent: parent.id,
        child: child.id,
        kind: EdgeKind::Insertion,
        integrated_loop,
        outer_map,
        line_slots: line_slots.to_vec(),
        merged_slot,
    };
    (edge, Arc::new(child.clone()))
}

fn carpet_edge(
    parent: &Presentation,
    child: &Presentation,
    integrated_loop: u8,
    outer_map: SignedAffineLoopMap,
    line_slots: &[Option<u16>],
) -> (EdgeMap, Arc<Presentation>) {
    let edge = EdgeMap {
        parent: parent.id,
        child: child.id,
        kind: EdgeKind::Carpet,
        integrated_loop,
        outer_map,
        line_slots: line_slots.to_vec(),
        merged_slot: None,
    };
    (edge, Arc::new(child.clone()))
}

/// A presentation whose first two lines are the bubble of loop 0.
const FIRST_BUBBLE: BubbleRoles = BubbleRoles {
    lines: [0, 1],
    loop_index: 0,
};

/// The 1L bubble `k^2, (k - q)^2` with the given eps units, `d3`, `d4`, `d6`, `d7`.
fn one_loop(id: u32, eps: [u8; 2]) -> Arc<Presentation> {
    presentation(
        id,
        1,
        vec![eps_line(&[1, 0], eps[0]), eps_line(&[1, -1], eps[1])],
    )
}

fn one_loop_step(p: &Arc<Presentation>) -> Arc<InsertionStep> {
    Arc::new(InsertionStep::new(p.clone(), FIRST_BUBBLE, &[]).unwrap())
}

/// `value` at a rational `d`.
fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

fn sign(power: i64) -> Rational {
    rational(if power % 2 == 0 { 1 } else { -1 }, 1)
}

fn integer(value: i64) -> Rational {
    rational(value, 1)
}

fn power_of(base: &Rational, exponent: u32) -> Rational {
    (0..exponent).fold(Rational::one(), |product, _| &product * base)
}

fn dimensions(rng: &mut StdRng, count: usize) -> Vec<Rational> {
    // Over the prime 101, never a multiple of it: no integer or half-integer.
    (0..count)
        .map(|_| {
            rational(
                101 * rng.random_range(1..20_i64) + rng.random_range(1..101),
                101,
            )
        })
        .collect()
}

/// `Gamma(c + m d/2)` for `m != 0` as `Pochhammer * Gamma(m d/2)`.
fn gamma_value(c: i64, m: i64, d: &Rational) -> GValue {
    let x = &(d * &rational(m, 2));
    let mut coefficient = Rational::one();
    for j in 0..c {
        coefficient = &coefficient * &(x + &integer(j));
    }
    for j in 1..=-c {
        coefficient = &coefficient / &(x - &integer(j));
    }
    GValue {
        coefficient,
        signature: BTreeMap::from([(m, 1)]),
    }
}

/// The value of a tag at `d`, with `1 + m eps = 1 + 2 m - m d/2`.
fn tag_value(tag: &GammaTag, d: &Rational) -> GValue {
    let mut value = GValue::rational(Rational::one());
    for &(m, e) in tag.exponents() {
        let (m, e) = (i64::from(m), i64::from(e));
        let base = gamma_value(1 + 2 * m, -m, d);
        let factor = if e > 0 {
            base
        } else {
            GValue {
                coefficient: base.coefficient.inv(),
                signature: base.signature.iter().map(|(m, e)| (*m, -e)).collect(),
            }
        };
        for _ in 0..e.abs() {
            value = value.mul(&factor);
        }
    }
    value
}

/// `n + j eps` as `Power = c + m d/2`.
fn power(n: i64, j: i64) -> Power {
    (n + 2 * j, -j)
}

fn assert_values_equal(left: &[GValue], right: &[GValue], label: &str) {
    let minus = rational(-1, 1);
    let difference = left
        .iter()
        .cloned()
        .chain(right.iter().map(|value| value.scale(&minus)));
    assert!(
        collect(difference).is_empty(),
        "{label}: {:?} differs from {:?}",
        collect(left.iter().cloned()),
        collect(right.iter().cloned())
    );
}

/// The terminals of a fully reduced key at `d`.
fn terminal_values(output: &PStepOut, d: &Rational) -> Vec<GValue> {
    assert!(output.children.is_empty());
    output
        .terminals
        .iter()
        .map(|(id, tag, weight)| {
            assert_eq!(id.0, 0);
            tag_value(tag, d).scale(&at(weight, d))
        })
        .collect()
}

fn key_view<'a>(p: &Presentation, powers: &'a [i64], tag: &'a GammaTag) -> PKeyView<'a> {
    PKeyView {
        presentation: p.id,
        powers,
        tag,
    }
}

/// The eps units of every line, zero on ISPs.
fn eps_units(p: &Presentation) -> Vec<i64> {
    p.slots
        .iter()
        .map(|slot| match slot {
            Slot::Line(line) => i64::from(line.eps_units),
            Slot::Isp(_) => 0,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Gamma tags and rationals
// ---------------------------------------------------------------------------

#[test]
fn gamma_functions_split_into_tags_and_linear_factors() {
    let c = context();
    let eps = rustred::propagator::presentation::eps_power_shift(&c, 1).unwrap();
    let plus = |a: i64, b: i64| {
        let slope = c.try_mul(&c.integer(b), &eps, algebra()).unwrap();
        c.try_add(&c.integer(a), &slope, algebra()).unwrap()
    };

    // Gamma(3 + 2 eps) = Gamma(1 + 2 eps) (1 + 2 eps) (2 + 2 eps).
    let mut product = GammaProduct::one();
    product.gamma(3, 2).unwrap();
    assert_eq!(
        product.tag().unwrap(),
        GammaTag::from_pairs([(2, 1)]).unwrap()
    );
    let expected = c.try_mul(&plus(1, 2), &plus(2, 2), algebra()).unwrap();
    assert_eq!(
        product.rational().to_coefficient(&c, algebra()).unwrap(),
        expected
    );

    // Gamma(-1 + eps) = Gamma(1 + eps) / (eps (eps - 1)), the inverse likewise.
    let mut product = GammaProduct::one();
    product.gamma(-1, 1).unwrap();
    let expected = c
        .try_div(
            &c.one(),
            &c.try_mul(&plus(0, 1), &plus(-1, 1), algebra()).unwrap(),
            algebra(),
        )
        .unwrap();
    assert_eq!(
        product.rational().to_coefficient(&c, algebra()).unwrap(),
        expected
    );
    product.inverse_gamma(-1, 1).unwrap();
    assert_eq!(product, GammaProduct::one());

    // s = 0: factorials, a zero, and a typed refusal.
    let mut product = GammaProduct::one();
    product.gamma(4, 0).unwrap();
    assert_eq!(
        product.rational().to_coefficient(&c, algebra()).unwrap(),
        c.integer(6)
    );
    product.inverse_gamma(0, 0).unwrap();
    assert!(product.rational().is_zero());
    let mut product = GammaProduct::one();
    assert!(matches!(
        product.gamma(0, 0),
        Err(PError::Unsupported(detail)) if detail.contains("Gamma(0)")
    ));
    assert!(matches!(
        product.gamma(1, 100).and_then(|()| product.tag()),
        Err(PError::TagOutOfRange)
    ));

    // Cancelling factors leave no trace: 1 / (2 + 2 eps) * (1 + eps) = 1 / 2.
    let mut value = FactoredRational::one();
    value.mul_linear(2, 2, -1).unwrap();
    value.mul_linear(1, 1, 1).unwrap();
    assert_eq!(value.factors().count(), 0);
    assert_eq!(value.prefactor(), &rational(1, 2));
    assert!(matches!(
        FactoredRational::pochhammer(1, 0, -1),
        Err(PError::Unsupported(_))
    ));
    assert!(FactoredRational::pochhammer(-1, 0, 2).unwrap().is_zero());
}

// ---------------------------------------------------------------------------
// Acceptance (a): the bubble is the G-function, exact in Q(d) x tags
// ---------------------------------------------------------------------------

/// `(x)_k`, for `x` in `Q(d)`.
fn pochhammer_d(c: &CoefficientContext, x: &Coefficient, k: i64) -> Coefficient {
    let shifted = |i: i64| c.try_add(x, &c.integer(i), algebra()).unwrap();
    let mut value = c.one();
    for i in 0..k {
        value = c.try_mul(&value, &shifted(i), algebra()).unwrap();
    }
    for i in 1..=-k {
        value = c.try_div(&value, &shifted(-i), algebra()).unwrap();
    }
    value
}

/// `G(a, b) / T_ins(0, 0)` in `Q(d)` for integer `a`, `b`, from the Gamma ratio
/// `Gamma(d/2-a) Gamma(d/2-b) Gamma(a+b-d/2) / (Gamma(a) Gamma(b) Gamma(d-a-b))`.
fn g_over_tag(c: &CoefficientContext, a: i64, b: i64) -> Coefficient {
    if a <= 0 || b <= 0 {
        return c.zero();
    }
    let d = c.parameter("d").unwrap();
    let half = c.try_div(&d, &c.integer(2), algebra()).unwrap();
    let half_minus_one = c.try_sub(&half, &c.one(), algebra()).unwrap();
    let three_minus_half = c.try_sub(&c.integer(3), &half, algebra()).unwrap();
    let d_minus_three = c.try_sub(&d, &c.integer(3), algebra()).unwrap();
    let factorial = |n: i64| c.integer((1..n).product::<i64>().max(1));
    let product = [
        pochhammer_d(c, &half_minus_one, 1 - a),
        pochhammer_d(c, &half_minus_one, 1 - b),
        pochhammer_d(c, &three_minus_half, a + b - 3),
    ]
    .iter()
    .fold(c.one(), |value, factor| {
        c.try_mul(&value, factor, algebra()).unwrap()
    });
    let denominator = [
        factorial(a),
        factorial(b),
        pochhammer_d(c, &d_minus_three, 3 - a - b),
    ]
    .iter()
    .fold(c.one(), |value, factor| {
        c.try_mul(&value, factor, algebra()).unwrap()
    });
    c.try_div(&product, &denominator, algebra()).unwrap()
}

/// `I_M` of `(k.q)^m / ((k^2)^a ((k - P)^2)^b)` for `P = s q`, expanding
/// `k.q = s (k^2 + q^2 - (k - P)^2) / 2` with `q.q = -1` into scalar bubbles.
fn scalar_expansion(c: &CoefficientContext, a: i64, b: i64, m: u32, s: i64) -> Coefficient {
    let mut total = c.zero();
    let m = i64::from(m);
    let factorial = |n: i64| (1..=n).product::<i64>();
    for i in 0..=m {
        for l in 0..=m - i {
            let j = m - i - l;
            // (k^2)^i (q^2)^j (-(k-P)^2)^l with q^2 = -1.
            let count = factorial(m) / (factorial(i) * factorial(j) * factorial(l));
            let scale =
                &rational(count, 1 << m) * &(&sign(j + l) * &power_of(&integer(s), m as u32));
            let (first, second) = (a - i, b - l);
            let term = g_over_tag(c, first, second);
            let scale = &scale * &sign(first + second);
            let term = c
                .try_mul(&term, &coefficient_of(c, &scale), algebra())
                .unwrap();
            total = c.try_add(&total, &term, algebra()).unwrap();
        }
    }
    total
}

fn coefficient_of(c: &CoefficientContext, value: &Rational) -> Coefficient {
    FactoredRational::from_rational(value.clone())
        .to_coefficient(c, algebra())
        .unwrap()
}

/// `I_M` of the same integral through the Box kernel, Minkowski native:
/// `P^2 = q.q = -1`, `P.q = -s`, and the merged line absorbed with `(-1)^{a + b}`.
fn kernel_value(kernel: &BubbleKernel, a: i64, b: i64, m: u32, s: i64) -> Coefficient {
    let c = kernel.context();
    let terms = kernel
        .numerator_terms(LinePower::new(a, 0), LinePower::new(b, 0), &[m], algebra())
        .unwrap();
    let mut total = c.zero();
    for term in &terms {
        let contracted = term.pairing.pairs.iter().map(|&(_, _, n)| n).sum::<u32>();
        assert_eq!(contracted, term.sigma);
        // (P^2)^sigma (q.q)^contracted (P.q)^rest (-1)^{a + b}.
        let scale = &(&power_of(&integer(-1), term.sigma + contracted)
            * &power_of(&integer(-s), term.pairing.rest[0]))
            * &sign(a + b);
        let value = c
            .try_mul(&term.weight, &coefficient_of(c, &scale), algebra())
            .unwrap();
        total = c.try_add(&total, &value, algebra()).unwrap();
    }
    total
}

#[test]
fn the_one_loop_bubble_is_the_g_function_to_rank_8() {
    let c = context();
    let kernel = BubbleKernel::new(&c);
    let tag = relative_tag(0, 0).unwrap();
    assert_eq!(
        tag,
        GammaTag::from_pairs([(1, 1), (-1, 2), (-2, -1)]).unwrap()
    );
    // Orientation P = +-q, numerators (k.q)^m (k^2)^t of rank m + 2 t.
    for a in 1..=6 {
        for b in 1..=6 {
            for rank in 0..=8_u32 {
                for t in 0..=rank / 2 {
                    let m = rank - 2 * t;
                    let a = a - i64::from(t);
                    for s in [1, -1] {
                        assert_eq!(
                            kernel_value(&kernel, a, b, m, s),
                            scalar_expansion(&c, a, b, m, s),
                            "a = {a}, b = {b}, m = {m}, t = {t}, s = {s}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn one_loop_steps_end_at_the_unit_terminal_for_every_routing() {
    let c = context();
    let tag = relative_tag(0, 0).unwrap();
    let parent = GammaTag::from_pairs([(3, 2)]).unwrap();
    let routings: [(&[i64], &[i64]); 4] = [
        (&[1, 0], &[1, -1]),
        (&[-1, 0], &[1, -1]),
        (&[1, 0], &[1, 1]),
        (&[-1, 0], &[-1, 1]),
    ];
    for (first, second) in routings {
        let p = presentation(0, 1, vec![line(first), line(second)]);
        let step = one_loop_step(&p);
        for a in -1..=6_i64 {
            for b in -1..=6_i64 {
                let powers = [a, b];
                let out = step
                    .apply(&key_view(&p, &powers, &parent), &limits())
                    .unwrap();
                assert!(out.children.is_empty() && out.conditions.is_empty());
                if a <= 0 || b <= 0 {
                    assert!(out.terminals.is_empty());
                    continue;
                }
                let [(id, terminal_tag, weight)] = out.terminals.as_slice() else {
                    panic!("one terminal expected");
                };
                assert_eq!(id.0, 0);
                assert_eq!(*terminal_tag, parent.add(&tag));
                let expected = c
                    .try_mul(
                        &g_over_tag(&c, a, b),
                        &coefficient_of(&c, &sign(a + b)),
                        algebra(),
                    )
                    .unwrap();
                assert_eq!(*weight, expected, "a = {a}, b = {b}");
            }
        }
    }
}

/// `P1L(1,1) = 1/eps` in the G-scheme: `I_M(1,1) = G(1,1) = T_ins / (eps (d - 3))`
/// (convention C3), and the odd-rank `k.q` anchor for `q` and `-q`.
#[test]
fn the_normalisation_anchors_hold() {
    let c = context();
    let p = one_loop(0, [0, 0]);
    let step = one_loop_step(&p);
    let tag = GammaTag::one();
    let out = step.apply(&key_view(&p, &[1, 1], &tag), &limits()).unwrap();
    let [(_, terminal_tag, weight)] = out.terminals.as_slice() else {
        panic!("one terminal expected");
    };
    assert_eq!(
        *terminal_tag,
        GammaTag::from_pairs([(1, 1), (-1, 2), (-2, -1)]).unwrap()
    );
    let d = c.parameter("d").unwrap();
    let d_minus_three = c.try_sub(&d, &c.integer(3), algebra()).unwrap();
    let four_minus_d = c.try_sub(&c.integer(4), &d, algebra()).unwrap();
    let one_over_eps = c.try_div(&c.integer(2), &four_minus_d, algebra()).unwrap();
    assert_eq!(
        c.try_mul(weight, &d_minus_three, algebra()).unwrap(),
        one_over_eps
    );

    // Int k.q / (k^2 (k - P)^2) with P = s q: k.q -> (P.q) P^2 / (2 P^2) = s (q.q) / 2.
    let kernel = BubbleKernel::new(&c);
    for s in [1, -1] {
        let half = coefficient_of(&c, &rational(-s, 2));
        assert_eq!(
            kernel_value(&kernel, 1, 1, 1, s),
            c.try_mul(&half, &g_over_tag(&c, 1, 1), algebra()).unwrap()
        );
    }
}

// ---------------------------------------------------------------------------
// Acceptance (b): Forcer's reduceG, verbatim
// ---------------------------------------------------------------------------

/// `n + slope eps`-valued `x` of Forcer's `poch` and `pochinv`.
#[derive(Clone, Copy)]
struct ForcerArgument {
    constant: i64,
    slope: i64,
}

fn forcer_num(c: &CoefficientContext, x: ForcerArgument, shift: i64) -> Coefficient {
    let eps = rustred::propagator::presentation::eps_power_shift(c, 1).unwrap();
    let slope = c.try_mul(&c.integer(x.slope), &eps, algebra()).unwrap();
    c.try_add(&c.integer(x.constant + shift), &slope, algebra())
        .unwrap()
}

/// `repeat id poch(n?pos_, x?) = poch(n-1, x) * num(n-1+x)`,
/// `poch(n?neg_, x?) = poch(n+1, x) * den(n+x)` (fa:1409-1410).
fn forcer_poch(c: &CoefficientContext, n: i64, x: ForcerArgument) -> Coefficient {
    let mut value = c.one();
    for i in 0..n {
        value = c.try_mul(&value, &forcer_num(c, x, i), algebra()).unwrap();
    }
    for i in n..0 {
        value = c.try_div(&value, &forcer_num(c, x, i), algebra()).unwrap();
    }
    value
}

/// `repeat id pochinv(n?pos_, x?) = pochinv(n-1, x) * den(n-1+x)`,
/// `pochinv(n?neg_, x?) = pochinv(n+1, x) * num(n+x)` (fa:1411-1412).
fn forcer_pochinv(c: &CoefficientContext, n: i64, x: ForcerArgument) -> Coefficient {
    let mut value = c.one();
    for i in 0..n {
        value = c.try_div(&value, &forcer_num(c, x, i), algebra()).unwrap();
    }
    for i in n..0 {
        value = c.try_mul(&value, &forcer_num(c, x, i), algebra()).unwrap();
    }
    value
}

/// `G(n1 + x1 ep, n2 + x2 ep, n3, n4) / (1 + x1 + x2) G(1 + x1 ep, 1 + x2 ep, 0, 0) * (1 + x1 + x2)`,
/// the Pochhammer product of `reduceG` (fa:1414-1420).
fn forcer_reduce_g(
    c: &CoefficientContext,
    (n1, x1): (i64, i64),
    (n2, x2): (i64, i64),
    n3: i64,
    n4: i64,
) -> Coefficient {
    let arg = |constant, slope| ForcerArgument { constant, slope };
    let factors = [
        forcer_poch(c, n1 + n2 - n4 - 2, arg(0, 1 + x1 + x2)),
        forcer_poch(c, 1 - n1 + n3 - n4, arg(1, -1 - x1)),
        forcer_poch(c, 1 - n2 + n4, arg(1, -1 - x2)),
        forcer_pochinv(c, n1 - 1, arg(1, x1)),
        forcer_pochinv(c, n2 - 1, arg(1, x2)),
        forcer_pochinv(c, 2 - n1 - n2 + n3, arg(2, -2 - x1 - x2)),
    ];
    factors.iter().fold(c.one(), |value, factor| {
        c.try_mul(&value, factor, algebra()).unwrap()
    })
}

#[test]
fn the_kernel_equals_forcer_reduce_g() {
    let c = context();
    let kernel = BubbleKernel::new(&c);
    for x1 in 0..=3 {
        for x2 in 0..=3 {
            let base = kernel
                .ratio(
                    LinePower::new(1, x1),
                    LinePower::new(1, x2),
                    0,
                    0,
                    algebra(),
                )
                .unwrap();
            // GschemeConstants(x1, x2) = T_ins / (eps (1 - (2 + x1 + x2) eps)).
            let constant = forcer_num(
                &c,
                ForcerArgument {
                    constant: 1,
                    slope: -2 - x1 - x2,
                },
                0,
            );
            let eps = forcer_num(
                &c,
                ForcerArgument {
                    constant: 0,
                    slope: 1,
                },
                0,
            );
            let g_scheme = c.try_mul(&constant, &eps, algebra()).unwrap();
            assert_eq!(
                c.try_mul(&base, &g_scheme, algebra()).unwrap(),
                c.try_div(&c.one(), &c.integer(1 + x1 + x2), algebra())
                    .unwrap()
            );
            for n1 in -1..=4 {
                for n2 in -1..=4 {
                    for n in 0..=4 {
                        for sigma in 0..=n / 2 {
                            let ratio = kernel
                                .ratio(
                                    LinePower::new(n1, x1),
                                    LinePower::new(n2, x2),
                                    n,
                                    sigma,
                                    algebra(),
                                )
                                .unwrap();
                            let forcer = forcer_reduce_g(&c, (n1, x1), (n2, x2), n, sigma);
                            assert_eq!(
                                ratio,
                                c.try_mul(&base, &forcer, algebra()).unwrap(),
                                "x = ({x1}, {x2}), n = ({n1}, {n2}, {n}, {sigma})"
                            );
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance (c): the transverse oracle of the architecture doc, section 3.2
// ---------------------------------------------------------------------------

/// One monomial of the oracle: `D_a^a D_b^b (P^2)^p2` times outer scalar products.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OracleMonomial {
    a: i32,
    b: i32,
    p2: i32,
    outer: Vec<u32>,
}

type OraclePolynomial = BTreeMap<OracleMonomial, Rational>;

fn oracle_mul(x: &OraclePolynomial, y: &OraclePolynomial) -> OraclePolynomial {
    let mut product = OraclePolynomial::new();
    for (left, u) in x {
        for (right, v) in y {
            let monomial = OracleMonomial {
                a: left.a + right.a,
                b: left.b + right.b,
                p2: left.p2 + right.p2,
                outer: left
                    .outer
                    .iter()
                    .zip(&right.outer)
                    .map(|(p, q)| p + q)
                    .collect(),
            };
            let entry = product.entry(monomial).or_insert_with(Rational::zero);
            *entry = &*entry + &(u * v);
        }
    }
    product.retain(|_, value| !value.is_zero());
    product
}

fn oracle_add(x: &mut OraclePolynomial, y: &OraclePolynomial) {
    for (monomial, value) in y {
        let entry = x.entry(monomial.clone()).or_insert_with(Rational::zero);
        *entry = &*entry + value;
    }
    x.retain(|_, value| !value.is_zero());
}

/// Every set of `pairs` disjoint pairs of `0..size`.
fn matchings(size: usize, pairs: usize) -> Vec<Vec<(usize, usize)>> {
    fn extend(
        first: usize,
        left: usize,
        used: &mut Vec<bool>,
        current: &mut Vec<(usize, usize)>,
        out: &mut Vec<Vec<(usize, usize)>>,
    ) {
        if left == 0 {
            out.push(current.clone());
            return;
        }
        for a in first..used.len() {
            if used[a] {
                continue;
            }
            used[a] = true;
            for b in a + 1..used.len() {
                if used[b] {
                    continue;
                }
                used[b] = true;
                current.push((a, b));
                extend(a + 1, left - 1, used, current, out);
                current.pop();
                used[b] = false;
            }
            used[a] = false;
        }
    }
    let mut out = Vec::new();
    extend(0, pairs, &mut vec![false; size], &mut Vec::new(), &mut out);
    out
}

/// Outer variables of the oracle: `Y_s.Y_t` for `s <= t`, then `P.Y_s`.
struct OuterVariables {
    variables: usize,
}

impl OuterVariables {
    fn count(&self) -> usize {
        self.variables * (self.variables + 1) / 2 + self.variables
    }

    fn pair(&self, s: usize, t: usize) -> usize {
        let (s, t) = (s.min(t), s.max(t));
        s * self.variables - s * (s + 1) / 2 + t
    }

    fn residual(&self, s: usize) -> usize {
        self.variables * (self.variables + 1) / 2 + s
    }

    fn unit(&self) -> OracleMonomial {
        OracleMonomial {
            a: 0,
            b: 0,
            p2: 0,
            outer: vec![0; self.count()],
        }
    }

    fn variable(&self, index: usize) -> OracleMonomial {
        let mut monomial = self.unit();
        monomial.outer[index] = 1;
        monomial
    }
}

/// `Int d^dk / pi^(d/2) Prod_i (k.Y_{factors[i]}) / ((k^2)^alpha ((k-P)^2)^beta)` by
/// the transverse decomposition, without any use of the kernel:
/// `k.Y = (k.P)(P.Y)/P^2 + k_perp.Y`, `k.P = (D_a + P^2 - D_b) / 2`,
/// `k_perp^2 = D_a - (k.P)^2 / P^2`, and an even tensor of `k_perp` is
/// `(k_perp^2)^m` times the pairings of `g_perp`, over `(d-1)(d+1)..(d+2m-3)`.
///
/// The result, as a polynomial in `D_a`, `D_b`, `P^2` and the outer scalars,
/// awaits the scalar integrals `D_a^s D_b^t -> G(alpha - s, beta - t) (P^2)^(s + t)`.
fn transverse_oracle(
    variables: usize,
    factors: &[usize],
    d: &Rational,
) -> (OuterVariables, OraclePolynomial) {
    let outer = OuterVariables { variables };
    let unit = outer.unit();
    let rank = factors.len();
    let one_half = rational(1, 2);
    let k_dot_p = OraclePolynomial::from([
        (
            OracleMonomial {
                a: 1,
                ..unit.clone()
            },
            one_half.clone(),
        ),
        (
            OracleMonomial {
                p2: 1,
                ..unit.clone()
            },
            one_half.clone(),
        ),
        (
            OracleMonomial {
                b: 1,
                ..unit.clone()
            },
            -one_half.clone(),
        ),
    ]);
    let over_p2 = OraclePolynomial::from([(
        OracleMonomial {
            p2: -1,
            ..unit.clone()
        },
        Rational::one(),
    )]);
    let k_perp_2 = {
        let mut value = OraclePolynomial::from([(
            OracleMonomial {
                a: 1,
                ..unit.clone()
            },
            Rational::one(),
        )]);
        let square = oracle_mul(&oracle_mul(&k_dot_p, &k_dot_p), &over_p2);
        oracle_add(
            &mut value,
            &square
                .into_iter()
                .map(|(monomial, value)| (monomial, -value))
                .collect(),
        );
        value
    };
    let parallel = oracle_mul(&k_dot_p, &over_p2);
    let one = OraclePolynomial::from([(unit.clone(), Rational::one())]);

    let mut total = OraclePolynomial::new();
    for pairs in 0..=rank / 2 {
        // (k.P / P^2)^(r - 2 m) (k_perp^2)^m / ((d-1)(d+1)..).
        let mut scalar = one.clone();
        for _ in 0..rank - 2 * pairs {
            scalar = oracle_mul(&scalar, &parallel);
        }
        for i in 0..pairs {
            scalar = oracle_mul(&scalar, &k_perp_2);
            let divisor = &(d - &integer(1)) + &integer(2 * i as i64);
            for value in scalar.values_mut() {
                *value = &*value / &divisor;
            }
        }
        // Sum over the partial matchings of Prod P.Y and Prod g_perp.
        let mut tensors = OraclePolynomial::new();
        for matching in matchings(rank, pairs) {
            let mut matched = vec![false; rank];
            let mut term = one.clone();
            for &(a, b) in &matching {
                matched[a] = true;
                matched[b] = true;
                let (s, t) = (factors[a], factors[b]);
                let mut both = outer.variable(outer.residual(s));
                both.outer[outer.residual(t)] += 1;
                both.p2 = -1;
                term = oracle_mul(
                    &term,
                    &OraclePolynomial::from([
                        (outer.variable(outer.pair(s, t)), Rational::one()),
                        (both, rational(-1, 1)),
                    ]),
                );
            }
            for (position, _) in matched.iter().enumerate().filter(|(_, done)| !**done) {
                term = oracle_mul(
                    &term,
                    &OraclePolynomial::from([(
                        outer.variable(outer.residual(factors[position])),
                        Rational::one(),
                    )]),
                );
            }
            oracle_add(&mut tensors, &term);
        }
        oracle_add(&mut total, &oracle_mul(&scalar, &tensors));
    }
    (outer, total)
}

/// The numerator as a list of variables per factor.
fn factor_list(powers: &[u32]) -> Vec<usize> {
    powers
        .iter()
        .enumerate()
        .flat_map(|(s, &m)| std::iter::repeat_n(s, m as usize))
        .collect()
}

fn random_line_power(rng: &mut StdRng) -> LinePower {
    if rng.random_bool(0.5) {
        LinePower::new(rng.random_range(1..=4), 0)
    } else {
        LinePower::new(rng.random_range(-2..=4), rng.random_range(1..=3))
    }
}

#[test]
fn the_box_kernel_equals_the_transverse_oracle_to_rank_8() {
    let c = context();
    let kernel = BubbleKernel::new(&c);
    let mut rng = StdRng::seed_from_u64(0x1b0c);
    let mut d_values = dimensions(&mut rng, 4);
    d_values.push(rational(37, 10));
    for round in 0..40 {
        let d = &d_values[round % d_values.len()];
        let variables = rng.random_range(1..=3_usize);
        let powers: Vec<u32> = (0..variables).map(|_| rng.random_range(0..=4)).collect();
        let rank: u32 = powers.iter().sum();
        if rank > 8 {
            continue;
        }
        let (alpha, beta) = (random_line_power(&mut rng), random_line_power(&mut rng));
        let (outer, oracle) = transverse_oracle(variables, &factor_list(&powers), d);

        // Oracle: (P^2 exponent, outer monomial) -> G values.
        let (a, b): (Power, Power) = (
            power(alpha.integer, alpha.eps_units),
            power(beta.integer, beta.eps_units),
        );
        let mut expected: BTreeMap<(i32, Vec<u32>), Vec<GValue>> = BTreeMap::new();
        for (monomial, value) in &oracle {
            let g = g(
                (a.0 - i64::from(monomial.a), a.1),
                (b.0 - i64::from(monomial.b), b.1),
                d,
            );
            expected
                .entry((
                    monomial.a + monomial.b + monomial.p2,
                    monomial.outer.clone(),
                ))
                .or_default()
                .push(g.scale(value));
        }

        // Kernel: the same polynomial from the pairings.
        let tag = tag_value(&relative_tag(alpha.eps_units, beta.eps_units).unwrap(), d);
        let mut actual: BTreeMap<(i32, Vec<u32>), Vec<GValue>> = BTreeMap::new();
        for term in kernel
            .numerator_terms(alpha, beta, &powers, algebra())
            .unwrap()
        {
            let mut monomial = outer.unit().outer;
            for &(s, t, count) in &term.pairing.pairs {
                monomial[outer.pair(usize::from(s), usize::from(t))] += count;
            }
            for (s, &rest) in term.pairing.rest.iter().enumerate() {
                monomial[outer.residual(s)] += rest;
            }
            actual
                .entry((term.sigma as i32, monomial))
                .or_default()
                .push(tag.scale(&at(&term.weight, d)));
        }

        let keys: BTreeSet<_> = expected.keys().chain(actual.keys()).cloned().collect();
        for key in keys {
            assert_values_equal(
                actual.get(&key).map_or(&[], Vec::as_slice),
                expected.get(&key).map_or(&[], Vec::as_slice),
                &format!("round {round}: {alpha:?}, {beta:?}, {powers:?}, monomial {key:?}"),
            );
        }
    }
}

#[test]
fn box_pairings_count_the_matchings() {
    // Prod x_s^{m_s} has r! / ((r - 2 sigma)! sigma! 2^sigma) matchings of order
    // sigma; the weights carry 2^-sigma on top.
    for (powers, sigma, matchings) in [
        (vec![2], 1, 1),
        (vec![4], 1, 6),
        (vec![4], 2, 3),
        (vec![1, 1], 1, 1),
        (vec![2, 2], 1, 6),
        (vec![2, 2], 2, 3),
        (vec![3, 2, 1], 3, 15),
    ] {
        let total = box_pairings(&powers, sigma)
            .iter()
            .fold(Rational::zero(), |sum, pairing| &sum + &pairing.weight);
        assert_eq!(
            total,
            rational(matchings, 1 << sigma),
            "{powers:?}, {sigma}"
        );
    }
    // x^3 y at order 1: 3 x^2-pairs of x.x with x y left, 3 x.y with x^2 left.
    let pairings = box_pairings(&[3, 1], 1);
    assert_eq!(pairings.len(), 2);
    assert!(pairings.iter().all(|p| p.weight == rational(3, 2)));
}

// ---------------------------------------------------------------------------
// Two-loop series-parallel nodes
// ---------------------------------------------------------------------------

/// One 2L node: a bubble over a 1L child with lines `k^2`, `(k - q)^2`.
struct TwoLoopNode {
    name: &'static str,
    parent: Arc<Presentation>,
    roles: BubbleRoles,
    edge: (EdgeMap, Arc<Presentation>),
}

impl TwoLoopNode {
    fn new(
        name: &'static str,
        slots: Vec<Slot>,
        roles: BubbleRoles,
        child_eps: [u8; 2],
        line_slots: &[Option<u16>],
        merged_slot: Option<u16>,
    ) -> Self {
        let parent = presentation(0, 2, slots);
        let child = one_loop(1, child_eps);
        let edge = insertion_edge(
            &parent,
            &child,
            roles.loop_index,
            loop_map(&[1], &[0], 1),
            line_slots,
            merged_slot,
        );
        Self {
            name,
            parent,
            roles,
            edge,
        }
    }

    fn step(&self) -> SeriesParallelStep {
        let step = SeriesParallelStep::new(
            self.parent.clone(),
            self.roles,
            std::slice::from_ref(&self.edge),
        )
        .unwrap();
        step.with_fold([one_loop_step(&self.edge.1)])
    }

    fn reduce(&self, powers: &[i64]) -> PStepOut {
        self.step()
            .apply(&key_view(&self.parent, powers, &GammaTag::one()), &limits())
            .unwrap()
    }
}

const KEPT_AFTER_BUBBLE: [Option<u16>; 5] = [None, None, Some(0), Some(1), None];

/// `d26`: two bubbles in series (kite without the middle line), the merged
/// line is absorbed.
fn d26() -> TwoLoopNode {
    TwoLoopNode::new(
        "d26",
        vec![
            line(&[1, 0, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 1, 0]),
        ],
        FIRST_BUBBLE,
        [0, 0],
        &KEPT_AFTER_BUBBLE,
        None,
    )
}

/// `d22`: `2-3 2-3 1-2 1-3`, the bubble `l^2, (l - P)^2`, `P = k - q`, in series
/// with `(k - q)^2`.
fn d22() -> TwoLoopNode {
    TwoLoopNode::new(
        "d22",
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
        FIRST_BUBBLE,
        [0, 1],
        &KEPT_AFTER_BUBBLE,
        Some(1),
    )
}

/// `d22` with both bubble lines and the ISP's `l` read backwards.
fn d22_flipped() -> TwoLoopNode {
    TwoLoopNode::new(
        "d22 flipped",
        vec![
            line(&[-1, 0, 0]),
            line(&[-1, 1, -1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[-1, 0, 0], &[0, 0, 1]),
        ],
        FIRST_BUBBLE,
        [0, 1],
        &KEPT_AFTER_BUBBLE,
        Some(1),
    )
}

/// `d22` with the ISP `(k + q).(-l)`: `l` on the right, a mixed `Y`, a sign.
fn d22_mixed_isp() -> TwoLoopNode {
    TwoLoopNode::new(
        "d22 mixed ISP",
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[0, 1, 1], &[-1, 0, 0]),
        ],
        FIRST_BUBBLE,
        [0, 1],
        &KEPT_AFTER_BUBBLE,
        Some(1),
    )
}

/// `d9`: the sunset; `k^2` is not a line, so it is an ISP free of `l`.
fn d9() -> TwoLoopNode {
    TwoLoopNode::new(
        "d9",
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
            isp(&[0, 1, 0], &[0, 1, 0]),
        ],
        FIRST_BUBBLE,
        [1, 0],
        &[None, None, Some(1), None, None],
        Some(0),
    )
}

/// `d11`: the sunset with an eps line, ISPs `l.q` and `k.q`.
fn d11() -> TwoLoopNode {
    TwoLoopNode::new(
        "d11",
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            eps_line(&[0, 1, -1], 1),
            isp(&[1, 0, 0], &[0, 0, 1]),
            isp(&[0, 1, 0], &[0, 0, 1]),
        ],
        FIRST_BUBBLE,
        [1, 1],
        &[None, None, Some(1), None, None],
        Some(0),
    )
}

/// `d13`: two bubbles in series, one eps line; the integer bubble goes first and
/// its merged line, absorbed, is an eps line.
fn d13_integer_bubble() -> TwoLoopNode {
    TwoLoopNode::new(
        "d13",
        vec![
            eps_line(&[1, 0, 0], 1),
            line(&[1, 0, -1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 1, 0]),
        ],
        BubbleRoles {
            lines: [2, 3],
            loop_index: 1,
        },
        [1, 0],
        &[Some(0), Some(1), None, None, None],
        None,
    )
}

/// `d13` integrating the eps bubble instead: an absorbed line with two units.
fn d13_eps_bubble() -> TwoLoopNode {
    TwoLoopNode::new(
        "d13'",
        vec![
            eps_line(&[1, 0, 0], 1),
            line(&[1, 0, -1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 1, 0]),
        ],
        FIRST_BUBBLE,
        [0, 0],
        &KEPT_AFTER_BUBBLE,
        None,
    )
}

/// `d14`, `d15`, `d17`: bubble `l^2, (l - k)^2` in series with `k^2`, parallel to
/// `(k - q)^2`, with one eps line on the parallel line, in the bubble, or on the
/// series line.
fn nested(name: &'static str, eps: [u8; 3], child_eps: [u8; 2]) -> TwoLoopNode {
    TwoLoopNode::new(
        name,
        vec![
            eps_line(&[1, 0, 0], eps[0]),
            line(&[1, -1, 0]),
            eps_line(&[0, 1, 0], eps[1]),
            eps_line(&[0, 1, -1], eps[2]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
        FIRST_BUBBLE,
        child_eps,
        &KEPT_AFTER_BUBBLE,
        Some(0),
    )
}

fn d14() -> TwoLoopNode {
    nested("d14", [0, 0, 1], [1, 1])
}

fn d15() -> TwoLoopNode {
    nested("d15", [1, 0, 0], [2, 0])
}

fn d17() -> TwoLoopNode {
    nested("d17", [0, 1, 0], [2, 0])
}

type Euclidean2 = BTreeMap<(i32, i32), Rational>;

fn euclidean_mul(x: &Euclidean2, y: &Euclidean2) -> Euclidean2 {
    let mut product = Euclidean2::new();
    for (left, u) in x {
        for (right, v) in y {
            let entry = product
                .entry((left.0 + right.0, left.1 + right.1))
                .or_insert_with(Rational::zero);
            *entry = &*entry + &(u * v);
        }
    }
    product.retain(|_, value| !value.is_zero());
    product
}

/// `x.y` for `x`, `y` over `(k, q)`, in `D1 = k^2`, `D2 = (k - q)^2`, Euclidean `q^2 = 1`.
fn euclidean_dot(x: (i64, i64), y: (i64, i64)) -> Euclidean2 {
    let mixed = rational(x.0 * y.1 + x.1 * y.0, 2);
    let mut value = Euclidean2::new();
    let mut add = |monomial, term: Rational| {
        let entry = value.entry(monomial).or_insert_with(Rational::zero);
        *entry = &*entry + &term;
    };
    add((1, 0), integer(x.0 * y.0));
    add((1, 0), mixed.clone());
    add((0, 0), mixed.clone());
    add((0, 1), -mixed);
    add((0, 0), integer(x.1 * y.1));
    value.retain(|_, term| !term.is_zero());
    value
}

fn euclidean_power(x: &Euclidean2, exponent: u32) -> Euclidean2 {
    (0..exponent).fold(
        Euclidean2::from([((0, 0), Rational::one())]),
        |product, _| euclidean_mul(&product, x),
    )
}

fn add_power(a: Power, b: Power) -> Power {
    (a.0 + b.0, a.1 + b.1)
}

/// `I_E` of a 2L key, nested: the transverse oracle on the inner bubble, the
/// outer scalars in the lines `k^2`, `(k - q)^2`, then one G-function, all
/// Euclidean with `q^2 = 1`. No kernel code is involved.
fn nested_euclidean(
    p: &Presentation,
    roles: BubbleRoles,
    powers: &[i64],
    d: &Rational,
) -> Vec<GValue> {
    let l = usize::from(roles.loop_index);
    let o = 1 - l;
    let outer = |row: &[i64]| (row[o], row[2]);
    let eps = eps_units(p);
    let [first, second] = roles.lines.map(usize::from);
    let alpha = power(powers[first], eps[first]);
    let beta = power(powers[second], eps[second]);
    let Slot::Line(second_line) = &p.slots[second] else {
        panic!("the bubble lines are lines");
    };
    let s1 = second_line.momentum[l];
    let bubble = (-s1 * second_line.momentum[o], -s1 * second_line.momentum[2]);

    let (mut a, mut b): (Power, Power) = ((0, 0), (0, 0));
    let mut vectors = Vec::new();
    let mut box_powers = Vec::new();
    let mut prefactor = Euclidean2::from([((0, 0), Rational::one())]);
    for (index, slot) in p.slots.iter().enumerate() {
        if index == first || index == second {
            continue;
        }
        match slot {
            Slot::Line(line) => {
                let line_power = power(powers[index], eps[index]);
                match outer(&line.momentum) {
                    (1, 0) | (-1, 0) => a = add_power(a, line_power),
                    (1, -1) | (-1, 1) => b = add_power(b, line_power),
                    other => panic!("no outer line {other:?}"),
                }
            }
            Slot::Isp(isp) => {
                let m = u32::try_from(-powers[index]).unwrap();
                match (isp.left[l], isp.right[l]) {
                    (0, 0) => {
                        let dot = euclidean_dot(outer(&isp.left), outer(&isp.right));
                        prefactor = euclidean_mul(&prefactor, &euclidean_power(&dot, m));
                    }
                    (sign, 0) => {
                        let row = outer(&isp.right);
                        vectors.push((sign * row.0, sign * row.1));
                        box_powers.push(m);
                    }
                    (0, sign) => {
                        let row = outer(&isp.left);
                        vectors.push((sign * row.0, sign * row.1));
                        box_powers.push(m);
                    }
                    _ => panic!("ISP slot {index}"),
                }
            }
        }
    }
    let variables = vectors.len().max(1);
    if vectors.is_empty() {
        vectors.push((0, 0));
        box_powers.push(0);
    }
    let (names, oracle) = transverse_oracle(variables, &factor_list(&box_powers), d);

    let merged = (alpha.0 + beta.0, alpha.1 + beta.1 - 1);
    let mut values = Vec::new();
    for (monomial, value) in &oracle {
        let inner = g(
            (alpha.0 - i64::from(monomial.a), alpha.1),
            (beta.0 - i64::from(monomial.b), beta.1),
            d,
        )
        .scale(value);
        if inner.is_zero() {
            continue;
        }
        let mut poly = prefactor.clone();
        for s in 0..variables {
            for t in s..variables {
                let exponent = monomial.outer[names.pair(s, t)];
                let dot = euclidean_dot(vectors[s], vectors[t]);
                poly = euclidean_mul(&poly, &euclidean_power(&dot, exponent));
            }
            let exponent = monomial.outer[names.residual(s)];
            let dot = euclidean_dot(bubble, vectors[s]);
            poly = euclidean_mul(&poly, &euclidean_power(&dot, exponent));
        }
        // (P^2)^(d/2 - alpha - beta + shift) is a line power merged - shift.
        let shift = i64::from(monomial.a + monomial.b + monomial.p2);
        let merged = (merged.0 - shift, merged.1);
        let (a, b) = match bubble {
            (1, 0) | (-1, 0) => (add_power(a, merged), b),
            (1, -1) | (-1, 1) => (a, add_power(b, merged)),
            (0, 1) | (0, -1) => (a, b),
            other => panic!("no merged line {other:?}"),
        };
        for ((e1, e2), coefficient) in &poly {
            let outer = g((a.0 - i64::from(*e1), a.1), (b.0 - i64::from(*e2), b.1), d);
            values.push(inner.mul(&outer).scale(coefficient));
        }
    }
    values
}

/// `I_M = (-1)^{Sum n} I_E` of every slot (C1), compared with the reduced key.
fn check_node_against_nested_forms(node: &TwoLoopNode, powers: &[i64], d: &Rational) {
    let output = node.reduce(powers);
    let total: i64 = powers.iter().sum();
    let expected: Vec<GValue> = nested_euclidean(&node.parent, node.roles, powers, d)
        .iter()
        .map(|value| value.scale(&sign(total)))
        .collect();
    assert_values_equal(
        &terminal_values(&output, d),
        &expected,
        &format!("{} {powers:?} at d = {d}", node.name),
    );
}

/// The node's top sector with `numerators` on the last slot, and with one dot.
fn dotted_keys(node: &TwoLoopNode, numerators: i64) -> Vec<Vec<i64>> {
    let slots = &node.parent.slots;
    let lines = slots
        .iter()
        .filter(|slot| matches!(slot, Slot::Line(_)))
        .count();
    let mut top: Vec<i64> = (0..slots.len()).map(|i| i64::from(i < lines)).collect();
    top[slots.len() - 1] = -numerators;
    let mut keys = vec![top.clone()];
    for line in 0..lines {
        let mut dotted = top.clone();
        dotted[line] = 2;
        keys.push(dotted);
    }
    keys
}

/// A random key of a 2L node: the bubble at positive powers, or any power on an
/// eps line; other lines anywhere; numerators up to `rank`.
fn random_powers(node: &TwoLoopNode, rng: &mut StdRng, rank: i64) -> Vec<i64> {
    let eps = eps_units(&node.parent);
    let mut left = rank;
    node.parent
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| match slot {
            Slot::Line(_) if eps[index] != 0 => rng.random_range(-2..=3),
            Slot::Line(_) if node.roles.lines.contains(&(index as u16)) => rng.random_range(1..=3),
            Slot::Line(_) if rng.random_bool(0.8) => rng.random_range(1..=3),
            Slot::Line(_) => rng.random_range(-1..=0),
            Slot::Isp(_) => {
                let numerator = rng.random_range(0..=left);
                left -= numerator;
                -numerator
            }
        })
        .collect()
}

#[test]
fn eps_nodes_with_odd_rank_equal_nested_euclidean_forms() {
    let mut rng = StdRng::seed_from_u64(0x2c13);
    let points = dimensions(&mut rng, 5);
    for node in [
        d11(),
        d13_integer_bubble(),
        d13_eps_bubble(),
        d14(),
        d15(),
        d17(),
    ] {
        // Odd rank on purpose: the Wick ratio of C2 is what these pin.
        for numerators in [1, 3, 5] {
            for powers in dotted_keys(&node, numerators) {
                for d in &points {
                    check_node_against_nested_forms(&node, &powers, d);
                }
            }
        }
        for _ in 0..12 {
            let powers = random_powers(&node, &mut rng, 5);
            check_node_against_nested_forms(&node, &powers, &points[0]);
        }
    }
}

#[test]
fn integer_series_parallel_nodes_equal_nested_euclidean_forms() {
    let mut rng = StdRng::seed_from_u64(0x2c22);
    let points = dimensions(&mut rng, 3);
    for node in [d22(), d22_flipped(), d26(), d9()] {
        let mut keys: Vec<Vec<i64>> = (0..=3).flat_map(|rank| dotted_keys(&node, rank)).collect();
        keys.extend((0..12).map(|_| random_powers(&node, &mut rng, 5)));
        for powers in &keys {
            for d in &points {
                check_node_against_nested_forms(&node, powers, d);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance (d): the 2L SP nodes equal reductions of the same kite sectors
// ---------------------------------------------------------------------------

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

/// A presentation family under the P0 lane, values of its masters by [`GValue`].
struct ReductionLane {
    lane: DynamicLiteRedSession,
    masters: Vec<IntegralKey>,
    values: PFamily,
}

impl ReductionLane {
    fn new(p: &Presentation) -> Self {
        let root: Vec<bool> = p.slots.iter().map(|s| matches!(s, Slot::Line(_))).collect();
        let options = LiteRedOptions {
            workers: workers(),
            ..LiteRedOptions::default()
        };
        let (mut lane, _) = DynamicLiteRedSession::generate(
            p.family.clone(),
            &root,
            workers(),
            true,
            options,
            None,
        )
        .unwrap();
        lane.compress(&[]).unwrap();
        let masters = lane.masters();
        // ISP rows are never read: masters have no numerators (asserted below).
        let width = usize::from(p.loops) + 1;
        let lines: Vec<&'static [i64]> = p
            .slots
            .iter()
            .map(|slot| -> &'static [i64] {
                match slot {
                    Slot::Line(line) => Box::leak(line.momentum.clone().into_boxed_slice()),
                    Slot::Isp(_) => Box::leak(vec![0; width].into_boxed_slice()),
                }
            })
            .collect();
        let values = PFamily {
            name: "presentation",
            loops: usize::from(p.loops),
            lines: Box::leak(lines.into_boxed_slice()),
            root: Box::leak(root.into_boxed_slice()),
        };
        Self {
            lane,
            masters,
            values,
        }
    }

    /// `I_M` of `targets` at `d`: reduction onto the masters, whose values are
    /// `I_M = (-1)^{Sum n} I_E`.
    fn values(&mut self, targets: &[(Vec<i64>, Rational)], d: &Rational) -> Vec<GValue> {
        let keys: Vec<IntegralKey> = targets
            .iter()
            .map(|(powers, _)| IntegralKey::try_new(powers.clone()).unwrap())
            .collect();
        let ids = self.lane.add_targets(&keys).unwrap();
        assert_eq!(self.lane.statistics(&ids).uncovered, 0);
        let reduction = self
            .lane
            .reduce_exact_at_point(std::slice::from_ref(d), &ids)
            .unwrap();
        let mut values = Vec::new();
        for ((_, weight), row) in targets.iter().zip(&reduction.coefficients) {
            for (master, coefficient) in self.masters.iter().zip(row) {
                if coefficient.is_zero() {
                    continue;
                }
                let powers = master.powers();
                let value = self
                    .values
                    .value(master, d)
                    .unwrap_or_else(|| panic!("the master {powers:?} has no G-function value"));
                let total: i64 = powers.iter().sum();
                values.push(value.scale(&(&sign(total) * &(coefficient * weight))));
            }
        }
        values
    }
}

/// `Prod_slots D_slot^{-powers}` of `parent` in the slots of `target`, with the
/// parent's loops replaced by `substitution` (rows over the target's momenta) and
/// `q -> q`. A line at a positive power must be a target line.
fn transport(
    parent: &Presentation,
    target: &Presentation,
    substitution: &[Vec<i64>],
    powers: &[i64],
) -> Vec<(Vec<i64>, Rational)> {
    let width = usize::from(target.loops) + 1;
    let image = |row: &[i64]| -> Vec<i64> {
        let mut out = vec![0; width];
        for (entry, source) in row.iter().zip(substitution) {
            for (value, c) in out.iter_mut().zip(source) {
                *value += entry * c;
            }
        }
        out[width - 1] += row[row.len() - 1];
        out
    };
    let base = vec![0_i64; target.slots.len()];
    let mut terms: BTreeMap<Vec<i64>, Rational> = BTreeMap::from([(base, Rational::one())]);
    for (slot, &exponent) in parent.slots.iter().zip(powers) {
        let (left, right) = match slot {
            Slot::Line(line) => (&line.momentum, &line.momentum),
            Slot::Isp(isp) => (&isp.left, &isp.right),
        };
        let form = target
            .express_scalar_product(&image(left), &image(right))
            .unwrap();
        let single =
            form.constant.is_zero() && form.terms.len() == 1 && form.terms[0].1 == Rational::one();
        if single {
            let position = usize::from(form.terms[0].0);
            terms = terms
                .into_iter()
                .map(|(mut key, value)| {
                    key[position] += exponent;
                    (key, value)
                })
                .collect();
            continue;
        }
        assert!(exponent <= 0, "{slot:?} at power {exponent}");
        for _ in 0..-exponent {
            let mut next: BTreeMap<Vec<i64>, Rational> = BTreeMap::new();
            for (key, value) in &terms {
                let mut add = |key: Vec<i64>, term: Rational| {
                    let entry = next.entry(key).or_insert_with(Rational::zero);
                    *entry = &*entry + &term;
                };
                if !form.constant.is_zero() {
                    add(key.clone(), value * &form.constant);
                }
                for (position, factor) in &form.terms {
                    let mut lowered = key.clone();
                    lowered[usize::from(*position)] -= 1;
                    add(lowered, value * factor);
                }
            }
            terms = next;
        }
    }
    terms.into_iter().filter(|(_, v)| !v.is_zero()).collect()
}

fn kite() -> Arc<Presentation> {
    presentation(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[0, 1, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, -1]),
            line(&[1, -1, 0]),
        ],
    )
}

#[test]
fn the_five_two_loop_sp_nodes_equal_kite_sector_reductions() {
    let kite = kite();
    let mut lane = ReductionLane::new(&kite);
    let mut rng = StdRng::seed_from_u64(0x2d26);
    let points = dimensions(&mut rng, 30);
    // The node's loops as kite momenta.
    type Node = (TwoLoopNode, Vec<Vec<i64>>);
    let nodes: [Node; 5] = [
        (d26(), vec![vec![1, 0, 0], vec![0, 1, 0]]),
        // l = k1 - q, k = k2: lines k1 - q, k1 - k2, k2, k2 - q of the kite.
        (d22(), vec![vec![1, 0, -1], vec![0, 1, 0]]),
        (d22_flipped(), vec![vec![1, 0, -1], vec![0, 1, 0]]),
        (d22_mixed_isp(), vec![vec![1, 0, -1], vec![0, 1, 0]]),
        // l = k1, k = k2: lines k1, k1 - k2, k2 - q.
        (d9(), vec![vec![1, 0, 0], vec![0, 1, 0]]),
    ];
    for (node, substitution) in &nodes {
        // Dots and numerators, odd rank included, and lines at power 0.
        let mut keys: Vec<Vec<i64>> = (0..=3).flat_map(|rank| dotted_keys(node, rank)).collect();
        for _ in 0..6 {
            let mut powers = random_powers(node, &mut rng, 4);
            // Lines of the node with a positive power must stay kite lines.
            for (slot, power) in node.parent.slots.iter().zip(&mut powers) {
                if matches!(slot, Slot::Line(_)) {
                    *power = (*power).max(0);
                }
            }
            keys.push(powers);
        }
        let mut nonzero = 0;
        for d in &points {
            for powers in &keys {
                let reduced = node.reduce(powers);
                let values = lane.values(&transport(&node.parent, &kite, substitution, powers), d);
                nonzero += usize::from(!collect(values.iter().cloned()).is_empty());
                assert_values_equal(
                    &terminal_values(&reduced, d),
                    &values,
                    &format!("{} {powers:?} at d = {d}", node.name),
                );
            }
        }
        // The fixed keys at least: sector tops with numerators do not vanish.
        assert!(nonzero >= 5 * points.len(), "{}: {nonzero}", node.name);
    }

    // The 1L nodes d3 and d4 of the same DAG: d3 against the kite's own lane is
    // the bubble family, d4 is reached from d22 and d9 above and closed here.
    let bubble = one_loop(0, [0, 0]);
    let mut lane = ReductionLane::new(&bubble);
    let step = one_loop_step(&bubble);
    for d in &points {
        for powers in [[1, 1], [2, 1], [3, 2], [1, 4]] {
            let output = step
                .apply(&key_view(&bubble, &powers, &GammaTag::one()), &limits())
                .unwrap();
            let values = lane.values(&[(powers.to_vec(), Rational::one())], d);
            assert_values_equal(&terminal_values(&output, d), &values, "d3");
        }
    }
    let eps_bubble = one_loop(0, [0, 1]);
    let step = one_loop_step(&eps_bubble);
    for d in &points {
        for (n1, n2) in [(1, 1), (2, 0), (1, -2), (3, 2)] {
            let output = step
                .apply(
                    &key_view(&eps_bubble, &[n1, n2], &GammaTag::one()),
                    &limits(),
                )
                .unwrap();
            let closed = g((n1, 0), power(n2, 1), d).scale(&sign(n1 + n2));
            assert_values_equal(&terminal_values(&output, d), &[closed], "d4");
        }
    }
}

// ---------------------------------------------------------------------------
// Acceptance (f): the carpet
// ---------------------------------------------------------------------------

/// The 2L carpet `l^2, (l - p)^2, p^2, (p - q)^2` with ISP `l.q`, loops `(l, p)`,
/// reachable both as a carpet of `p` and as an insertion of `l`.
struct TwoLoopCarpet {
    parent: Arc<Presentation>,
    carpet: CarpetStep,
    insertion: SeriesParallelStep,
    blob: Arc<InsertionStep>,
}

impl TwoLoopCarpet {
    /// `eps` = units of `l^2`, `(l - p)^2`, `p^2`, `(p - q)^2`.
    fn with_third(eps: [u8; 4]) -> Self {
        let parent = presentation(
            0,
            2,
            vec![
                eps_line(&[1, 0, 0], eps[0]),
                eps_line(&[1, -1, 0], eps[1]),
                eps_line(&[0, 1, 0], eps[2]),
                eps_line(&[0, 1, -1], eps[3]),
                isp(&[1, 0, 0], &[0, 0, 1]),
            ],
        );
        let roles = CarpetRoles {
            r: 3,
            third: Some(2),
            outer_loop: 1,
        };
        let merged = eps[0] + eps[1] + 1 + eps[2];
        Self::assemble(
            parent,
            roles,
            [eps[0], eps[1]],
            [merged, eps[3]],
            &[Some(0), Some(1), None, None, None],
            &KEPT_AFTER_BUBBLE,
        )
    }

    /// The same without `third`: `p^2` is an ISP, `eps` = units of `l^2`,
    /// `(l - p)^2`, `(p - q)^2`.
    fn without_third(eps: [u8; 3]) -> Self {
        let parent = presentation(
            0,
            2,
            vec![
                eps_line(&[1, 0, 0], eps[0]),
                eps_line(&[1, -1, 0], eps[1]),
                eps_line(&[0, 1, -1], eps[2]),
                isp(&[1, 0, 0], &[0, 0, 1]),
                isp(&[0, 1, 0], &[0, 1, 0]),
            ],
        );
        let roles = CarpetRoles {
            r: 2,
            third: None,
            outer_loop: 1,
        };
        Self::assemble(
            parent,
            roles,
            [eps[0], eps[1]],
            [eps[0] + eps[1] + 1, eps[2]],
            &[Some(0), Some(1), None, None, None],
            &[None, None, Some(1), None, None],
        )
    }

    fn assemble(
        parent: Arc<Presentation>,
        roles: CarpetRoles,
        blob_eps: [u8; 2],
        outer_eps: [u8; 2],
        blob_slots: &[Option<u16>],
        outer_slots: &[Option<u16>],
    ) -> Self {
        let blob = one_loop(1, blob_eps);
        let outer = one_loop(2, outer_eps);
        let carpet_edge = carpet_edge(&parent, &blob, 1, loop_map(&[1], &[0], 1), blob_slots);
        let carpet = CarpetStep::new(parent.clone(), roles, &[carpet_edge]).unwrap();
        let insertion_edge = insertion_edge(
            &parent,
            &outer,
            0,
            loop_map(&[1], &[0], 1),
            outer_slots,
            Some(0),
        );
        let insertion = SeriesParallelStep::new(parent.clone(), FIRST_BUBBLE, &[insertion_edge])
            .unwrap()
            .with_fold([one_loop_step(&outer)]);
        Self {
            blob: one_loop_step(&blob),
            parent,
            carpet,
            insertion,
        }
    }

    /// `(tag, weight)` of the key along carpet + bubble and along two bubbles.
    fn both_paths(&self, powers: &[i64]) -> [BTreeMap<GammaTag, Coefficient>; 2] {
        let c = self.parent.family.coefficient_context();
        let tag = GammaTag::one();
        let view = key_view(&self.parent, powers, &tag);
        let collect =
            |sum: &mut BTreeMap<GammaTag, Coefficient>, out: PStepOut, factor: &Coefficient| {
                for (_, tag, weight) in out.terminals {
                    let weight = c.try_mul(&weight, factor, algebra()).unwrap();
                    let entry = sum.entry(tag).or_insert_with(|| c.zero());
                    *entry = c.try_add(entry, &weight, algebra()).unwrap();
                }
            };
        let mut through_carpet = BTreeMap::new();
        for child in self.carpet.apply(&view, &limits()).unwrap().children {
            assert_eq!(child.stage, PStage::Route);
            let output = self
                .blob
                .apply(
                    &PKeyView {
                        presentation: child.presentation,
                        powers: &child.powers,
                        tag: &child.tag,
                    },
                    &limits(),
                )
                .unwrap();
            collect(&mut through_carpet, output, &child.coefficient);
        }
        let mut through_bubbles = BTreeMap::new();
        collect(
            &mut through_bubbles,
            self.insertion.apply(&view, &limits()).unwrap(),
            &c.one(),
        );
        through_carpet.retain(|_, weight: &mut Coefficient| !weight.is_zero());
        through_bubbles.retain(|_, weight: &mut Coefficient| !weight.is_zero());
        [through_carpet, through_bubbles]
    }
}

#[test]
fn the_carpet_equals_two_insertions_on_a_two_loop_carpet() {
    let mut rng = StdRng::seed_from_u64(0x2ca9);
    let carpets = [
        TwoLoopCarpet::with_third([0, 0, 0, 0]),
        TwoLoopCarpet::with_third([0, 0, 1, 0]),
        TwoLoopCarpet::with_third([1, 0, 0, 1]),
        TwoLoopCarpet::with_third([0, 2, 1, 1]),
    ];
    for carpet in &carpets {
        let eps = eps_units(&carpet.parent);
        for rank in 0..=6 {
            for _ in 0..6 {
                let n = |units: i64, rng: &mut StdRng| {
                    if units == 0 {
                        rng.random_range(1..=3)
                    } else {
                        rng.random_range(-1..=3)
                    }
                };
                let third = if eps[2] == 0 {
                    rng.random_range(-1..=3)
                } else {
                    n(eps[2], &mut rng)
                };
                let powers = [
                    n(eps[0], &mut rng),
                    n(eps[1], &mut rng),
                    third,
                    n(eps[3], &mut rng),
                    -rank,
                ];
                let [left, right] = carpet.both_paths(&powers);
                assert_eq!(left, right, "{eps:?} at {powers:?}");
            }
        }
    }
    // third absent: p^2 is a q-free numerator and counts in the blob's exponent.
    for carpet in [
        TwoLoopCarpet::without_third([0, 0, 0]),
        TwoLoopCarpet::without_third([1, 1, 1]),
    ] {
        let eps = eps_units(&carpet.parent);
        for rank in 0..=5 {
            for free in 0..=3 {
                let n = |units: i64, rng: &mut StdRng| {
                    if units == 0 {
                        rng.random_range(1..=3)
                    } else {
                        rng.random_range(-1..=3)
                    }
                };
                let powers = [
                    n(eps[0], &mut rng),
                    n(eps[1], &mut rng),
                    n(eps[2], &mut rng),
                    -rank,
                    -free,
                ];
                let [left, right] = carpet.both_paths(&powers);
                assert_eq!(left, right, "{eps:?} at {powers:?}");
            }
        }
    }
}

/// A 3L carpet whose blob is the kite of `(k1, k2; p)`.
struct ThreeLoopCarpet {
    name: &'static str,
    parent: Arc<Presentation>,
    step: CarpetStep,
}

impl ThreeLoopCarpet {
    /// `d121`: `third` present, lines `r, third, kite`, ISPs `k1.q`, `k2.q`.
    fn d121() -> Self {
        let parent = presentation(
            0,
            3,
            vec![
                line(&[0, 0, 1, -1]),
                line(&[0, 0, 1, 0]),
                line(&[1, 0, 0, 0]),
                line(&[0, 1, 0, 0]),
                line(&[1, 0, -1, 0]),
                line(&[0, 1, -1, 0]),
                line(&[1, -1, 0, 0]),
                isp(&[1, 0, 0, 0], &[0, 0, 0, 1]),
                isp(&[0, 0, 0, 1], &[0, 1, 0, 0]),
            ],
        );
        let roles = CarpetRoles {
            r: 0,
            third: Some(1),
            outer_loop: 2,
        };
        let slots = [
            None,
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
            None,
        ];
        Self::assemble("d121", parent, roles, &slots)
    }

    /// `d60`: `third` absent, lines `r, kite`, ISPs `k1.q`, `k2.q`, `p.p`.
    fn d60() -> Self {
        let parent = presentation(
            0,
            3,
            vec![
                line(&[0, 0, 1, -1]),
                line(&[1, 0, 0, 0]),
                line(&[0, 1, 0, 0]),
                line(&[1, 0, -1, 0]),
                line(&[0, 1, -1, 0]),
                line(&[1, -1, 0, 0]),
                isp(&[1, 0, 0, 0], &[0, 0, 0, 1]),
                isp(&[0, 0, 0, 1], &[0, 1, 0, 0]),
                isp(&[0, 0, 1, 0], &[0, 0, 1, 0]),
            ],
        );
        let roles = CarpetRoles {
            r: 0,
            third: None,
            outer_loop: 2,
        };
        let slots = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            None,
            None,
            None,
        ];
        Self::assemble("d60", parent, roles, &slots)
    }

    fn assemble(
        name: &'static str,
        parent: Arc<Presentation>,
        roles: CarpetRoles,
        line_slots: &[Option<u16>],
    ) -> Self {
        let child = {
            let kite = kite();
            let mut child = (*kite).clone();
            child.id = PresentationId(1);
            child
        };
        let edge = carpet_edge(
            &parent,
            &child,
            2,
            loop_map(&[1, 0, 0, 1], &[0, 0], 1),
            line_slots,
        );
        let step = CarpetStep::new(parent.clone(), roles, &[edge]).unwrap();
        Self { name, parent, step }
    }
}

#[test]
fn carpet_steps_on_d121_and_d60_equal_sector_reductions() {
    let kite = kite();
    let mut blob = ReductionLane::new(&kite);
    let mut rng = StdRng::seed_from_u64(0x3c60);
    let points = dimensions(&mut rng, 10);
    for carpet in [ThreeLoopCarpet::d121(), ThreeLoopCarpet::d60()] {
        let mut lane = ReductionLane::new(&carpet.parent);
        let arity = carpet.parent.slots.len();
        let lines = carpet
            .parent
            .slots
            .iter()
            .filter(|slot| matches!(slot, Slot::Line(_)))
            .count();
        let top: Vec<i64> = (0..arity).map(|i| i64::from(i < lines)).collect();
        // (l.q)^k with k >= 2, mixed, q-free numerators, dots, a blob line at 0.
        let mut keys = vec![top.clone()];
        let mut with = |edits: &[(usize, i64)]| {
            let mut powers = top.clone();
            for &(slot, value) in edits {
                powers[slot] = value;
            }
            keys.push(powers);
        };
        let (first_isp, second_isp) = (lines, lines + 1);
        with(&[(first_isp, -1)]);
        with(&[(first_isp, -2)]);
        with(&[(second_isp, -3)]);
        with(&[(first_isp, -2), (second_isp, -2)]);
        with(&[(0, 2), (first_isp, -2)]);
        with(&[(lines - 1, 2), (second_isp, -2)]);
        with(&[(lines - 1, 0), (first_isp, -2)]);
        with(&[(2, 2), (first_isp, -1), (second_isp, -1)]);
        if lines == 6 {
            // d60: p.p numerators, the q-free ISP of a third-absent carpet.
            with(&[(8, -1)]);
            with(&[(8, -2), (first_isp, -2)]);
        } else {
            // d121: third at a dot and at power 0.
            with(&[(1, 2), (first_isp, -2)]);
            with(&[(1, 0), (second_isp, -2)]);
        }
        let mut nonzero = 0;
        for d in &points {
            for powers in &keys {
                let tag = GammaTag::one();
                let output = carpet
                    .step
                    .apply(&key_view(&carpet.parent, powers, &tag), &limits())
                    .unwrap();
                assert!(output.terminals.is_empty());
                nonzero += usize::from(!output.children.is_empty());
                let mut actual = Vec::new();
                for child in &output.children {
                    assert_eq!(child.presentation, PresentationId(1));
                    let factor = tag_value(&child.tag, d).scale(&at(&child.coefficient, d));
                    let values = blob.values(&[(child.powers.clone(), Rational::one())], d);
                    actual.extend(values.iter().map(|value| value.mul(&factor)));
                }
                let expected = lane.values(&[(powers.clone(), Rational::one())], d);
                assert_values_equal(
                    &actual,
                    &expected,
                    &format!("{} {powers:?} at d = {d}", carpet.name),
                );
            }
        }
        assert_eq!(nonzero, keys.len() * points.len(), "{}", carpet.name);
    }
}

// ---------------------------------------------------------------------------
// A 3L chain, and acceptance (g): linear denominators, tags within L + 1
// ---------------------------------------------------------------------------

/// Three nested bubbles: `l, l - k` on `k`, `k - p` on `p`, `p - q`; integrating `l`
/// and then `k` merges into `k^2` and `p^2`.
struct ThreeLoopChain {
    parent: Arc<Presentation>,
    top: InsertionStep,
    middle: Arc<InsertionStep>,
    bottom: Arc<InsertionStep>,
}

fn three_loop_chain() -> ThreeLoopChain {
    let parent = presentation(
        0,
        3,
        vec![
            line(&[1, 0, 0, 0]),
            line(&[1, -1, 0, 0]),
            line(&[0, 1, 0, 0]),
            line(&[0, 1, -1, 0]),
            line(&[0, 0, 1, 0]),
            line(&[0, 0, 1, -1]),
            isp(&[1, 0, 0, 0], &[0, 0, 1, 0]),
            isp(&[1, 0, 0, 0], &[0, 0, 0, 1]),
            isp(&[0, 1, 0, 0], &[0, 0, 0, 1]),
        ],
    );
    let middle = presentation(
        1,
        2,
        vec![
            eps_line(&[1, 0, 0], 1),
            line(&[1, -1, 0]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    );
    let bottom = one_loop(2, [2, 0]);
    let top_edge = insertion_edge(
        &parent,
        &middle,
        0,
        loop_map(&[1, 0, 0, 1], &[0, 0], 1),
        &[
            None,
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            None,
            None,
            None,
        ],
        Some(0),
    );
    let middle_edge = insertion_edge(
        &middle,
        &bottom,
        0,
        loop_map(&[1], &[0], 1),
        &KEPT_AFTER_BUBBLE,
        Some(0),
    );
    ThreeLoopChain {
        top: InsertionStep::new(parent.clone(), FIRST_BUBBLE, &[top_edge]).unwrap(),
        middle: Arc::new(InsertionStep::new(middle, FIRST_BUBBLE, &[middle_edge]).unwrap()),
        bottom: one_loop_step(&bottom),
        parent,
    }
}

fn assert_linear_denominator(weight: &Coefficient) {
    for (factor, _) in weight.denominator.factor() {
        assert!(
            factor.degree(0) <= 1,
            "denominator factor of degree > 1 in {weight:?}"
        );
    }
}

#[test]
fn a_three_loop_chain_equals_its_reduction_and_stays_in_range() {
    let chain = three_loop_chain();
    let mut lane = ReductionLane::new(&chain.parent);
    let step = SeriesParallelStep::from(three_loop_chain().top)
        .with_fold([chain.middle.clone(), chain.bottom.clone()]);
    let mut rng = StdRng::seed_from_u64(0x3cba);
    let points = dimensions(&mut rng, 4);
    let keys: [[i64; 9]; 6] = [
        [1, 1, 1, 1, 1, 1, 0, 0, 0],
        [2, 1, 1, 1, 1, 1, -1, 0, 0],
        [1, 1, 1, 2, 1, 1, 0, -2, 0],
        [1, 1, 1, 1, 1, 2, -1, -1, -1],
        [1, 1, 0, 1, 1, 1, -2, 0, -1],
        [1, 2, 1, 1, 2, 1, -1, -2, -2],
    ];
    for powers in &keys {
        let output = step
            .apply(
                &key_view(&chain.parent, powers, &GammaTag::one()),
                &limits(),
            )
            .unwrap();
        for (_, tag, weight) in &output.terminals {
            assert!(tag.exponents().iter().all(|&(m, _)| m.unsigned_abs() <= 4));
            assert_linear_denominator(weight);
        }
        for d in &points {
            let values = lane.values(&[(powers.to_vec(), Rational::one())], d);
            assert_values_equal(
                &terminal_values(&output, d),
                &values,
                &format!("chain {powers:?} at d = {d}"),
            );
        }
    }
}

#[test]
fn weights_have_linear_denominators_and_tags_stay_within_l_plus_one() {
    // One-step outputs: Q(d) sums of the orders keep linear denominators.
    let mut rng = StdRng::seed_from_u64(0x7a95);
    // (node, loops of the DAG that holds it): eps nodes need one more loop.
    let nodes = [
        (d22(), 2),
        (d26(), 2),
        (d9(), 2),
        (d11(), 3),
        (d13_eps_bubble(), 3),
        (d17(), 3),
    ];
    for (node, dag_loops) in nodes {
        let step = SeriesParallelStep::new(
            node.parent.clone(),
            node.roles,
            std::slice::from_ref(&node.edge),
        )
        .unwrap();
        for _ in 0..10 {
            let powers = random_powers(&node, &mut rng, 6);
            let tag = GammaTag::one();
            let output = step
                .apply(&key_view(&node.parent, &powers, &tag), &limits())
                .unwrap();
            for child in &output.children {
                assert_linear_denominator(&child.coefficient);
                assert!(
                    child
                        .tag
                        .exponents()
                        .iter()
                        .all(|&(m, _)| m.unsigned_abs() <= dag_loops + 1),
                    "{} {powers:?}: tag {:?}",
                    node.name,
                    child.tag
                );
            }
        }
    }
    // Carpets: the tag of d121 is T_ins(2, 0): |m| <= 4 = L + 1.
    let carpet = ThreeLoopCarpet::d121();
    let tag = GammaTag::one();
    let powers = [1, 1, 1, 1, 1, 1, 1, -3, -3];
    let output = carpet
        .step
        .apply(&key_view(&carpet.parent, &powers, &tag), &limits())
        .unwrap();
    assert!(!output.children.is_empty());
    for child in &output.children {
        assert_eq!(child.tag, relative_tag(2, 0).unwrap());
        assert!(child.tag.exponents().iter().all(|&(m, _)| m.abs() <= 4));
        assert_linear_denominator(&child.coefficient);
    }
}

// ---------------------------------------------------------------------------
// Acceptance (h): memoisation
// ---------------------------------------------------------------------------

/// Two applications of each rank-8 key: the second one only hits the memos.
fn memoised_rank_8_step(node: &TwoLoopNode, powers: &[i64]) -> std::time::Duration {
    let step = InsertionStep::new(
        node.parent.clone(),
        node.roles,
        std::slice::from_ref(&node.edge),
    )
    .unwrap();
    let tag = GammaTag::one();
    let view = key_view(&node.parent, powers, &tag);
    let cold = step.apply(&view, &limits()).unwrap();
    let (weights, tables) = step.memo_counters();
    assert_eq!((weights.hits, tables.hits), (0, 0));
    assert_eq!(weights.misses, 5, "orders 0..=4");
    assert_eq!(tables.misses, 1);
    let start = Instant::now();
    let warm = step.apply(&view, &limits()).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(cold, warm);
    let (weights, tables) = step.memo_counters();
    assert_eq!((weights.hits, weights.misses), (5, 5));
    assert_eq!((tables.hits, tables.misses), (1, 1));
    elapsed
}

#[test]
fn rank_8_steps_hit_the_memos() {
    let mut powers = [1, 1, 1, 1, 0];
    powers[4] = -8;
    memoised_rank_8_step(&d22(), &powers);
}

#[test]
fn steps_are_pure_across_threads() {
    let node = d22();
    let new_step = || {
        InsertionStep::new(
            node.parent.clone(),
            node.roles,
            std::slice::from_ref(&node.edge),
        )
        .unwrap()
    };
    let tag = GammaTag::one();
    let keys: Vec<Vec<i64>> = (0..=6).flat_map(|rank| dotted_keys(&node, rank)).collect();
    let run = |step: &InsertionStep, powers: &[i64]| {
        step.apply(&key_view(&node.parent, powers, &tag), &limits())
            .unwrap()
    };
    let expected: Vec<PStepOut> = {
        let step = new_step();
        keys.iter().map(|powers| run(&step, powers)).collect()
    };
    let shared = new_step();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for (powers, output) in keys.iter().zip(&expected) {
                    assert_eq!(run(&shared, powers), *output);
                }
            });
        }
    });
}

#[test]
#[ignore = "slow: timing of the memoised rank-8 steps, depends on the load of the host"]
fn a_memoised_rank_8_step_takes_less_than_a_millisecond() {
    let c = context();
    let kernel = BubbleKernel::new(&c);
    let (alpha, beta) = (LinePower::new(1, 0), LinePower::new(1, 0));
    kernel
        .numerator_terms(alpha, beta, &[8], algebra())
        .unwrap();
    let mut best = std::time::Duration::MAX;
    for _ in 0..20 {
        let start = Instant::now();
        kernel
            .numerator_terms(alpha, beta, &[8], algebra())
            .unwrap();
        best = best.min(start.elapsed());
    }
    println!("rank-8 1L bubble kernel, memoised: {best:?}");
    assert!(best < std::time::Duration::from_millis(1));

    let mut powers = [1, 1, 1, 1, 0];
    powers[4] = -8;
    let step = (0..20)
        .map(|_| memoised_rank_8_step(&d22(), &powers))
        .min()
        .unwrap();
    println!("rank-8 2L insertion step (d22 -> d4), memoised: {step:?}");
}

// ---------------------------------------------------------------------------
// Edge maps: the reduction does not depend on the child's routing
// ---------------------------------------------------------------------------

#[test]
fn insertion_steps_do_not_depend_on_the_child_routing() {
    let node = d22();
    let parent = &node.parent;
    // k = q - k', swapping the child lines.
    let swapped_child = one_loop(1, [1, 0]);
    let swapped = TwoLoopNode {
        edge: insertion_edge(
            parent,
            &swapped_child,
            0,
            loop_map(&[-1], &[1], 1),
            &[None, None, Some(1), Some(0), None],
            Some(0),
        ),
        ..d22()
    };
    // q = -q', child lines k^2, (k + q')^2.
    let reversed_child = presentation(1, 1, vec![line(&[1, 0]), eps_line(&[1, 1], 1)]);
    let reversed = TwoLoopNode {
        edge: insertion_edge(
            parent,
            &reversed_child,
            0,
            loop_map(&[1], &[0], -1),
            &KEPT_AFTER_BUBBLE,
            Some(1),
        ),
        ..d22()
    };
    let mut rng = StdRng::seed_from_u64(0x7071);
    for _ in 0..20 {
        let powers = random_powers(&node, &mut rng, 5);
        let reduced = node.reduce(&powers);
        assert_eq!(reduced, swapped.reduce(&powers), "{powers:?}");
        assert_eq!(reduced, reversed.reduce(&powers), "{powers:?}");
    }

    // A 3L edge exchanging the outer loops: (k, p) -> (k'[1], k'[0]).
    let chain = three_loop_chain();
    let middle = presentation(
        1,
        2,
        vec![
            eps_line(&[0, 1, 0], 1),
            line(&[-1, 1, 0]),
            line(&[1, 0, 0]),
            line(&[1, 0, -1]),
            isp(&[0, 1, 0], &[0, 0, 1]),
        ],
    );
    let bottom = one_loop(2, [2, 0]);
    let roles = BubbleRoles {
        lines: [0, 1],
        loop_index: 1,
    };
    let middle_edge = insertion_edge(
        &middle,
        &bottom,
        1,
        loop_map(&[1], &[0], 1),
        &KEPT_AFTER_BUBBLE,
        Some(0),
    );
    let top_edge = insertion_edge(
        &chain.parent,
        &middle,
        0,
        loop_map(&[0, 1, 1, 0], &[0, 0], 1),
        &[
            None,
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            None,
            None,
            None,
        ],
        Some(0),
    );
    let exchanged = SeriesParallelStep::new(chain.parent.clone(), FIRST_BUBBLE, &[top_edge])
        .unwrap()
        .with_fold([
            Arc::new(InsertionStep::new(middle, roles, &[middle_edge]).unwrap()),
            one_loop_step(&bottom),
        ]);
    let straight =
        SeriesParallelStep::from(chain.top).with_fold([chain.middle.clone(), chain.bottom.clone()]);
    let tag = GammaTag::one();
    for powers in [
        [1, 2, 1, 1, 2, 1, -1, -2, -2],
        [2, 1, 1, 3, 1, 1, -3, -1, -1],
    ] {
        let view = key_view(&chain.parent, &powers, &tag);
        assert_eq!(
            straight.apply(&view, &limits()),
            exchanged.apply(&view, &limits()),
            "{powers:?}"
        );
    }
}

#[test]
fn carpet_steps_do_not_depend_on_the_child_routing() {
    let carpet = TwoLoopCarpet::with_third([0, 0, 1, 0]);
    let roles = CarpetRoles {
        r: 3,
        third: Some(2),
        outer_loop: 1,
    };
    let terminals = |edge: (EdgeMap, Arc<Presentation>), powers: &[i64]| {
        let blob = one_loop_step(&edge.1);
        let step = CarpetStep::new(carpet.parent.clone(), roles, &[edge]).unwrap();
        let tag = GammaTag::one();
        let c = carpet.parent.family.coefficient_context();
        let mut sum: BTreeMap<GammaTag, Coefficient> = BTreeMap::new();
        for child in step
            .apply(&key_view(&carpet.parent, powers, &tag), &limits())
            .unwrap()
            .children
        {
            let view = PKeyView {
                presentation: child.presentation,
                powers: &child.powers,
                tag: &child.tag,
            };
            for (_, tag, weight) in blob.apply(&view, &limits()).unwrap().terminals {
                let weight = c.try_mul(&weight, &child.coefficient, algebra()).unwrap();
                let entry = sum.entry(tag).or_insert_with(|| c.zero());
                *entry = c.try_add(entry, &weight, algebra()).unwrap();
            }
        }
        sum.retain(|_, weight| !weight.is_zero());
        sum
    };
    let line_slots = [Some(0), Some(1), None, None, None];
    let straight = carpet_edge(
        &carpet.parent,
        &one_loop(1, [0, 0]),
        1,
        loop_map(&[1], &[0], 1),
        &line_slots,
    );
    // l = q' - l', p = q': the blob lines swap.
    let swapped = carpet_edge(
        &carpet.parent,
        &one_loop(1, [0, 0]),
        1,
        loop_map(&[-1], &[1], 1),
        &[Some(1), Some(0), None, None, None],
    );
    // p = -q': blob lines l^2, (l + q')^2.
    let reversed_child = presentation(1, 1, vec![line(&[1, 0]), line(&[1, 1])]);
    let reversed = carpet_edge(
        &carpet.parent,
        &reversed_child,
        1,
        loop_map(&[1], &[0], -1),
        &line_slots,
    );
    for powers in [
        [1, 1, 1, 1, -1],
        [2, 1, 2, 1, -3],
        [1, 2, 1, 2, -4],
        [3, 2, -1, 1, -5],
    ] {
        let expected = terminals(straight.clone(), &powers);
        assert!(!expected.is_empty());
        assert_eq!(expected, carpet.both_paths(&powers)[1], "{powers:?}");
        assert_eq!(expected, terminals(swapped.clone(), &powers), "{powers:?}");
        assert_eq!(expected, terminals(reversed.clone(), &powers), "{powers:?}");
    }
}

// ---------------------------------------------------------------------------
// Constructors and keys fail closed; the trait and owner entry points
// ---------------------------------------------------------------------------

fn assert_contract<T: std::fmt::Debug>(result: Result<T, PError>) {
    assert!(matches!(result, Err(PError::Contract(_))), "{result:?}");
}

#[test]
fn insertion_steps_refuse_what_their_roles_do_not_say() {
    let node = d26();
    let (edge, child) = node.edge.clone();
    let edges = |edge: EdgeMap| vec![(edge, child.clone())];
    let new = |roles, edges: &[(EdgeMap, Arc<Presentation>)]| {
        InsertionStep::new(node.parent.clone(), roles, edges)
    };
    new(FIRST_BUBBLE, &edges(edge.clone())).unwrap();

    // The second bubble is a valid role of d26, but this edge integrates the first.
    let other = BubbleRoles {
        lines: [2, 3],
        loop_index: 1,
    };
    check_insertion_contract(&node.parent, &other).unwrap();
    assert_contract(new(other, &edges(edge.clone())));

    // An edge map that does not verify.
    let wrong_slots = EdgeMap {
        line_slots: vec![None, None, Some(1), Some(0), None],
        ..edge.clone()
    };
    assert_contract(new(FIRST_BUBBLE, &edges(wrong_slots)));

    // The number of edges against the loops, and a bubble other than q at 1L.
    assert_contract(new(FIRST_BUBBLE, &[]));
    let two = edges(edge.clone());
    let doubled: Vec<_> = two.iter().chain(&two).cloned().collect();
    assert_contract(new(FIRST_BUBBLE, &doubled));
    let bubble = one_loop(1, [0, 0]);
    assert_contract(InsertionStep::new(bubble, FIRST_BUBBLE, &two));
    let two_q = presentation(0, 1, vec![line(&[1, 0]), line(&[1, -2])]);
    assert_contract(InsertionStep::new(two_q, FIRST_BUBBLE, &[]));

    // A carpet edge on an insertion step, and an insertion edge on a carpet.
    let carpet = TwoLoopCarpet::with_third([0, 0, 0, 0]);
    let blob = one_loop(1, [0, 0]);
    let wrong = carpet_edge(
        &carpet.parent,
        &blob,
        1,
        loop_map(&[1], &[0], 1),
        &[Some(0), Some(1), None, None, None],
    );
    assert_contract(InsertionStep::new(
        carpet.parent.clone(),
        FIRST_BUBBLE,
        std::slice::from_ref(&wrong),
    ));
    let roles = CarpetRoles {
        r: 3,
        third: Some(2),
        outer_loop: 1,
    };
    check_carpet_contract(&carpet.parent, &roles).unwrap();
    assert_contract(CarpetStep::new(carpet.parent.clone(), roles, &[]));
    let outer = one_loop(2, [1, 0]);
    let wrong = insertion_edge(
        &carpet.parent,
        &outer,
        0,
        loop_map(&[1], &[0], 1),
        &KEPT_AFTER_BUBBLE,
        Some(0),
    );
    assert_contract(CarpetStep::new(carpet.parent.clone(), roles, &[wrong]));
}

#[test]
fn carpet_steps_refuse_what_their_roles_do_not_say() {
    let carpet = TwoLoopCarpet::with_third([0, 0, 0, 0]);
    let blob = one_loop(1, [0, 0]);
    let roles = CarpetRoles {
        r: 3,
        third: Some(2),
        outer_loop: 1,
    };
    let edge = carpet_edge(
        &carpet.parent,
        &blob,
        1,
        loop_map(&[1], &[0], 1),
        &[Some(0), Some(1), None, None, None],
    );
    let new = |roles, edges: &[(EdgeMap, Arc<Presentation>)]| {
        CarpetStep::new(carpet.parent.clone(), roles, edges)
    };
    new(roles, std::slice::from_ref(&edge)).unwrap();

    // Without `third`, a step would drop the power of the line `p^2`.
    let no_third = CarpetRoles {
        third: None,
        ..roles
    };
    assert_contract(new(no_third, std::slice::from_ref(&edge)));

    // One outer loop has one edge.
    assert_contract(new(roles, &[edge.clone(), edge.clone()]));

    // An edge that keeps `third`: `p` is the blob's `q`, not one of its lines.
    let keeps_third = EdgeMap {
        line_slots: vec![Some(0), Some(1), Some(1), None, None],
        ..edge.0.clone()
    };
    assert_contract(new(roles, &[(keeps_third, edge.1.clone())]));
}

#[test]
fn steps_refuse_keys_that_are_not_theirs() {
    let node = d22();
    let step = InsertionStep::new(
        node.parent.clone(),
        node.roles,
        std::slice::from_ref(&node.edge),
    )
    .unwrap();
    let one = GammaTag::one();
    let run = |powers: &[i64], tag: &GammaTag| {
        step.apply(&key_view(&node.parent, powers, tag), &limits())
    };
    assert_contract(run(&[1, 1, 1], &one));
    assert_contract(step.apply(
        &PKeyView {
            presentation: PresentationId(9),
            powers: &[1, 1, 1, 1, 0],
            tag: &one,
        },
        &limits(),
    ));
    assert!(matches!(
        run(&[1, 1, 1, 1, 1], &one),
        Err(PError::Unsupported(detail)) if detail.contains("ISP slot 4")
    ));
    // T_ins(0, 0) holds Gamma(1 + eps): one more step leaves the key range.
    let full = GammaTag::from_pairs([(1, 127)]).unwrap();
    assert!(matches!(
        run(&[1, 1, 1, 1, 0], &full),
        Err(PError::TagOutOfRange)
    ));
    // Massless tadpoles vanish without a tag.
    assert_eq!(run(&[0, 1, 1, 1, -3], &full), Ok(PStepOut::default()));

    let carpet = TwoLoopCarpet::with_third([0, 0, 0, 0]);
    let view = |powers| key_view(&carpet.parent, powers, &one);
    assert_contract(carpet.carpet.apply(&view(&[1, 1]), &limits()));
    assert!(matches!(
        carpet.carpet.apply(&view(&[1, 1, 1, 1, 2]), &limits()),
        Err(PError::Unsupported(_))
    ));
    assert_eq!(
        carpet.carpet.apply(&view(&[1, 1, 1, 0, -2]), &limits()),
        Ok(PStepOut::default())
    );
}

#[test]
fn lanes_run_through_the_owner_and_the_trait() {
    let node = d22();
    let record = PClassRecord {
        key: OwnerClassKey {
            loops: 2,
            lines: 4,
            eps_lines: 0,
            canonical: Arc::from(&b"d22"[..]),
        },
        owner: (PresentationId(0), 0b1111),
        node: None,
        nodes: Vec::new(),
        t: 4,
        eps_lines: 0,
    };
    let class = record.key.id();
    let mut census = PCensus::default();
    census.insert_class(record).unwrap();
    let mut builder = PLibrary::builder(context(), 1..=2);
    builder.add_presentation((*node.parent).clone()).unwrap();
    builder.add_presentation((*node.edge.1).clone()).unwrap();
    let edge = builder.add_edge_map(node.edge.0.clone()).unwrap();
    let spec = POwnerSpec {
        class,
        lane: OwnerLane::SeriesParallel,
        presentation: PresentationId(0),
        node: None,
        roles: LaneRoles::Insertion(node.roles),
        edge_maps: vec![edge],
        payload: None,
        custom: None,
        terminals: Vec::new(),
    };
    builder
        .add_owner_with_program(spec.clone(), OwnerProgram::Missing { ledger: 0 })
        .unwrap();
    builder.set_census(census).unwrap();
    let library = builder.build().unwrap();

    let insertion = InsertionStep::from_owner(&library, &spec).unwrap();
    let series = SeriesParallelStep::from_owner(&library, &spec).unwrap();
    assert_eq!(insertion.lane(), OwnerLane::Insertion);
    assert_eq!(series.lane(), OwnerLane::SeriesParallel);
    assert_eq!(insertion.child().unwrap().id, PresentationId(1));
    assert_eq!(series.insertion().presentation().id, PresentationId(0));
    assert_contract(CarpetStep::from_owner(&library, &spec));

    let cx = PStepContext {
        library: &library,
        owner: library.owner(class).unwrap(),
        limits: &limits(),
    };
    let tag = GammaTag::from_pairs([(2, 1)]).unwrap();
    let powers = [2, 1, 1, 1, -3];
    let view = key_view(&node.parent, &powers, &tag);
    let by_trait = insertion.step(&cx, &view).unwrap();
    assert_eq!(by_trait, series.step(&cx, &view).unwrap());
    assert!(!by_trait.children.is_empty());
    let mut previous: Option<&Vec<i64>> = None;
    for child in &by_trait.children {
        assert_eq!(child.stage, PStage::Route);
        assert_eq!(child.presentation, PresentationId(1));
        assert_eq!(child.tag, tag.add(&relative_tag(0, 0).unwrap()));
        assert!(previous.is_none_or(|powers| powers > &child.powers));
        previous = Some(&child.powers);
    }
    assert!(
        ghat(LinePower::new(1, 0), LinePower::new(0, 0), 0, 0)
            .unwrap()
            .rational()
            .is_zero()
    );
}

#[test]
fn the_kernel_versions_are_pinned() {
    // Bump these with the semantics of the kernels, see the module docs.
    assert_eq!(P_INSERTION_KERNEL_VERSION, 1);
    assert_eq!(P_CARPET_KERNEL_VERSION, 1);
}
