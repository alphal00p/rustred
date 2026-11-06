//! The outer loop of a carpet (owner: M2-T4 rr-kernel). Ruijl's thesis eq. 18,
//! Forcer's `carpet` (`forcer-aux.h:1157-1360`).
//!
//! An outer loop `p` with lines `(p^2)^alpha`, `((p - q)^2)^beta` around a blob of
//! `L_sub` loops, lines `1 / (p_i^2)^(a_i)` and numerator `P_n({p_i}, q)` of degree
//! `n` in `q`, `A = Sum a_i` (every quadratic scalar of the blob counts, numerators
//! with negative exponents):
//!
//! ```text
//! (q^2)^(D/2 - alpha - beta) Sum_{sigma = 0}^{floor(n / 2)} (D/2 + n - sigma)_(-sigma)
//!   Ghat(alpha + A - L_sub D/2 - sigma, beta, n - 2 sigma, 0)
//!   Sum_{j = 0}^{floor(n / 2) - sigma} (-1)^j (D/2 + n - 2 sigma - 1)_(-j) (q^2)^(sigma + j)
//!   [ blob with numerator (Box_q / 4)^(sigma + j) P_n / (sigma! j!) ] at external momentum q
//! ```
//!
//! Only `s = sigma + j` reaches the numerator, so the double sum is one sum over
//! orders `s` of [`CarpetKernel::order_weight`], with `(Box_q / 4)^s / s!` as in
//! [`super::bubble::box_pairings`] and the binomial `s! / (sigma! j!)` inside the
//! weight. In the native Minkowski form of [`super`] the explicit `(-1)^j` is
//! replaced by `(-1)^sigma`; the sign `(-1)^{n_r + n_3}` of the removed lines stays
//! with the step.
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use symbolica::prelude::Rational;

use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::propagator::error::PError;
use crate::propagator::tag::GammaTag;

use super::bubble::{LinePower, ghat, relative_tag};
use super::gamma::{FactoredRational, algebra};
use super::{MEMO_CAPACITY, MemoCounters, read_memo, write_memo};

fn binomial(n: i64, k: i64) -> Rational {
    (1..=k).fold(Rational::one(), |product, i| {
        &(&product * &Rational::from(n - k + i)) / &Rational::from(i)
    })
}

/// The tag of a carpet: `T_ins(j_inner, j_r)`, `j_inner = j_3 + j_blob + L_sub`.
pub fn carpet_tag(inner: LinePower, r: LinePower) -> Result<GammaTag, PError> {
    relative_tag(inner.eps_units, r.eps_units)
}

/// One `(sigma, j)` term of an order, without the sign of the removed lines.
pub fn order_term(
    inner: LinePower,
    r: LinePower,
    rank: i64,
    sigma: i64,
    j: i64,
) -> Result<FactoredRational, PError> {
    let shifted = LinePower::new(inner.integer - sigma, inner.eps_units);
    let mut term = FactoredRational::from_rational(binomial(sigma + j, sigma));
    if sigma % 2 != 0 {
        term = term.mul(&FactoredRational::from_rational(Rational::from(-1_i64)));
    }
    term = term.mul(&FactoredRational::pochhammer(2 + rank - sigma, -1, -sigma)?);
    term = term.mul(&FactoredRational::pochhammer(1 + rank - 2 * sigma, -1, -j)?);
    Ok(term.mul(ghat(shifted, r, rank - 2 * sigma, 0)?.rational()))
}

type WeightKey = (i64, i64, i64, i64, i64, i64);

/// The weight of each order in `Q(d)`, memoised per
/// `(j_inner, j_r, n_inner, n_r, n, s)`.
pub struct CarpetKernel {
    context: CoefficientContext,
    weights: RwLock<HashMap<WeightKey, Coefficient>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CarpetKernel {
    pub fn new(context: &CoefficientContext) -> Self {
        Self {
            context: context.clone(),
            weights: RwLock::default(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// `Sum_{sigma + j = order} order_term(sigma, j)`, where
    /// `inner = alpha + A - L_sub D/2` at `sigma = 0`.
    pub fn order_weight(
        &self,
        inner: LinePower,
        r: LinePower,
        rank: i64,
        order: i64,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, PError> {
        let key = (
            inner.eps_units,
            r.eps_units,
            inner.integer,
            r.integer,
            rank,
            order,
        );
        if let Some(value) = read_memo(&self.weights, &key)? {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(value);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let mut total = self.context.zero();
        for sigma in 0..=order {
            let term = order_term(inner, r, rank, sigma, order - sigma)?
                .to_coefficient(&self.context, limits)?;
            total = self
                .context
                .try_add(&total, &term, limits)
                .map_err(algebra)?;
        }
        write_memo(&self.weights, key, &total, MEMO_CAPACITY)?;
        Ok(total)
    }

    pub fn counters(&self) -> MemoCounters {
        MemoCounters {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }
}

impl std::fmt::Debug for CarpetKernel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CarpetKernel")
            .field("counters", &self.counters())
            .finish_non_exhaustive()
    }
}
