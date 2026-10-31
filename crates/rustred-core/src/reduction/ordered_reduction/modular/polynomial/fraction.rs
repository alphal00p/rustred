//! Partial-fraction weights over the factor basis (`denominators=factorized`).
//!
//! Modulo a good prime (see `basis.rs`) a weight is stored as
//!
//! `P(d) + sum_f sum_{j=1..e_f} a_{f,j}(d) / f(d)^j`, with `deg a_{f,j} < deg f`,
//!
//! over the monic basis images `f`. Every weight's denominator divides a
//! product of basis powers and the images are pairwise coprime, so this
//! representation of an element of `Z_p(d)` is unique. Hence:
//!
//! - a sum is a coefficient-wise sum: no common denominator, no
//!   normalization; a cancelled pole disappears by itself, and a weight is
//!   zero exactly when all its coefficients are (exact zero tests);
//! - a product with a constant step coefficient scales every coefficient;
//! - a product with a step coefficient `N / prod g^e` (numerator already
//!   divided by the denominator's content and leading coefficients) is a
//!   combination of `w * d^i` (multiplication by `d` shifts each chain of
//!   coefficients by one power) followed by one division per factor `g`
//!   (closed recurrences when `g` and the chain's factor are linear,
//!   a small CRT step otherwise).
//!
//! The primes (lanes) of a job share one layout: which factors and powers
//! are present and the length of the polynomial part. A trailing coefficient
//! is trimmed only when it is zero in every lane, so a lane may carry
//! explicit zeros; values, zero tests and images are per lane and do not
//! depend on the other lanes of the job.
use std::collections::HashMap;

use super::super::field::{self, Fe, Poly};
use super::super::lift::{Image, image_from};
use super::super::{CoefRef, error};
use super::Representation;
use super::basis::{Basis, LaneTables};
use super::lane::{Lane, Scalar};
use crate::reduction::ReductionError;

/// All coefficients `a_{f,1..e}` of one basis factor `f`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chain {
    factor: u32,
    exponent: u32,
}

/// One weight for every lane of a job.
///
/// Per lane, `stride` coefficients: the polynomial part (`poly` ascending
/// coefficients), then every chain in factor order, each as `a_1, ..., a_e`
/// with `deg f` ascending coefficients per `a_j`. Lane `l` occupies
/// `data[l * stride..(l + 1) * stride]`. The default value is zero.
#[derive(Clone, Debug, Default)]
pub(super) struct Fraction {
    poly: usize,
    chains: Vec<Chain>,
    stride: usize,
    data: Vec<Fe>,
}

#[inline(always)]
fn zero() -> Fe {
    Fe::from_inner(0)
}

/// A per-lane factor of a scaled sum.
#[derive(Clone, Copy)]
enum Scale {
    One,
    Constant(usize),
    /// Coefficient of `d^power` in a polynomial coefficient's numerator.
    Numerator(usize, usize),
}

pub(super) struct Factorized<'a> {
    lanes: &'a [LaneTables],
    basis: &'a Basis,
    max_degree: usize,
    /// Scratch: `(source offset, destination offset, length)`.
    segments: Vec<(usize, usize, usize)>,
    /// `active * d^i` for `i = 1, 2, ...`, until the next pop.
    powers: Vec<Fraction>,
    /// `f^e` per lane, for the nonlinear paths.
    power_cache: Vec<HashMap<(u32, usize), Poly>>,
}

impl<'a> Factorized<'a> {
    pub fn new(lanes: &'a [LaneTables], basis: &'a Basis, max_degree: usize) -> Self {
        Self {
            lanes,
            basis,
            max_degree,
            segments: vec![],
            powers: vec![],
            power_cache: lanes.iter().map(|_| HashMap::new()).collect(),
        }
    }

    #[inline(always)]
    fn chain_len(&self, chain: Chain) -> usize {
        chain.exponent as usize * self.basis.degree(chain.factor)
    }

    /// A zero weight with the given layout (fails closed beyond `max_degree`).
    fn layout(&self, poly: usize, chains: Vec<Chain>) -> Result<Fraction, ReductionError> {
        let denominator: usize = chains.iter().map(|c| self.chain_len(*c)).sum();
        if denominator > self.max_degree || poly > self.max_degree + 1 {
            return Err(error(format!(
                "a modular_polynomial weight exceeds max_degree={}",
                self.max_degree
            )));
        }
        let stride = poly + denominator;
        Ok(Fraction {
            poly,
            chains,
            stride,
            data: vec![zero(); stride * self.lanes.len()],
        })
    }

    /// Segments placing `source`'s layout inside `destination`'s; false when
    /// `destination` does not cover `source`.
    fn segments(
        &self,
        destination: &Fraction,
        source: &Fraction,
        segments: &mut Vec<(usize, usize, usize)>,
    ) -> bool {
        segments.clear();
        if source.poly > destination.poly {
            return false;
        }
        let mut push = |from: usize, to: usize, len: usize| {
            if len == 0 {
                return;
            }
            match segments.last_mut() {
                Some(last) if last.0 + last.2 == from && last.1 + last.2 == to => last.2 += len,
                _ => segments.push((from, to, len)),
            }
        };
        push(0, 0, source.poly);
        let (mut from, mut to) = (source.poly, destination.poly);
        let mut chains = destination.chains.iter();
        'next: for chain in &source.chains {
            for candidate in chains.by_ref() {
                let size = self.chain_len(*candidate);
                if candidate.factor == chain.factor {
                    if candidate.exponent < chain.exponent {
                        return false;
                    }
                    let len = self.chain_len(*chain);
                    push(from, to, len);
                    from += len;
                    to += size;
                    continue 'next;
                }
                if candidate.factor > chain.factor {
                    return false;
                }
                to += size;
            }
            return false;
        }
        true
    }

    /// Copy `source` into a zero weight with a covering layout.
    fn relayout(
        &mut self,
        source: &Fraction,
        poly: usize,
        chains: Vec<Chain>,
    ) -> Result<Fraction, ReductionError> {
        let mut result = self.layout(poly, chains)?;
        let mut segments = std::mem::take(&mut self.segments);
        let covered = self.segments(&result, source, &mut segments);
        debug_assert!(covered);
        for lane in 0..self.lanes.len() {
            let from = &source.data[lane * source.stride..(lane + 1) * source.stride];
            let to = &mut result.data[lane * result.stride..(lane + 1) * result.stride];
            for &(f, t, len) in &segments {
                to[t..t + len].copy_from_slice(&from[f..f + len]);
            }
        }
        self.segments = segments;
        Ok(result)
    }

    #[inline(always)]
    fn scalar(&self, lane: usize, scale: Scale) -> Option<Scalar> {
        let tables = &self.lanes[lane];
        match scale {
            Scale::One => None,
            Scale::Constant(c) => Some(tables.constant_scalars[c]),
            Scale::Numerator(c, power) => Some(
                tables.numerator_scalars[c]
                    .get(power)
                    .copied()
                    .unwrap_or_default(),
            ),
        }
    }

    /// `scale * source` with the layout of `source`.
    fn scaled(&self, source: &Fraction, scale: Scale) -> Fraction {
        let mut result = Fraction {
            poly: source.poly,
            chains: source.chains.clone(),
            stride: source.stride,
            data: vec![zero(); source.data.len()],
        };
        for lane in 0..self.lanes.len() {
            let range = lane * source.stride..(lane + 1) * source.stride;
            match self.scalar(lane, scale) {
                None => result.data[range.clone()].copy_from_slice(&source.data[range]),
                Some(s) => self.lanes[lane].lane.scale_into(
                    &mut result.data[range.clone()],
                    &source.data[range],
                    s,
                ),
            }
        }
        result
    }

    /// `destination += scale * source`.
    fn axpy(
        &mut self,
        destination: &mut Fraction,
        source: &Fraction,
        scale: Scale,
    ) -> Result<(), ReductionError> {
        if source.stride == 0 {
            return Ok(());
        }
        if destination.stride == 0 {
            *destination = self.scaled(source, scale);
            return Ok(());
        }
        let mut segments = std::mem::take(&mut self.segments);
        if !self.segments(destination, source, &mut segments) {
            let (poly, chains) = self.union(destination, source);
            self.segments = segments;
            *destination = self.relayout(destination, poly, chains)?;
            segments = std::mem::take(&mut self.segments);
            let covered = self.segments(destination, source, &mut segments);
            debug_assert!(covered);
        }
        let (ds, ss) = (destination.stride, source.stride);
        for lane in 0..self.lanes.len() {
            let to = &mut destination.data[lane * ds..(lane + 1) * ds];
            let from = &source.data[lane * ss..(lane + 1) * ss];
            let arithmetic = &self.lanes[lane].lane;
            match self.scalar(lane, scale) {
                None => {
                    for &(f, t, len) in &segments {
                        arithmetic.add_into(&mut to[t..t + len], &from[f..f + len]);
                    }
                }
                Some(s) => {
                    for &(f, t, len) in &segments {
                        arithmetic.axpy(&mut to[t..t + len], &from[f..f + len], s);
                    }
                }
            }
        }
        self.segments = segments;
        Ok(())
    }

    /// The smallest layout covering both weights.
    fn union(&self, a: &Fraction, b: &Fraction) -> (usize, Vec<Chain>) {
        let mut chains = Vec::with_capacity(a.chains.len() + b.chains.len());
        let (mut i, mut j) = (0, 0);
        while i < a.chains.len() || j < b.chains.len() {
            match (a.chains.get(i), b.chains.get(j)) {
                (Some(x), Some(y)) if x.factor == y.factor => {
                    chains.push(Chain {
                        factor: x.factor,
                        exponent: x.exponent.max(y.exponent),
                    });
                    i += 1;
                    j += 1;
                }
                (Some(x), Some(y)) if x.factor < y.factor => {
                    chains.push(*x);
                    i += 1;
                }
                (Some(x), None) => {
                    chains.push(*x);
                    i += 1;
                }
                (_, Some(y)) => {
                    chains.push(*y);
                    j += 1;
                }
                (None, None) => unreachable!(),
            }
        }
        (a.poly.max(b.poly), chains)
    }

    /// Drop trailing coefficients that are zero in every lane.
    fn trim(&self, weight: &mut Fraction) {
        if weight.stride == 0 {
            return;
        }
        let lanes = self.lanes.len();
        let stride = weight.stride;
        let nonzero_in_some_lane = |offset: usize, len: usize| {
            (0..lanes).any(|lane| {
                weight.data[lane * stride + offset..lane * stride + offset + len]
                    .iter()
                    .any(|c| !Lane::is_zero(c))
            })
        };
        let mut poly = weight.poly;
        while poly > 0 && !nonzero_in_some_lane(poly - 1, 1) {
            poly -= 1;
        }
        let mut chains = Vec::with_capacity(weight.chains.len());
        let mut offset = weight.poly;
        let mut changed = poly != weight.poly;
        for chain in &weight.chains {
            let degree = self.basis.degree(chain.factor);
            let mut exponent = chain.exponent as usize;
            while exponent > 0 && !nonzero_in_some_lane(offset + (exponent - 1) * degree, degree) {
                exponent -= 1;
            }
            changed |= exponent != chain.exponent as usize;
            if exponent > 0 {
                chains.push(Chain {
                    factor: chain.factor,
                    exponent: exponent as u32,
                });
            }
            offset += self.chain_len(*chain);
        }
        if !changed {
            return;
        }
        // Copy the kept prefixes.
        let new_stride: usize = poly + chains.iter().map(|c| self.chain_len(*c)).sum::<usize>();
        let mut data = vec![zero(); new_stride * lanes];
        for lane in 0..lanes {
            let from = &weight.data[lane * stride..(lane + 1) * stride];
            let to = &mut data[lane * new_stride..(lane + 1) * new_stride];
            to[..poly].copy_from_slice(&from[..poly]);
            let (mut f, mut t) = (weight.poly, poly);
            let mut kept = chains.iter().peekable();
            for chain in &weight.chains {
                if let Some(k) = kept.peek()
                    && k.factor == chain.factor
                {
                    let len = self.chain_len(**k);
                    to[t..t + len].copy_from_slice(&from[f..f + len]);
                    t += len;
                    kept.next();
                }
                f += self.chain_len(*chain);
            }
        }
        *weight = Fraction {
            poly,
            chains,
            stride: new_stride,
            data,
        };
    }

    /// `d * source`.
    fn mul_x(&self, source: &Fraction) -> Result<Fraction, ReductionError> {
        if source.stride == 0 {
            return Ok(Fraction::default());
        }
        let mut result = self.layout(source.poly + 1, source.chains.clone())?;
        for (lane, tables) in self.lanes.iter().enumerate() {
            let arithmetic = &tables.lane;
            let from = &source.data[lane * source.stride..(lane + 1) * source.stride];
            let to = &mut result.data[lane * result.stride..(lane + 1) * result.stride];
            to[1..=source.poly].copy_from_slice(&from[..source.poly]);
            let mut constant = zero();
            let mut offset = source.poly;
            for chain in &source.chains {
                let degree = self.basis.degree(chain.factor);
                let exponent = chain.exponent as usize;
                let a = &from[offset..offset + exponent * degree];
                let b = &mut to[offset + 1..offset + 1 + exponent * degree];
                if degree == 1 {
                    // a_j d / (d - r)^j = a_j / (d - r)^(j-1) + r a_j / (d - r)^j
                    let r = tables.root_scalars[chain.factor as usize];
                    for j in 0..exponent {
                        let mut value = arithmetic.mul_scalar(a[j], r);
                        if j + 1 < exponent {
                            value = arithmetic.add(value, a[j + 1]);
                        }
                        b[j] = value;
                    }
                    constant = arithmetic.add(constant, a[0]);
                } else {
                    // d a_j = top f + rem: rem / f^j + top / f^(j-1).
                    let f = &tables.basis.monic[chain.factor as usize];
                    for j in 0..exponent {
                        let block = &a[j * degree..(j + 1) * degree];
                        let top = block[degree - 1];
                        let out = &mut b[j * degree..(j + 1) * degree];
                        out[0] = arithmetic.neg(arithmetic.mul(top, f[0]));
                        for i in 1..degree {
                            out[i] = arithmetic.sub(block[i - 1], arithmetic.mul(top, f[i]));
                        }
                        if j == 0 {
                            constant = arithmetic.add(constant, top);
                        } else {
                            let previous = &mut b[(j - 1) * degree];
                            *previous = arithmetic.add(*previous, top);
                        }
                    }
                }
                offset += exponent * degree;
            }
            to[0] = constant;
        }
        Ok(result)
    }

    /// `weight / g` for a basis factor `g`.
    fn divide(&mut self, weight: &Fraction, g: u32) -> Result<Fraction, ReductionError> {
        if weight.stride == 0 {
            return Ok(Fraction::default());
        }
        let k = self.basis.degree(g);
        let mut chains = weight.chains.clone();
        match chains.binary_search_by_key(&g, |c| c.factor) {
            Ok(i) => chains[i].exponent += 1,
            Err(i) => chains.insert(
                i,
                Chain {
                    factor: g,
                    exponent: 1,
                },
            ),
        }
        let mut result = self.layout(weight.poly.saturating_sub(k), chains)?;
        // Destination offset of every chain of `weight`, and of g's a_1.
        let mut plan = Vec::with_capacity(weight.chains.len());
        let mut g_offset = 0;
        {
            let mut to = result.poly;
            let mut from = weight.poly;
            let mut source = weight.chains.iter().peekable();
            for chain in &result.chains {
                if chain.factor == g {
                    g_offset = to;
                }
                if let Some(s) = source.peek()
                    && s.factor == chain.factor
                {
                    plan.push((**s, from, to));
                    from += self.chain_len(**s);
                    source.next();
                }
                to += self.chain_len(*chain);
            }
        }
        let g_linear = self.basis.linear_index(g);
        let linear_count = self.basis.linear_count();
        for lane in 0..self.lanes.len() {
            let tables = &self.lanes[lane];
            let arithmetic = &tables.lane;
            let from = &weight.data[lane * weight.stride..(lane + 1) * weight.stride];
            let rs = result.stride;
            // Polynomial part: P = Q g + R.
            if weight.poly > 0 {
                if k == 1 {
                    let s = tables.root_scalars[g as usize];
                    let to = &mut result.data[lane * rs..(lane + 1) * rs];
                    let mut carry = from[weight.poly - 1];
                    for i in (0..weight.poly - 1).rev() {
                        to[i] = carry;
                        carry = arithmetic.add(from[i], arithmetic.mul_scalar(carry, s));
                    }
                    to[g_offset] = arithmetic.add(to[g_offset], carry);
                } else {
                    let (quotient, remainder) = arithmetic
                        .poly_quot_rem(&from[..weight.poly], &tables.basis.monic[g as usize]);
                    let to = &mut result.data[lane * rs..(lane + 1) * rs];
                    to[..quotient.len()].copy_from_slice(&quotient);
                    arithmetic.add_into(&mut to[g_offset..g_offset + remainder.len()], &remainder);
                }
            }
            for &(chain, f, t) in &plan {
                let degree = self.basis.degree(chain.factor);
                let exponent = chain.exponent as usize;
                let a = &from[f..f + exponent * degree];
                if chain.factor == g {
                    // One more power of g.
                    let to = &mut result.data[lane * rs..(lane + 1) * rs];
                    to[t + k..t + k + exponent * k].copy_from_slice(a);
                    continue;
                }
                let h_linear = self.basis.linear_index(chain.factor);
                if let (Some(gl), Some(hl)) = (g_linear, h_linear) {
                    // a_j / ((d - r)^j (d - s)) with t = s - r:
                    // a'_e = -a_e / t, a'_i = (a'_(i+1) - a_i) / t, and the
                    // residue at s is -a'_1.
                    let inverse = tables.basis.inverse_difference[gl * linear_count + hl];
                    let to = &mut result.data[lane * rs..(lane + 1) * rs];
                    let mut next = zero();
                    for i in (0..exponent).rev() {
                        next = arithmetic.mul(arithmetic.sub(next, a[i]), inverse);
                        to[t + i] = next;
                    }
                    to[g_offset] = arithmetic.sub(to[g_offset], next);
                    continue;
                }
                let (chain_part, g_part) =
                    divide_chain(tables, &mut self.power_cache[lane], chain, a, g)?;
                let to = &mut result.data[lane * rs..(lane + 1) * rs];
                to[t..t + chain_part.len()].copy_from_slice(&chain_part);
                arithmetic.add_into(&mut to[g_offset..g_offset + g_part.len()], &g_part);
            }
        }
        Ok(result)
    }

    /// `active * d^power`; `powers[i]` caches `active * d^(i + 1)`.
    fn power<'w>(&'w self, active: &'w Fraction, power: usize) -> &'w Fraction {
        if power == 0 {
            active
        } else {
            &self.powers[power - 1]
        }
    }

    fn polynomial_product(
        &mut self,
        active: &Fraction,
        index: usize,
    ) -> Result<Fraction, ReductionError> {
        let length = self
            .lanes
            .iter()
            .map(|tables| tables.numerators[index].len())
            .max()
            .unwrap_or(0);
        if length == 0 || active.stride == 0 {
            return Ok(Fraction::default());
        }
        let top = length - 1;
        while self.powers.len() < top {
            let next = self.mul_x(self.power(active, self.powers.len()))?;
            self.powers.push(next);
        }
        let mut result = self.scaled(self.power(active, top), Scale::Numerator(index, top));
        let powers = std::mem::take(&mut self.powers);
        for power in (0..top).rev() {
            let source = if power == 0 {
                active
            } else {
                &powers[power - 1]
            };
            self.axpy(&mut result, source, Scale::Numerator(index, power))?;
        }
        self.powers = powers;
        self.divide_by_denominator(result, index)
    }

    fn divide_by_denominator(
        &mut self,
        mut weight: Fraction,
        index: usize,
    ) -> Result<Fraction, ReductionError> {
        let basis = self.basis;
        for &(g, exponent) in &basis.coefficients[index].factors {
            for _ in 0..exponent {
                weight = self.divide(&weight, g)?;
            }
        }
        Ok(weight)
    }
}

/// `sum_j a_j / h^j` (dense `A / h^e` with `A = sum_j a_j h^(e-j)`) divided
/// by a basis factor `g` coprime to `h`: `A / (h^e g) = B / h^e + C / g` with
/// `C = A (h^e)^-1 mod g` and `B = (A - C h^e) / g`. Returns the chain
/// coefficients of `B` (`e deg h` values) and `C` (`deg g` values at most).
fn divide_chain(
    tables: &LaneTables,
    cache: &mut HashMap<(u32, usize), Poly>,
    chain: Chain,
    a: &[Fe],
    g: u32,
) -> Result<(Poly, Poly), ReductionError> {
    let arithmetic = &tables.lane;
    let h = &tables.basis.monic[chain.factor as usize];
    let gm = &tables.basis.monic[g as usize];
    let exponent = chain.exponent as usize;
    let dense = to_dense(arithmetic, a, h, exponent);
    let power = cache
        .entry((chain.factor, exponent))
        .or_insert_with(|| arithmetic.poly_pow(h, exponent));
    let inverse = arithmetic
        .poly_inverse_mod(&arithmetic.poly_rem(power, gm), gm)
        .ok_or_else(|| error("basis factor images are not coprime"))?;
    let c = arithmetic.poly_rem(
        &arithmetic.poly_mul(&arithmetic.poly_rem(&dense, gm), &inverse),
        gm,
    );
    let numerator = arithmetic.poly_sub(&dense, &arithmetic.poly_mul(&c, power));
    let (b, remainder) = arithmetic.poly_quot_rem(&numerator, gm);
    if !remainder.is_empty() {
        return Err(error("partial-fraction division left a remainder"));
    }
    Ok((expand(arithmetic, b, h, exponent), c))
}

/// `A = sum_j a_j h^(e-j)` from the chain `a_1, ..., a_e`.
fn to_dense(arithmetic: &Lane, a: &[Fe], h: &[Fe], exponent: usize) -> Poly {
    let degree = h.len() - 1;
    let mut dense: Poly = vec![];
    for j in 0..exponent {
        let mut block = a[j * degree..(j + 1) * degree].to_vec();
        field::trim(&arithmetic.field, &mut block);
        dense = arithmetic.poly_add(&arithmetic.poly_mul(&dense, h), &block);
    }
    dense
}

/// The chain `a_1, ..., a_e` of `B = sum_j a_j h^(e-j)` (`deg B < e deg h`).
fn expand(arithmetic: &Lane, mut b: Poly, h: &[Fe], exponent: usize) -> Poly {
    let degree = h.len() - 1;
    let mut chain = vec![zero(); exponent * degree];
    for j in (0..exponent).rev() {
        let (quotient, remainder) = arithmetic.poly_quot_rem(&b, h);
        chain[j * degree..j * degree + remainder.len()].copy_from_slice(&remainder);
        b = quotient;
    }
    debug_assert!(b.is_empty());
    chain
}

impl Representation for Factorized<'_> {
    type Weight = Fraction;

    fn coefficient(&mut self, c: CoefRef) -> Result<Fraction, ReductionError> {
        if c.is_constant() {
            let mut weight = self.layout(1, vec![])?;
            for (lane, tables) in self.lanes.iter().enumerate() {
                weight.data[lane] = tables.constants[c.index()];
            }
            return Ok(weight);
        }
        let index = c.index();
        let length = self
            .lanes
            .iter()
            .map(|tables| tables.numerators[index].len())
            .max()
            .unwrap_or(0);
        if length == 0 {
            return Ok(Fraction::default());
        }
        let mut weight = self.layout(length, vec![])?;
        for (lane, tables) in self.lanes.iter().enumerate() {
            let numerator = &tables.numerators[index];
            weight.data[lane * length..lane * length + numerator.len()].copy_from_slice(numerator);
        }
        self.divide_by_denominator(weight, index)
    }

    fn activate(&mut self, active: &mut Fraction) {
        self.trim(active);
        self.powers.clear();
    }

    fn product(&mut self, active: &Fraction, c: CoefRef) -> Result<Fraction, ReductionError> {
        if c.is_constant() {
            Ok(self.scaled(active, Scale::Constant(c.index())))
        } else {
            self.polynomial_product(active, c.index())
        }
    }

    fn add_product(
        &mut self,
        destination: &mut Fraction,
        active: &Fraction,
        c: CoefRef,
    ) -> Result<(), ReductionError> {
        if c.is_constant() {
            return self.axpy(destination, active, Scale::Constant(c.index()));
        }
        let product = self.polynomial_product(active, c.index())?;
        if destination.stride == 0 {
            *destination = product;
            Ok(())
        } else {
            self.axpy(destination, &product, Scale::One)
        }
    }

    fn is_zero(&self, weight: &Fraction) -> bool {
        weight.data.iter().all(Lane::is_zero)
    }

    fn words(&self, weight: &Fraction) -> usize {
        weight.data.len()
    }

    /// Reduced `numerator / monic denominator` of one lane.
    fn image(&mut self, weight: &Fraction, lane: usize) -> Image {
        let tables = &self.lanes[lane];
        let arithmetic = &tables.lane;
        let f = &arithmetic.field;
        if weight.stride == 0 {
            return image_from(f, &[], &[arithmetic.one()]);
        }
        let from = &weight.data[lane * weight.stride..(lane + 1) * weight.stride];
        let mut numerator: Poly = from[..weight.poly].to_vec();
        field::trim(f, &mut numerator);
        let mut denominator: Poly = vec![arithmetic.one()];
        let mut offset = weight.poly;
        for chain in &weight.chains {
            let h = &tables.basis.monic[chain.factor as usize];
            let size = self.chain_len(*chain);
            let dense = to_dense(
                arithmetic,
                &from[offset..offset + size],
                h,
                chain.exponent as usize,
            );
            offset += size;
            if dense.is_empty() {
                continue;
            }
            let power = arithmetic.poly_pow(h, chain.exponent as usize);
            numerator = arithmetic.poly_add(
                &arithmetic.poly_mul(&numerator, &power),
                &arithmetic.poly_mul(&dense, &denominator),
            );
            denominator = arithmetic.poly_mul(&denominator, &power);
        }
        if numerator.is_empty() {
            return image_from(f, &[], &[arithmetic.one()]);
        }
        // Coprime on good, lucky primes; reduce anyway (an unlucky prime
        // shows up in the degrees and is outvoted by the lift).
        let gcd = field::gcd(f, &numerator, &denominator);
        if gcd.len() > 1 {
            numerator = arithmetic.poly_quot_rem(&numerator, &gcd).0;
            denominator = arithmetic.poly_quot_rem(&denominator, &gcd).0;
        }
        let lead = arithmetic.inv(*denominator.last().expect("nonzero denominator"));
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = arithmetic.mul(*c, lead);
        }
        image_from(f, &numerator, &denominator)
    }
}
