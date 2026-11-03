//! Massless propagator-type ("p-integral") families and their exact
//! G-function values, shared by the p-integral tests.
//!
//! Lines are Euclidean, `q.q = 1`, and the measure is `d^dk / pi^(d/2)` per
//! loop. Values of recursively one-loop integrals are products of
//! `G(a, b) = Gamma(d/2-a) Gamma(d/2-b) Gamma(a+b-d/2)
//!            / (Gamma(a) Gamma(b) Gamma(d-a-b))`,
//! evaluated exactly at a rational `d` as `R * prod_m Gamma(m d/2)^e_m`
//! with rational `R` (see [`GValue`]): every Gamma argument has the form
//! `c + m d/2` with integer `c` and `m`, and `Gamma(c + m d/2)` is a
//! Pochhammer multiple of `Gamma(m d/2)`. Two sums of such values agree iff
//! their rational coefficients agree per signature `e` (for a `d` that keeps
//! the `Gamma(m d/2)` algebraically independent in practice, such as
//! `d = 37/10`).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::family::{IntegralFamily, IntegralKey};
use rustred::input::{Compiler, Limits, LoweringLimits};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

/// A p-integral family: the compact input and the line momenta over
/// `(k1, .., kL, q)`, in denominator order.
pub struct PFamily {
    pub name: &'static str,
    pub loops: usize,
    pub lines: &'static [&'static [i64]],
    /// Positions that may carry positive powers (ISPs are `false`).
    pub root: &'static [bool],
}

pub const BUBBLE: PFamily = PFamily {
    name: "p1l_bubble",
    loops: 1,
    lines: &[&[1, 0], &[1, -1]],
    root: &[true, true],
};

pub const KITE: PFamily = PFamily {
    name: "p2l_kite",
    loops: 2,
    lines: &[
        &[1, 0, 0],
        &[0, 1, 0],
        &[1, 0, -1],
        &[0, 1, -1],
        &[1, -1, 0],
    ],
    root: &[true; 5],
};

/// Mincer's ladder (LA), Benz (BE) and non-planar (NO) three-loop families,
/// each with one ISP in the last slot.
pub const LA: PFamily = PFamily {
    name: "p3l_la",
    loops: 3,
    lines: &[
        &[1, 0, 0, 0],
        &[0, 1, 0, 0],
        &[0, 0, 1, 0],
        &[1, 0, 0, -1],
        &[0, 1, 0, -1],
        &[0, 0, 1, -1],
        &[1, -1, 0, 0],
        &[0, 1, -1, 0],
        &[1, 0, -1, 0],
    ],
    root: &[true, true, true, true, true, true, true, true, false],
};

pub const BE: PFamily = PFamily {
    name: "p3l_be",
    loops: 3,
    lines: &[
        &[1, 0, 0, 0],
        &[1, 0, 0, -1],
        &[0, 1, 0, 0],
        &[1, -1, 0, 0],
        &[0, 0, 1, 0],
        &[-1, 0, -1, 1],
        &[1, -1, 1, 0],
        &[0, -1, 0, 1],
        &[0, 0, 1, -1],
    ],
    root: &[true, true, true, true, true, true, true, true, false],
};

pub const NO: PFamily = PFamily {
    name: "p3l_no",
    loops: 3,
    lines: &[
        &[1, 0, 0, 0],
        &[1, 0, 0, -1],
        &[0, 1, 0, 0],
        &[1, -1, 0, 0],
        &[0, 0, 1, 0],
        &[-1, 0, -1, 1],
        &[0, 1, 1, 0],
        &[0, -1, -1, 1],
        &[0, 0, 1, -1],
    ],
    root: &[true, true, true, true, true, true, true, true, false],
};

/// The four-loop ladder and the non-planar crossed-rung family (eleven
/// lines, three ISPs).
pub const LADDER_4L: PFamily = PFamily {
    name: "p4l_la",
    loops: 4,
    lines: &[
        &[-1, 1, 1, 1, 1],
        &[-1, 0, 1, 1, 1],
        &[-1, 0, 0, 1, 1],
        &[-1, 0, 0, 0, 1],
        &[1, -1, -1, -1, 0],
        &[1, 0, -1, -1, 0],
        &[1, 0, 0, -1, 0],
        &[1, 0, 0, 0, 0],
        &[0, 1, 0, 0, 0],
        &[0, 0, 1, 0, 0],
        &[0, 0, 0, 1, 0],
        &[1, -1, 0, 0, 0],
        &[1, 0, -1, 0, 0],
        &[0, 1, -1, 0, 0],
    ],
    root: &[
        true, true, true, true, true, true, true, true, true, true, true, false, false, false,
    ],
};

pub const CROSSED_4L: PFamily = PFamily {
    name: "p4l_np",
    loops: 4,
    lines: &[
        &[-1, 0, 1, 0, 1],
        &[0, -1, 1, 1, 1],
        &[0, -1, 0, 1, 1],
        &[0, -1, 0, 0, 1],
        &[1, 0, -1, 0, 0],
        &[1, 0, 0, 0, 0],
        &[0, 1, 0, -1, 0],
        &[0, 1, 0, 0, 0],
        &[-1, 1, 0, -1, 0],
        &[0, 0, 1, 0, 0],
        &[0, 0, 0, 1, 0],
        &[1, -1, 0, 0, 0],
        &[0, 1, -1, 0, 0],
        &[0, 0, 1, -1, 0],
    ],
    root: &[
        true, true, true, true, true, true, true, true, true, true, true, false, false, false,
    ],
};

impl PFamily {
    /// The compact `I(...)` input with `q.q = external_square`.
    pub fn source(&self, external_square: i64) -> String {
        let loops = (1..=self.loops)
            .map(|l| format!("k{l}"))
            .collect::<Vec<_>>();
        let props = self
            .lines
            .iter()
            .enumerate()
            .map(|(slot, row)| {
                let mut momentum = String::new();
                for (column, &c) in row.iter().enumerate() {
                    if c == 0 {
                        continue;
                    }
                    let name = if column < self.loops {
                        loops[column].as_str()
                    } else {
                        "q"
                    };
                    momentum += &format!("{c:+}*{name}");
                }
                format!("  prop(D{},({momentum})^2,1),\n", slot + 1)
            })
            .collect::<String>();
        format!(
            "I(\n  name({}),\n  loops({}),\n  externals(q),\n  dimension(d),\n{props}  gram(q,q,{external_square})\n)",
            self.name,
            loops.join(",")
        )
    }

    pub fn family(&self) -> Arc<IntegralFamily> {
        Arc::new(compile(&self.source(1)))
    }

    pub fn key(&self, powers: &[i64]) -> IntegralKey {
        assert_eq!(powers.len(), self.lines.len());
        IntegralKey::try_new(powers.to_vec()).unwrap()
    }

    /// Exact value of a recursively one-loop integral, `None` otherwise.
    pub fn value(&self, key: &IntegralKey, d: &Rational) -> Option<GValue> {
        let lines = self
            .lines
            .iter()
            .zip(key.powers())
            .filter(|(_, power)| **power != 0)
            .map(|(row, &power)| (canonical(row), (power, 0)))
            .fold(BTreeMap::new(), |mut lines, (row, (alpha, m))| {
                let entry = lines.entry(row).or_insert((0, 0));
                entry.0 += alpha;
                entry.1 += m;
                lines
            });
        integrate(lines, self.loops, d)
    }
}

pub fn compile(source: &str) -> IntegralFamily {
    Compiler::new(Limits::default())
        .unwrap()
        .compile_compact(source, None)
        .unwrap()
        .into_lowered(LoweringLimits::default())
        .unwrap()
        .into_family()
}

pub fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::from((Integer::from(numerator), Integer::from(denominator)))
}

/// `coefficient * prod_m Gamma(m d/2)^signature[m]` at one rational `d`.
#[derive(Clone, Debug, PartialEq)]
pub struct GValue {
    pub coefficient: Rational,
    pub signature: BTreeMap<i64, i64>,
}

impl GValue {
    pub fn rational(coefficient: Rational) -> Self {
        Self {
            coefficient,
            signature: BTreeMap::new(),
        }
    }

    pub fn zero() -> Self {
        Self::rational(Rational::zero())
    }

    pub fn is_zero(&self) -> bool {
        self.coefficient.is_zero()
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut signature = self.signature.clone();
        for (m, e) in &other.signature {
            *signature.entry(*m).or_insert(0) += e;
        }
        signature.retain(|_, e| *e != 0);
        Self {
            coefficient: &self.coefficient * &other.coefficient,
            signature,
        }
    }

    pub fn scale(&self, factor: &Rational) -> Self {
        Self {
            coefficient: &self.coefficient * factor,
            signature: self.signature.clone(),
        }
    }

    fn inverse(&self) -> Self {
        Self {
            coefficient: self.coefficient.inv(),
            signature: self.signature.iter().map(|(m, e)| (*m, -e)).collect(),
        }
    }
}

/// A sum of [`GValue`]s collected by signature, zero terms dropped.
pub fn collect(terms: impl IntoIterator<Item = GValue>) -> BTreeMap<Vec<(i64, i64)>, Rational> {
    let mut sum: BTreeMap<Vec<(i64, i64)>, Rational> = BTreeMap::new();
    for term in terms {
        if term.is_zero() {
            continue;
        }
        let key = term.signature.into_iter().collect::<Vec<_>>();
        let entry = sum.entry(key).or_insert_with(Rational::zero);
        *entry = &*entry + &term.coefficient;
    }
    sum.retain(|_, value| !value.is_zero());
    sum
}

/// `Gamma(c + m d/2)`; `None` at a pole.
fn gamma(c: i64, m: i64, d: &Rational) -> Option<GValue> {
    if m == 0 {
        if c <= 0 {
            return None;
        }
        let factorial = (1..c).fold(Integer::from(1), |product, j| product * Integer::from(j));
        return Some(GValue::rational(Rational::from(factorial)));
    }
    let x = &(d * &rational(m, 2));
    let mut coefficient = Rational::one();
    if c >= 0 {
        for j in 0..c {
            coefficient = &coefficient * &(x + &rational(j, 1));
        }
    } else {
        for j in 1..=-c {
            coefficient = &coefficient / &(x - &rational(j, 1));
        }
    }
    Some(GValue {
        coefficient,
        signature: BTreeMap::from([(m, 1)]),
    })
}

/// `1 / Gamma(c + m d/2)`, zero at a pole of Gamma.
fn inverse_gamma(c: i64, m: i64, d: &Rational) -> GValue {
    gamma(c, m, d).map_or_else(GValue::zero, |value| value.inverse())
}

/// A line power `alpha + m d/2`.
pub type Power = (i64, i64);

/// `G(a, b)` for line powers `a`, `b`.
pub fn g(a: Power, b: Power, d: &Rational) -> GValue {
    let numerator = [(-a.0, 1 - a.1), (-b.0, 1 - b.1), (a.0 + b.0, a.1 + b.1 - 1)];
    let denominator = [a, b, (-a.0 - b.0, 2 - a.1 - b.1)];
    let mut value = GValue::rational(Rational::one());
    for (c, m) in denominator {
        value = value.mul(&inverse_gamma(c, m, d));
    }
    if value.is_zero() {
        return value;
    }
    for (c, m) in numerator {
        value = value.mul(&gamma(c, m, d).expect("G-function pole at the chosen d"));
    }
    value
}

fn canonical(row: &[i64]) -> Vec<i64> {
    match row.iter().find(|c| **c != 0) {
        Some(c) if *c < 0 => row.iter().map(|c| -c).collect(),
        _ => row.to_vec(),
    }
}

/// Integrate one loop at a time where it runs through exactly two lines with
/// unit coefficients (a one-loop insertion), the remaining lines carrying
/// powers `alpha + m d/2`. `None` if no such loop exists before all loops are
/// gone; zero for a scaleless integral.
fn integrate(mut lines: BTreeMap<Vec<i64>, Power>, loops: usize, d: &Rational) -> Option<GValue> {
    let mut value = GValue::rational(Rational::one());
    let mut integrated = vec![false; loops];
    loop {
        lines.retain(|_, power| *power != (0, 0));
        let active: Vec<usize> = (0..loops)
            .filter(|&l| lines.keys().any(|row| row[l] != 0))
            .collect();
        if active.len() + integrated.iter().filter(|&&done| done).count() < loops {
            // A loop momentum without lines: scaleless.
            return Some(GValue::zero());
        }
        if active.is_empty() {
            // Only lines of momentum +-q remain: (q^2)^(-power) = 1.
            return lines
                .keys()
                .all(|row| row[..loops].iter().all(|c| *c == 0) && row[loops].abs() == 1)
                .then_some(value);
        }
        let mut progressed = false;
        for &l in &active {
            let hit: Vec<Vec<i64>> = lines.keys().filter(|row| row[l] != 0).cloned().collect();
            if hit.len() == 1 {
                return Some(GValue::zero());
            }
            if hit.len() != 2 || hit.iter().any(|row| row[l].abs() != 1) {
                continue;
            }
            let a = lines.remove(&hit[0]).unwrap();
            let b = lines.remove(&hit[1]).unwrap();
            // Orient both lines as k_l + r; the bubble momentum is r2 - r1.
            let first: Vec<i64> = hit[0].iter().map(|c| c * hit[0][l]).collect();
            let second: Vec<i64> = hit[1].iter().map(|c| c * hit[1][l]).collect();
            let momentum: Vec<i64> = second.iter().zip(&first).map(|(x, y)| x - y).collect();
            if momentum.iter().all(|c| *c == 0) {
                return Some(GValue::zero());
            }
            value = value.mul(&g(a, b, d));
            if value.is_zero() {
                return Some(value);
            }
            let entry = lines.entry(canonical(&momentum)).or_insert((0, 0));
            entry.0 += a.0 + b.0;
            entry.1 += a.1 + b.1 - 1;
            integrated[l] = true;
            progressed = true;
            break;
        }
        if !progressed {
            return None;
        }
    }
}
