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
//! are present and the length of the polynomial part. The lanes of one
//! coefficient position are adjacent in memory, so the layout bookkeeping of
//! every operation (most chains hold one or two coefficients) is paid once
//! for all lanes. A layout may be larger than a value needs (zero
//! coefficients); trailing zeros are trimmed when a weight is popped, and
//! only when they are zero in every lane. Values, zero tests and images of a
//! lane never depend on the other lanes of the job.
use std::collections::HashMap;

use super::super::lift::Image;
use super::super::{CoefRef, error};
use super::Representation;
use super::basis::{Basis, LaneTables};
use super::lane::{Lane, Raw, Scalar};
use crate::reduction::ReductionError;

/// All coefficients `a_{f,1..e}` of one basis factor `f`, starting at
/// coefficient position `offset`.
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
/// `positions` coefficient positions: the polynomial part (`poly` ascending
/// coefficients), then every chain in factor order, each as `a_1, ..., a_e`
/// with `deg f` ascending coefficients per `a_j`. Position `i` of lane `l` is
/// `data[i * lanes + l]` (raw Montgomery residues). The default value is
/// zero.
#[derive(Clone, Debug, Default)]
pub(super) struct Fraction {
    poly: usize,
    chains: Vec<Chain>,
    positions: usize,
    data: Vec<u64>,
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
    /// Scratch: `(source position, destination position, length)`.
    segments: Vec<(usize, usize, usize)>,
    /// `active * d^i` for `i = 1, 2, ...`, until the next pop.
    powers: Vec<Fraction>,
    /// Per lane: `f^e`, and `(h^e)^-1 mod g`, for the nonlinear paths.
    power_cache: Vec<HashMap<(u32, u32), Raw>>,
    inverse_cache: Vec<HashMap<(u32, u32, u32), Raw>>,
    /// rr-alloc: reused buffers of the nonlinear division paths.
    scratch: Scratch,
    /// rr-alloc: the chain block being divided (reused).
    chain_block: Raw,
    /// Scalars of every constant coefficient, lanes adjacent.
    constant_scalars: Vec<Scalar>,
    /// `(start, length)` in `numerator_table` of every polynomial
    /// coefficient's numerator scalars (power-major, lanes adjacent).
    numerator_scalars: Vec<(usize, usize)>,
    numerator_table: Vec<Scalar>,
    zero_scalars: Vec<Scalar>,
}

impl<'a> Factorized<'a> {
    pub fn new(lanes: &'a [LaneTables], basis: &'a Basis, max_degree: usize) -> Self {
        assert!(!lanes.is_empty());
        let width = lanes.len();
        let constants = lanes[0].constant_scalars.len();
        let mut constant_scalars = Vec::with_capacity(constants * width);
        for c in 0..constants {
            constant_scalars.extend(lanes.iter().map(|tables| tables.constant_scalars[c]));
        }
        let mut numerator_scalars = Vec::with_capacity(basis.coefficients.len());
        let mut numerator_table = vec![];
        for c in 0..basis.coefficients.len() {
            let length = lanes
                .iter()
                .map(|tables| tables.numerator_scalars[c].len())
                .max()
                .unwrap_or(0);
            numerator_scalars.push((numerator_table.len(), length));
            for power in 0..length {
                numerator_table.extend(lanes.iter().map(|tables| {
                    tables.numerator_scalars[c]
                        .get(power)
                        .copied()
                        .unwrap_or_default()
                }));
            }
        }
        Self {
            lanes,
            basis,
            max_degree,
            segments: vec![],
            powers: vec![],
            power_cache: lanes.iter().map(|_| HashMap::new()).collect(),
            inverse_cache: lanes.iter().map(|_| HashMap::new()).collect(),
            scratch: Scratch::default(),
            chain_block: vec![],
            constant_scalars,
            numerator_scalars,
            numerator_table,
            zero_scalars: vec![Scalar::default(); width],
        }
    }

    #[inline(always)]
    fn width(&self) -> usize {
        self.lanes.len()
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
            positions: offset,
            data: vec![0; offset * self.width()],
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
        let mut targets = destination.chains.iter();
        'next: for chain in &source.chains {
            for target in targets.by_ref() {
                if target.factor == chain.factor {
                    if target.exponent < chain.exponent {
                        return false;
                    }
                    push(
                        chain.offset as usize,
                        target.offset as usize,
                        self.chain_len(*chain),
                    );
                    continue 'next;
                }
                if target.factor > chain.factor {
                    return false;
                }
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
        let width = self.width();
        for &(f, t, len) in &segments {
            result.data[t * width..(t + len) * width]
                .copy_from_slice(&source.data[f * width..(f + len) * width]);
        }
        self.segments = segments;
        Ok(result)
    }

    /// The per-lane scalars of `scale`, or `None` for one.
    #[inline(always)]
    fn scalars(&self, scale: Scale) -> Option<&[Scalar]> {
        let width = self.width();
        match scale {
            Scale::One => None,
            Scale::Constant(c) => Some(&self.constant_scalars[c * width..(c + 1) * width]),
            Scale::Numerator(c, power) => {
                let (start, length) = self.numerator_scalars[c];
                Some(if power < length {
                    let at = start + power * width;
                    &self.numerator_table[at..at + width]
                } else {
                    &self.zero_scalars
                })
            }
        }
    }

    /// `destination (=|+=) scalars * source` over interleaved positions.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    fn kernel(
        &self,
        destination: &mut [u64],
        source: &[u64],
        scalars: Option<&[Scalar]>,
        assign: bool,
    ) {
        let width = self.width();
        if width == 1 {
            let lane = &self.lanes[0].lane;
            // Indexed loops over equal-length prefixes (what `zip` visits):
            // the hot path of single-lane replays, without an out-of-line
            // `Zip` constructor per call.
            let n = destination.len().min(source.len());
            let (destination, source) = (&mut destination[..n], &source[..n]);
            match (scalars, assign) {
                (None, true) => destination.copy_from_slice(source),
                (None, false) => {
                    for i in 0..n {
                        destination[i] = lane.add(destination[i], source[i]);
                    }
                }
                (Some(s), true) => {
                    let s = s[0];
                    for i in 0..n {
                        destination[i] = lane.mul_scalar(source[i], s);
                    }
                }
                (Some(s), false) => {
                    let s = s[0];
                    for i in 0..n {
                        destination[i] = lane.add(destination[i], lane.mul_scalar(source[i], s));
                    }
                }
            }
            return;
        }
        for (d, x) in destination
            .chunks_exact_mut(width)
            .zip(source.chunks_exact(width))
        {
            for (l, tables) in self.lanes.iter().enumerate() {
                let lane = &tables.lane;
                let value = match scalars {
                    None => x[l],
                    Some(s) => lane.mul_scalar(x[l], s[l]),
                };
                d[l] = if assign { value } else { lane.add(d[l], value) };
            }
        }
    }

    /// `scale * source` with the layout of `source`.
    fn scaled(&self, source: &Fraction, scale: Scale) -> Fraction {
        let mut result = Fraction {
            poly: source.poly,
            chains: source.chains.clone(),
            positions: source.positions,
            data: vec![0; source.data.len()],
        };
        self.kernel(&mut result.data, &source.data, self.scalars(scale), true);
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
        if source.positions == 0 {
            return Ok(());
        }
        if destination.positions == 0 {
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
        let width = self.width();
        let scalars = self.scalars(scale);
        for &(f, t, len) in &segments {
            self.kernel(
                &mut destination.data[t * width..(t + len) * width],
                &source.data[f * width..(f + len) * width],
                scalars,
                false,
            );
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
        if weight.positions == 0 {
            return;
        }
        let width = self.width();
        let zero_from = |position: usize, len: usize| {
            weight.data[position * width..(position + len) * width]
                .iter()
                .all(|&c| c == 0)
        };
        let mut poly = weight.poly;
        while poly > 0 && zero_from(poly - 1, 1) {
            poly -= 1;
        }
        let mut kept = Vec::with_capacity(weight.chains.len());
        let mut changed = poly != weight.poly;
        for chain in &weight.chains {
            let degree = self.basis.degree(chain.factor);
            let mut exponent = chain.exponent as usize;
            while exponent > 0 && zero_from(chain.offset as usize + (exponent - 1) * degree, degree)
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
        trimmed.data[..poly * width].copy_from_slice(&weight.data[..poly * width]);
        for ((old, _), new) in kept.iter().zip(&trimmed.chains) {
            let len = self.chain_len(*new) * width;
            let (f, t) = (old.offset as usize * width, new.offset as usize * width);
            trimmed.data[t..t + len].copy_from_slice(&weight.data[f..f + len]);
        }
        *weight = trimmed;
    }

    /// `d * source`.
    fn mul_x(&self, source: &Fraction) -> Result<Fraction, ReductionError> {
        if source.positions == 0 {
            return Ok(Fraction::default());
        }
        let width = self.width();
        let chains = source
            .chains
            .iter()
            .map(|c| Chain::new(c.factor, c.exponent))
            .collect();
        // The result's layout is the source's shifted by one position.
        let mut result = self.layout(source.poly + 1, chains)?;
        let from = &source.data;
        let to = &mut result.data;
        to[width..(source.poly + 1) * width].copy_from_slice(&from[..source.poly * width]);
        for chain in &source.chains {
            let degree = self.basis.degree(chain.factor);
            let exponent = chain.exponent as usize;
            let offset = chain.offset as usize;
            for (l, tables) in self.lanes.iter().enumerate() {
                let lane = &tables.lane;
                let a = |position: usize| from[(offset + position) * width + l];
                let b = |position: usize| (offset + 1 + position) * width + l;
                if degree == 1 {
                    // a_j d / (d - r)^j = a_j / (d - r)^(j-1) + r a_j / (d - r)^j
                    let r = tables.root_scalars[chain.factor as usize];
                    for j in 0..exponent {
                        let mut value = lane.mul_scalar(a(j), r);
                        if j + 1 < exponent {
                            value = lane.add(value, a(j + 1));
                        }
                        to[b(j)] = value;
                    }
                    to[l] = lane.add(to[l], a(0));
                } else {
                    // d a_j = top f + rem: rem / f^j + top / f^(j-1).
                    let f = &tables.basis.monic[chain.factor as usize];
                    for j in 0..exponent {
                        let base = j * degree;
                        let top = a(base + degree - 1);
                        to[b(base)] = lane.neg(lane.mul(top, f[0]));
                        for i in 1..degree {
                            to[b(base + i)] = lane.sub(a(base + i - 1), lane.mul(top, f[i]));
                        }
                        let target = if j == 0 { l } else { b(base - degree) };
                        to[target] = lane.add(to[target], top);
                    }
                }
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
        if length == 0 || active.positions == 0 {
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
        let width = self.width();
        let g_chain = weight.chains[position];
        let g = g_chain.factor;
        let k = self.basis.degree(g);
        let g_offset = g_chain.offset as usize;
        let g_exponent = extent.exponents[position] as usize;
        debug_assert!(g_exponent < g_chain.exponent as usize);
        let g_linear = self.basis.linear_index(g);
        let linear_count = self.basis.linear_count();
        let poly = extent.poly;
        let w = &mut weight.data;
        // One more power of g: a_j -> a_(j+1), then the new a_1 collects the
        // residues (for every lane at once).
        w.copy_within(
            g_offset * width..(g_offset + g_exponent * k) * width,
            (g_offset + k) * width,
        );
        w[g_offset * width..(g_offset + k) * width].fill(0);
        let lanes = self.lanes;
        for (l, tables) in lanes.iter().enumerate() {
            let lane = &tables.lane;
            let at = |position: usize| position * width + l;
            // Polynomial part: P = Q g + R.
            if poly > 0 {
                if k == 1 {
                    let s = tables.root_scalars[g as usize];
                    let mut carry = w[at(poly - 1)];
                    for i in (0..poly - 1).rev() {
                        let next = lane.add(w[at(i)], lane.mul_scalar(carry, s));
                        w[at(i)] = carry;
                        carry = next;
                    }
                    w[at(poly - 1)] = 0;
                    w[at(g_offset)] = lane.add(w[at(g_offset)], carry);
                } else {
                    let s = &mut self.scratch;
                    s.part.clear();
                    s.part.extend((0..poly).map(|i| w[at(i)]));
                    lane.divrem_monic_into(
                        &s.part,
                        &tables.basis.monic[g as usize],
                        &mut s.q,
                        &mut s.r,
                    );
                    for i in 0..poly {
                        w[at(i)] = s.q.get(i).copied().unwrap_or(0);
                    }
                    for (i, &r) in s.r.iter().enumerate() {
                        w[at(g_offset + i)] = lane.add(w[at(g_offset + i)], r);
                    }
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
                    let mut next = 0;
                    for i in (0..exponent).rev() {
                        next = lane.mul(lane.sub(next, w[at(offset + i)]), inverse);
                        w[at(offset + i)] = next;
                    }
                    w[at(g_offset)] = lane.sub(w[at(g_offset)], next);
                    continue;
                }
                let block = &mut self.chain_block;
                block.clear();
                block.extend((0..exponent * degree).map(|i| w[at(offset + i)]));
                divide_chain(
                    tables,
                    &mut self.power_cache[l],
                    &mut self.inverse_cache[l],
                    chain.factor,
                    exponent,
                    block,
                    g,
                    &mut self.scratch,
                )?;
                for (i, &value) in block.iter().enumerate() {
                    w[at(offset + i)] = value;
                }
                for (i, &r) in self.scratch.c.iter().enumerate() {
                    w[at(g_offset + i)] = lane.add(w[at(g_offset + i)], r);
                }
            }
        }
        extent.poly = poly.saturating_sub(k);
        extent.exponents[position] += 1;
        Ok(())
    }
}

/// rr-alloc: reused buffers of [`divide_chain`] and the nonlinear part
/// division (the replay allocated a fresh vector for every intermediate).
#[derive(Default)]
struct Scratch {
    dense: Raw,
    t0: Raw,
    block: Raw,
    q: Raw,
    r: Raw,
    b: Raw,
    c: Raw,
    numerator: Raw,
    part: Raw,
}

/// Divide the chain `a = (a_1, ..., a_e)` of `h` (`A / h^e` with
/// `A = sum_j a_j h^(e-j)`) by a basis factor `g` coprime to `h`:
/// `A / (h^e g) = B / h^e + C / g` with `C = A (h^e)^-1 mod g` and
/// `B = (A - C h^e) / g`. Overwrites `a` with the chain of `B` and leaves
/// `C` in `s.c`.
#[allow(clippy::too_many_arguments)]
fn divide_chain(
    tables: &LaneTables,
    powers: &mut HashMap<(u32, u32), Raw>,
    inverses: &mut HashMap<(u32, u32, u32), Raw>,
    h: u32,
    exponent: usize,
    a: &mut [u64],
    g: u32,
    s: &mut Scratch,
) -> Result<(), ReductionError> {
    let lane = &tables.lane;
    let hm = &tables.basis.monic[h as usize];
    let gm = &tables.basis.monic[g as usize];
    to_dense_into(lane, a, hm, exponent, &mut s.dense, &mut s.t0, &mut s.block);
    let power = powers
        .entry((h, exponent as u32))
        .or_insert_with(|| lane.poly_pow(hm, exponent));
    let inverse = match inverses.entry((h, g, exponent as u32)) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
            lane.poly_inverse_mod(&lane.divrem_monic(power, gm).1, gm)
                .ok_or_else(|| error("basis factor images are not coprime"))?,
        ),
    };
    // C = ((A mod g) inverse) mod g
    lane.divrem_monic_into(&s.dense, gm, &mut s.q, &mut s.r);
    lane.poly_mul_into(&s.r, inverse, &mut s.t0);
    lane.divrem_monic_into(&s.t0, gm, &mut s.q, &mut s.c);
    // B = (A - C h^e) / g
    lane.poly_mul_into(&s.c, power, &mut s.t0);
    lane.poly_sub_into(&s.dense, &s.t0, &mut s.numerator);
    lane.divrem_monic_into(&s.numerator, gm, &mut s.b, &mut s.r);
    if !s.r.is_empty() {
        return Err(error("partial-fraction division left a remainder"));
    }
    expand_into(lane, &mut s.b, hm, exponent, a, &mut s.q, &mut s.r);
    Ok(())
}

/// `A = sum_j a_j h^(e-j)` from the chain `a_1, ..., a_e`.
fn to_dense(lane: &Lane, a: &[u64], h: &[u64], exponent: usize) -> Raw {
    let (mut dense, mut t0, mut block) = (vec![], vec![], vec![]);
    to_dense_into(lane, a, h, exponent, &mut dense, &mut t0, &mut block);
    dense
}

/// rr-alloc: [`to_dense`] into reused buffers.
fn to_dense_into(
    lane: &Lane,
    a: &[u64],
    h: &[u64],
    exponent: usize,
    dense: &mut Raw,
    t0: &mut Raw,
    block: &mut Raw,
) {
    let degree = h.len() - 1;
    dense.clear();
    for j in 0..exponent {
        block.clear();
        block.extend_from_slice(&a[j * degree..(j + 1) * degree]);
        Lane::trim(block);
        lane.poly_mul_into(dense, h, t0);
        lane.poly_add_into(t0, block, dense);
    }
}

/// Write the chain `a_1, ..., a_e` of `B = sum_j a_j h^(e-j)`
/// (`deg B < e deg h`) into `chain` (`b` is consumed as scratch).
fn expand_into(
    lane: &Lane,
    b: &mut Raw,
    h: &[u64],
    exponent: usize,
    chain: &mut [u64],
    quotient: &mut Raw,
    remainder: &mut Raw,
) {
    let degree = h.len() - 1;
    chain.fill(0);
    for j in (0..exponent).rev() {
        lane.divrem_monic_into(b, h, quotient, remainder);
        chain[j * degree..j * degree + remainder.len()].copy_from_slice(remainder);
        std::mem::swap(b, quotient);
    }
    debug_assert!(b.is_empty());
}

impl Representation for Factorized<'_> {
    type Weight = Fraction;

    fn coefficient(&mut self, c: CoefRef) -> Result<Fraction, ReductionError> {
        if c.is_constant() {
            let mut weight = self.layout(1, vec![])?;
            for (l, tables) in self.lanes.iter().enumerate() {
                weight.data[l] = tables.constants[c.index()];
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
        let width = self.width();
        for (l, tables) in self.lanes.iter().enumerate() {
            for (i, &c) in tables.numerators[index].iter().enumerate() {
                weight.data[i * width + l] = c;
            }
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
        if destination.positions == 0 {
            *destination = product;
            Ok(())
        } else {
            self.axpy(destination, &product, Scale::One)
        }
    }

    fn is_zero(&self, weight: &Fraction) -> bool {
        weight.data.iter().all(|&c| c == 0)
    }

    fn words(&self, weight: &Fraction) -> usize {
        weight.data.len()
    }

    /// Reduced `numerator / monic denominator` of one lane.
    fn image(&mut self, weight: &Fraction, l: usize) -> Image {
        let tables = &self.lanes[l];
        let lane = &tables.lane;
        let width = self.width();
        let value = |position: usize| weight.data[position * width + l];
        let mut numerator: Raw = (0..weight.poly).map(value).collect();
        Lane::trim(&mut numerator);
        let mut denominator: Raw = vec![lane.one()];
        for chain in &weight.chains {
            let h = &tables.basis.monic[chain.factor as usize];
            let offset = chain.offset as usize;
            let block: Raw = (offset..offset + self.chain_len(*chain))
                .map(value)
                .collect();
            let dense = to_dense(lane, &block, h, chain.exponent as usize);
            if dense.is_empty() {
                continue;
            }
            let power = self.power_cache[l]
                .entry((chain.factor, chain.exponent))
                .or_insert_with(|| lane.poly_pow(h, chain.exponent as usize));
            numerator = lane.poly_add(
                &lane.poly_mul(&numerator, power),
                &lane.poly_mul(&dense, &denominator),
            );
            denominator = lane.poly_mul(&denominator, power);
        }
        if numerator.is_empty() {
            return Image {
                numerator: vec![],
                denominator: vec![1],
            };
        }
        // Coprime on good, lucky primes; reduce anyway (an unlucky prime
        // shows up in the degrees and is outvoted by the lift).
        let gcd = lane.gcd(&numerator, &denominator);
        if gcd.len() > 1 {
            numerator = lane.divrem_monic(&numerator, &gcd).0;
            denominator = lane.divrem_monic(&denominator, &gcd).0;
        }
        let lead = lane.make_monic(&mut denominator);
        for c in numerator.iter_mut() {
            *c = lane.mul(*c, lead);
        }
        Image {
            numerator: lane.standard_poly(&numerator),
            denominator: lane.standard_poly(&denominator),
        }
    }
}
