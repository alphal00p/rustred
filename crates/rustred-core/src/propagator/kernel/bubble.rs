//! The one-loop bubble kernel shared by the Insertion and SeriesParallel lanes
//! (owner: M2-T4 rr-kernel). Ruijl's thesis eqs. 15-17, Forcer's `insertion` and
//! `reduceG` (`forcer-aux.h:890-1080`, `:1380-1423`).
//!
//! With `D = 4 - 2 eps` and `f` homogeneous of degree `n` in `k`:
//!
//! ```text
//! Int d^Dk / pi^(D/2)  f(k) / ((k^2)^alpha ((k - P)^2)^beta)
//!   = (P^2)^(D/2 - alpha - beta)
//!     Sum_{sigma = 0}^{floor(n / 2)} Ghat(alpha, beta, n, sigma) (P^2)^sigma / sigma! (Box / 4)^sigma f |_{k = P}
//! Ghat(alpha, beta, n, sigma) = Gamma(alpha + beta - sigma - D/2) Gamma(D/2 - alpha + n - sigma) Gamma(D/2 - beta + sigma)
//!                               / (Gamma(alpha) Gamma(beta) Gamma(D - alpha - beta + n))
//! ```
//!
//! `Ghat = T_ins(j1, j2) R_ins`: for `alpha = n1 + j1 eps`, `beta = n2 + j2 eps` the
//! tag [`relative_tag`] depends on the eps units alone and `R_ins`, [`ghat`], is
//! a product of linear factors. For `f = Prod_s (k.Y_s)^{m_s}`,
//! `(Box / 4)^sigma / sigma! f` is `2^-sigma` times the sum over all sets of
//! `sigma` disjoint pairs of the factors of `f`, each pair contracted to
//! `Y_s.Y_t`, every other factor evaluated to `P.Y_s`: [`box_pairings`].
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::propagator::error::PError;
use crate::propagator::tag::GammaTag;

use super::gamma::{FactoredRational, GammaProduct, factorial};
use super::{MEMO_CAPACITY, MemoCounters, read_memo, write_memo};

/// A power `integer + eps_units eps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinePower {
    pub integer: i64,
    pub eps_units: i64,
}

impl LinePower {
    pub const fn new(integer: i64, eps_units: i64) -> Self {
        Self { integer, eps_units }
    }

    /// An integer line at a power `<= 0`: `1 / Gamma(n <= 0) = 0`, the bubble is
    /// a massless tadpole.
    pub const fn vanishes(self) -> bool {
        self.eps_units == 0 && self.integer <= 0
    }
}

/// `Ghat(alpha, beta, n, sigma) = T_ins R_ins`, as a Gamma product.
pub fn ghat(alpha: LinePower, beta: LinePower, n: i64, sigma: i64) -> Result<GammaProduct, PError> {
    let (n1, j1) = (alpha.integer, alpha.eps_units);
    let (n2, j2) = (beta.integer, beta.eps_units);
    let mut product = GammaProduct::one();
    product.gamma(n1 + n2 - sigma - 2, 1 + j1 + j2)?;
    product.gamma(2 - n1 + n - sigma, -(1 + j1))?;
    product.gamma(2 - n2 + sigma, -(1 + j2))?;
    product.inverse_gamma(n1, j1)?;
    product.inverse_gamma(n2, j2)?;
    product.inverse_gamma(4 - n1 - n2 + n, -(2 + j1 + j2))?;
    Ok(product)
}

/// `T_ins(j1, j2)`: the tag of every [`ghat`] with these eps units.
pub fn relative_tag(j1: i64, j2: i64) -> Result<GammaTag, PError> {
    ghat(LinePower::new(1, j1), LinePower::new(1, j2), 0, 0)?.tag()
}

/// One term of `(Box / 4)^sigma / sigma! Prod_s x_s^{m_s}`:
/// `weight Prod (Y_s.Y_t)^c Prod_s (P.Y_s)^{rest_s}`, with `weight` the number of
/// sets of pairs giving this term, over `2^sigma`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxPairing {
    /// `(s, t, c)`, `s <= t`, `c >= 1`: `Y_s.Y_t` contracted `c` times.
    pub pairs: Vec<(u16, u16, u32)>,
    /// The uncontracted factors per variable.
    pub rest: Vec<u32>,
    pub weight: Rational,
}

fn power_of_two(n: u32) -> Rational {
    (0..n).fold(Rational::one(), |product, _| {
        &product * &Rational::from(2_i64)
    })
}

/// Every pairing of order `sigma` of `Prod_s x_s^{powers[s]}`, see [`BoxPairing`].
///
/// Of the `Prod_s m_s!` orderings, `Prod_s rest_s! Prod_{s<t} c_st! Prod_s c_ss! 2^c_ss`
/// give the same term, so the count of sets of pairs is their quotient.
pub fn box_pairings(powers: &[u32], sigma: u32) -> Vec<BoxPairing> {
    let mut variables = Vec::new();
    for s in 0..powers.len() {
        for t in s..powers.len() {
            variables.push((s, t));
        }
    }
    let mut out = Vec::new();
    let mut remaining = powers.to_vec();
    let mut pairs = Vec::new();
    collect_pairings(&variables, sigma, &mut remaining, &mut pairs, &mut out);
    let numerator = powers.iter().fold(Rational::one(), |product, &m| {
        &product * &factorial(i64::from(m))
    });
    for pairing in &mut out {
        let mut denominator = power_of_two(sigma);
        for &rest in &pairing.rest {
            denominator = &denominator * &factorial(i64::from(rest));
        }
        for &(s, t, count) in &pairing.pairs {
            denominator = &denominator * &factorial(i64::from(count));
            if s == t {
                denominator = &denominator * &power_of_two(count);
            }
        }
        pairing.weight = &numerator / &denominator;
    }
    out
}

fn collect_pairings(
    variables: &[(usize, usize)],
    left: u32,
    remaining: &mut Vec<u32>,
    pairs: &mut Vec<(u16, u16, u32)>,
    out: &mut Vec<BoxPairing>,
) {
    let Some((&(s, t), rest)) = variables.split_first() else {
        if left == 0 {
            out.push(BoxPairing {
                pairs: pairs.clone(),
                rest: remaining.clone(),
                weight: Rational::one(),
            });
        }
        return;
    };
    let most = if s == t {
        remaining[s] / 2
    } else {
        remaining[s].min(remaining[t])
    };
    for count in 0..=most.min(left) {
        remaining[s] -= count;
        remaining[t] -= count;
        if count > 0 {
            pairs.push((s as u16, t as u16, count));
        }
        collect_pairings(rest, left - count, remaining, pairs, out);
        if count > 0 {
            pairs.pop();
        }
        remaining[s] += count;
        remaining[t] += count;
    }
}

/// One term of [`BubbleKernel::numerator_terms`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubbleTerm {
    pub sigma: u32,
    pub pairing: BoxPairing,
    /// `R_ins(sigma)` times the pairing weight.
    pub weight: Coefficient,
}

type RatioKey = (i64, i64, i64, i64, i64, i64);

/// `R_ins` in `Q(d)`, memoised per `(j1, j2, n1, n2, n, sigma)`.
pub struct BubbleKernel {
    context: CoefficientContext,
    ratios: RwLock<HashMap<RatioKey, Coefficient>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl BubbleKernel {
    pub fn new(context: &CoefficientContext) -> Self {
        Self {
            context: context.clone(),
            ratios: RwLock::default(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub fn context(&self) -> &CoefficientContext {
        &self.context
    }

    /// `R_ins(alpha, beta, n, sigma)` in `Q(d)`, `Ghat` without its tag.
    pub fn ratio(
        &self,
        alpha: LinePower,
        beta: LinePower,
        n: i64,
        sigma: i64,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, PError> {
        let key = (
            alpha.eps_units,
            beta.eps_units,
            alpha.integer,
            beta.integer,
            n,
            sigma,
        );
        if let Some(value) = read_memo(&self.ratios, &key)? {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(value);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let value = ghat(alpha, beta, n, sigma)?
            .rational()
            .to_coefficient(&self.context, limits)?;
        write_memo(&self.ratios, key, &value, MEMO_CAPACITY)?;
        Ok(value)
    }

    pub fn counters(&self) -> MemoCounters {
        MemoCounters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    /// The bubble with numerator `Prod_s (k.Y_s)^{powers[s]}`: every term of the
    /// sum over `sigma` and the pairings, each to be multiplied by
    /// `(P^2)^(D/2 - alpha - beta + sigma)` and the tag `T_ins`.
    pub fn numerator_terms(
        &self,
        alpha: LinePower,
        beta: LinePower,
        powers: &[u32],
        limits: ExactAlgebraLimits,
    ) -> Result<Vec<BubbleTerm>, PError> {
        let rank: u32 = powers.iter().sum();
        let mut terms = Vec::new();
        for sigma in 0..=rank / 2 {
            let ratio = self.ratio(alpha, beta, i64::from(rank), i64::from(sigma), limits)?;
            if ratio.is_zero() {
                continue;
            }
            for pairing in box_pairings(powers, sigma) {
                let weight = FactoredRational::from_rational(pairing.weight.clone())
                    .to_coefficient(&self.context, limits)?;
                let weight = self
                    .context
                    .try_mul(&ratio, &weight, limits)
                    .map_err(super::gamma::algebra)?;
                terms.push(BubbleTerm {
                    sigma,
                    pairing,
                    weight,
                });
            }
        }
        Ok(terms)
    }
}

impl std::fmt::Debug for BubbleKernel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BubbleKernel")
            .field("counters", &self.counters())
            .finish_non_exhaustive()
    }
}
