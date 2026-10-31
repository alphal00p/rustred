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
//!   divided by the denominator's content and leading coefficients) is the
//!   combination `sum_i n_i (w d^i)` (multiplication by `d` shifts each
//!   chain of coefficients by one power; the powers are cached per popped
//!   weight), written once into the product's final layout, followed by one
//!   in-place division per factor `g`: closed recurrences when `g` and the
//!   chain's factor are linear, a small CRT step otherwise.
//!
//! The primes (lanes) of a job share one layout: which factors and powers
//! are present and the length of the polynomial part. A layout may be larger
//! than a value needs (zero coefficients); trailing zeros are trimmed when a
//! weight is popped, and only when they are zero in every lane. Values, zero
//! tests and images of a lane never depend on the other lanes of the job.
use std::collections::HashMap;

use super::super::field::{self, Fe, Poly};
use super::super::lift::{Image, image_from};
use super::super::{CoefRef, error};
use super::Representation;
use super::basis::{Basis, LaneTables};
use super::lane::{Lane, Scalar};
use crate::reduction::ReductionError;

/// All coefficients `a_{f,1..e}` of one basis factor `f`, starting at
/// `offset` within a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chain {
    factor: u32,
    exponent: u32,
    offset: u32,
}

impl Chain {
    fn new(factor: u32, exponent: u32) -> Self {
        Self {
            factor,
            exponent,
            offset: 0,
        }
    }
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

/// Logical extent of a weight being divided in place inside a larger layout.
struct Extent {
    poly: usize,
    /// Current exponent of every chain of the layout.
    exponents: Vec<u32>,
}

pub(super) struct Factorized<'a> {
    lanes: &'a [LaneTables],
    basis: &'a Basis,
    max_degree: usize,
    /// Scratch: `(source offset, destination offset, length)`.
    segments: Vec<(usize, usize, usize)>,
    /// `active * d^i` for `i = 1, 2, ...`, until the next pop.
    powers: Vec<Fraction>,
    /// Per lane: `f^e`, and `(h^e)^-1 mod g`, for the nonlinear paths.
    power_cache: Vec<HashMap<(u32, u32), Poly>>,
    inverse_cache: Vec<HashMap<(u32, u32, u32), Poly>>,
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
            inverse_cache: lanes.iter().map(|_| HashMap::new()).collect(),
        }
    }

    #[inline(always)]
    fn chain_len(&self, chain: Chain) -> usize {
        chain.exponent as usize * self.basis.degree(chain.factor)
    }

    /// A zero weight with the given layout (offsets are assigned here; fails
    /// closed beyond `max_degree`).
    fn layout(&self, poly: usize, mut chains: Vec<Chain>) -> Result<Fraction, ReductionError> {
        let mut offset = poly;
        for chain in &mut chains {
            chain.offset = offset as u32;
            offset += self.chain_len(*chain);
        }
        let denominator = offset - poly;
        if denominator > self.max_degree || poly > self.max_degree + 1 || offset > u32::MAX as usize
        {
            return Err(error(format!(
                "a modular_polynomial weight exceeds max_degree={}",
                self.max_degree
            )));
        }
        Ok(Fraction {
            poly,
            chains,
            stride: offset,
            data: vec![zero(); offset * self.lanes.len()],
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
        let mut rest = &destination.chains[..];
        for chain in &source.chains {
            let Ok(position) = rest.binary_search_by_key(&chain.factor, |c| c.factor) else {
                return false;
            };
            let target = rest[position];
            if target.exponent < chain.exponent {
                return false;
            }
            push(
                chain.offset as usize,
                target.offset as usize,
                self.chain_len(*chain),
            );
            rest = &rest[position + 1..];
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

    /// `destination += scale * source`, widening the destination's layout
    /// when it does not cover the source.
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
                    chains.push(Chain::new(x.factor, x.exponent.max(y.exponent)));
                    i += 1;
                    j += 1;
                }
                (Some(x), Some(y)) if x.factor < y.factor => {
                    chains.push(Chain::new(x.factor, x.exponent));
                    i += 1;
                }
                (Some(x), None) => {
                    chains.push(Chain::new(x.factor, x.exponent));
                    i += 1;
                }
                (_, Some(y)) => {
                    chains.push(Chain::new(y.factor, y.exponent));
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
        let mut kept = Vec::with_capacity(weight.chains.len());
        let mut changed = poly != weight.poly;
        for chain in &weight.chains {
            let degree = self.basis.degree(chain.factor);
            let mut exponent = chain.exponent as usize;
            while exponent > 0
                && !nonzero_in_some_lane(chain.offset as usize + (exponent - 1) * degree, degree)
            {
                exponent -= 1;
            }
            changed |= exponent != chain.exponent as usize;
            if exponent > 0 {
                kept.push((*chain, exponent as u32));
            }
        }
        if !changed {
            return;
        }
        let chains = kept.iter().map(|(c, e)| Chain::new(c.factor, *e)).collect();
        let mut trimmed = self
            .layout(poly, chains)
            .expect("a trimmed layout is within the bounds of the original");
        let new_stride = trimmed.stride;
        for lane in 0..lanes {
            let from = &weight.data[lane * stride..(lane + 1) * stride];
            let to = &mut trimmed.data[lane * new_stride..(lane + 1) * new_stride];
            to[..poly].copy_from_slice(&from[..poly]);
            for ((old, _), new) in kept.iter().zip(&trimmed.chains) {
                let len = self.chain_len(*new);
                let (f, t) = (old.offset as usize, new.offset as usize);
                to[t..t + len].copy_from_slice(&from[f..f + len]);
            }
        }
        *weight = trimmed;
    }

    /// `d * source`.
    fn mul_x(&self, source: &Fraction) -> Result<Fraction, ReductionError> {
        if source.stride == 0 {
            return Ok(Fraction::default());
        }
        let chains = source
            .chains
            .iter()
            .map(|c| Chain::new(c.factor, c.exponent))
            .collect();
        let mut result = self.layout(source.poly + 1, chains)?;
        for (lane, tables) in self.lanes.iter().enumerate() {
            let arithmetic = &tables.lane;
            let from = &source.data[lane * source.stride..(lane + 1) * source.stride];
            let to = &mut result.data[lane * result.stride..(lane + 1) * result.stride];
            to[1..=source.poly].copy_from_slice(&from[..source.poly]);
            let mut constant = zero();
            for chain in &source.chains {
                let degree = self.basis.degree(chain.factor);
                let exponent = chain.exponent as usize;
                let offset = chain.offset as usize;
                let a = &from[offset..offset + exponent * degree];
                // The result's layout is the source's shifted by one.
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
            }
            to[0] = constant;
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

    /// The layout of `weight / prod g^e` over the denominator factors of
    /// polynomial coefficient `index`, keeping `weight`'s polynomial part.
    fn quotient_layout(
        &self,
        poly: usize,
        chains: &[Chain],
        index: usize,
    ) -> Result<Fraction, ReductionError> {
        let factors = &self.basis.coefficients[index].factors;
        let mut merged = Vec::with_capacity(chains.len() + factors.len());
        let (mut i, mut j) = (0, 0);
        while i < chains.len() || j < factors.len() {
            match (chains.get(i), factors.get(j)) {
                (Some(c), Some(&(g, e))) if c.factor == g => {
                    merged.push(Chain::new(g, c.exponent + e));
                    i += 1;
                    j += 1;
                }
                (Some(c), Some(&(g, _))) if c.factor < g => {
                    merged.push(Chain::new(c.factor, c.exponent));
                    i += 1;
                }
                (Some(c), None) => {
                    merged.push(Chain::new(c.factor, c.exponent));
                    i += 1;
                }
                (_, Some(&(g, e))) => {
                    merged.push(Chain::new(g, e));
                    j += 1;
                }
                (None, None) => unreachable!(),
            }
        }
        self.layout(poly, merged)
    }

    /// `active * N / prod g^e` for polynomial coefficient `index`.
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
        // The final layout covers every power; divisions then run in place.
        let mut result = self.quotient_layout(active.poly + top, &active.chains, index)?;
        let powers = std::mem::take(&mut self.powers);
        let mut status = Ok(());
        for power in 0..=top {
            let source = if power == 0 {
                active
            } else {
                &powers[power - 1]
            };
            status = self.axpy(&mut result, source, Scale::Numerator(index, power));
            if status.is_err() {
                break;
            }
        }
        self.powers = powers;
        status?;
        let mut extent = Extent {
            poly: active.poly + top,
            exponents: result
                .chains
                .iter()
                .map(|chain| {
                    active
                        .chains
                        .binary_search_by_key(&chain.factor, |c| c.factor)
                        .map_or(0, |i| active.chains[i].exponent)
                })
                .collect(),
        };
        self.divide_by_denominator(&mut result, &mut extent, index)?;
        Ok(result)
    }

    /// Divide `weight` (logically `extent`) by the denominator factors of
    /// polynomial coefficient `index`, in place; the layout must hold the
    /// final exponents.
    fn divide_by_denominator(
        &mut self,
        weight: &mut Fraction,
        extent: &mut Extent,
        index: usize,
    ) -> Result<(), ReductionError> {
        let basis = self.basis;
        for &(g, exponent) in &basis.coefficients[index].factors {
            let position = weight
                .chains
                .binary_search_by_key(&g, |c| c.factor)
                .expect("the quotient layout holds every denominator factor");
            for _ in 0..exponent {
                self.divide_in_place(weight, extent, position)?;
            }
        }
        Ok(())
    }

    /// Divide `weight` (logically `extent`) by the factor of its chain
    /// `position`, whose block has room for one more power.
    fn divide_in_place(
        &mut self,
        weight: &mut Fraction,
        extent: &mut Extent,
        position: usize,
    ) -> Result<(), ReductionError> {
        let g_chain = weight.chains[position];
        let g = g_chain.factor;
        let k = self.basis.degree(g);
        let g_offset = g_chain.offset as usize;
        let g_exponent = extent.exponents[position] as usize;
        debug_assert!(g_exponent < g_chain.exponent as usize);
        let g_linear = self.basis.linear_index(g);
        let linear_count = self.basis.linear_count();
        let stride = weight.stride;
        let poly = extent.poly;
        let lanes = self.lanes;
        for (lane, tables) in lanes.iter().enumerate() {
            let arithmetic = &tables.lane;
            let w = &mut weight.data[lane * stride..(lane + 1) * stride];
            // One more power of g: a_j -> a_(j+1), then the new a_1 collects
            // the residues.
            w.copy_within(g_offset..g_offset + g_exponent * k, g_offset + k);
            w[g_offset..g_offset + k].fill(zero());
            // Polynomial part: P = Q g + R.
            if poly > 0 {
                if k == 1 {
                    let s = tables.root_scalars[g as usize];
                    let mut carry = w[poly - 1];
                    for i in (0..poly - 1).rev() {
                        let next = arithmetic.add(w[i], arithmetic.mul_scalar(carry, s));
                        w[i] = carry;
                        carry = next;
                    }
                    w[poly - 1] = zero();
                    w[g_offset] = arithmetic.add(w[g_offset], carry);
                } else {
                    let (quotient, remainder) =
                        arithmetic.divrem_monic(&w[..poly], &tables.basis.monic[g as usize]);
                    w[..poly].fill(zero());
                    w[..quotient.len()].copy_from_slice(&quotient);
                    arithmetic.add_into(&mut w[g_offset..g_offset + remainder.len()], &remainder);
                }
            }
            for (index, chain) in weight.chains.iter().enumerate() {
                let exponent = extent.exponents[index] as usize;
                if index == position || exponent == 0 {
                    continue;
                }
                let degree = self.basis.degree(chain.factor);
                let offset = chain.offset as usize;
                if let (Some(gl), Some(hl)) = (g_linear, self.basis.linear_index(chain.factor)) {
                    // a_j / ((d - r)^j (d - s)) with t = s - r:
                    // a'_e = -a_e / t, a'_i = (a'_(i+1) - a_i) / t, and the
                    // residue at s is -a'_1.
                    let inverse = tables.basis.inverse_difference[gl * linear_count + hl];
                    let mut next = zero();
                    for i in (0..exponent).rev() {
                        next = arithmetic.mul(arithmetic.sub(next, w[offset + i]), inverse);
                        w[offset + i] = next;
                    }
                    w[g_offset] = arithmetic.sub(w[g_offset], next);
                    continue;
                }
                let residue = divide_chain(
                    tables,
                    &mut self.power_cache[lane],
                    &mut self.inverse_cache[lane],
                    chain.factor,
                    exponent,
                    &mut w[offset..offset + exponent * degree],
                    g,
                )?;
                arithmetic.add_into(&mut w[g_offset..g_offset + residue.len()], &residue);
            }
        }
        extent.poly = poly.saturating_sub(k);
        extent.exponents[position] += 1;
        Ok(())
    }
}

/// Divide the chain `a = (a_1, ..., a_e)` of `h` (`A / h^e` with
/// `A = sum_j a_j h^(e-j)`) by a basis factor `g` coprime to `h`:
/// `A / (h^e g) = B / h^e + C / g` with `C = A (h^e)^-1 mod g` and
/// `B = (A - C h^e) / g`. Overwrites `a` with the chain of `B` and returns `C`.
fn divide_chain(
    tables: &LaneTables,
    powers: &mut HashMap<(u32, u32), Poly>,
    inverses: &mut HashMap<(u32, u32, u32), Poly>,
    h: u32,
    exponent: usize,
    a: &mut [Fe],
    g: u32,
) -> Result<Poly, ReductionError> {
    let arithmetic = &tables.lane;
    let hm = &tables.basis.monic[h as usize];
    let gm = &tables.basis.monic[g as usize];
    let dense = to_dense(arithmetic, a, hm, exponent);
    let power = powers
        .entry((h, exponent as u32))
        .or_insert_with(|| arithmetic.poly_pow(hm, exponent));
    let inverse = match inverses.entry((h, g, exponent as u32)) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
            arithmetic
                .poly_inverse_mod(&arithmetic.divrem_monic(power, gm).1, gm)
                .ok_or_else(|| error("basis factor images are not coprime"))?,
        ),
    };
    let c = arithmetic
        .divrem_monic(
            &arithmetic.poly_mul(&arithmetic.divrem_monic(&dense, gm).1, inverse),
            gm,
        )
        .1;
    let numerator = arithmetic.poly_sub(&dense, &arithmetic.poly_mul(&c, power));
    let (b, remainder) = arithmetic.divrem_monic(&numerator, gm);
    if !remainder.is_empty() {
        return Err(error("partial-fraction division left a remainder"));
    }
    expand(arithmetic, b, hm, exponent, a);
    Ok(c)
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

/// Write the chain `a_1, ..., a_e` of `B = sum_j a_j h^(e-j)`
/// (`deg B < e deg h`) into `chain`.
fn expand(arithmetic: &Lane, mut b: Poly, h: &[Fe], exponent: usize, chain: &mut [Fe]) {
    let degree = h.len() - 1;
    chain.fill(zero());
    for j in (0..exponent).rev() {
        let (quotient, remainder) = arithmetic.divrem_monic(&b, h);
        chain[j * degree..j * degree + remainder.len()].copy_from_slice(&remainder);
        b = quotient;
    }
    debug_assert!(b.is_empty());
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
        let mut weight = self.quotient_layout(length, &[], index)?;
        for (lane, tables) in self.lanes.iter().enumerate() {
            let numerator = &tables.numerators[index];
            let start = lane * weight.stride;
            weight.data[start..start + numerator.len()].copy_from_slice(numerator);
        }
        let mut extent = Extent {
            poly: length,
            exponents: vec![0; weight.chains.len()],
        };
        self.divide_by_denominator(&mut weight, &mut extent, index)?;
        Ok(weight)
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
        for chain in &weight.chains {
            let h = &tables.basis.monic[chain.factor as usize];
            let offset = chain.offset as usize;
            let dense = to_dense(
                arithmetic,
                &from[offset..offset + self.chain_len(*chain)],
                h,
                chain.exponent as usize,
            );
            if dense.is_empty() {
                continue;
            }
            let power = self.power_cache[lane]
                .entry((chain.factor, chain.exponent))
                .or_insert_with(|| arithmetic.poly_pow(h, chain.exponent as usize));
            numerator = arithmetic.poly_add(
                &arithmetic.poly_mul(&numerator, power),
                &arithmetic.poly_mul(&dense, &denominator),
            );
            denominator = arithmetic.poly_mul(&denominator, power);
        }
        if numerator.is_empty() {
            return image_from(f, &[], &[arithmetic.one()]);
        }
        // Coprime on good, lucky primes; reduce anyway (an unlucky prime
        // shows up in the degrees and is outvoted by the lift).
        let gcd = field::gcd(f, &numerator, &denominator);
        if gcd.len() > 1 {
            numerator = arithmetic.divrem_monic(&numerator, &gcd).0;
            denominator = arithmetic.divrem_monic(&denominator, &gcd).0;
        }
        let lead = arithmetic.inv(*denominator.last().expect("nonzero denominator"));
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = arithmetic.mul(*c, lead);
        }
        image_from(f, &numerator, &denominator)
    }
}
