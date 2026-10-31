//! The denominator factor basis of one request and its images modulo a prime.
//!
//! Every non-constant step coefficient `N / D` of the recorded trace is
//! stored as `N / (content * prod f_i^e_i)` over one request-wide basis of
//! primitive integer polynomials `f_i`, irreducible over Q with a positive
//! leading coefficient (Symbolica's factorization, checked by multiplying it
//! back). Every frontier and terminal weight is a sum of products of step
//! coefficients, so its denominator divides a product of basis powers.
//!
//! Modulo a prime `p` the basis maps to monic factors. The prime is rejected
//! (unusable for every terminal) when the map does not keep the basis a set
//! of pairwise coprime factors of unchanged degree, or a coefficient
//! denominator vanishes: a leading coefficient or a content divisible by `p`,
//! two linear factors with the same root, a nonlinear factor vanishing at a
//! linear root, or two nonlinear factors with a common factor. On the
//! remaining primes the partial-fraction representation over the basis is
//! unique, so weights are exact elements of `Z_p(d)` and zero tests are exact.
use std::collections::HashMap;

use symbolica::domains::finite_field::Zp64;
use symbolica::domains::{Ring, RingOps};
use symbolica::poly::factor::Factorize;
use symbolica::prelude::Integer;

use super::super::field::{self, Fe, Poly, batch_invert, dense};
use super::super::{Trace, error};
use super::lane::{Lane, Scalar};
use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::reduction::ReductionError;

/// Marks a nonlinear factor in [`Basis::linear_index`].
const NONLINEAR: u32 = u32::MAX;

/// A polynomial step coefficient over the basis.
#[derive(Clone, Debug)]
pub(super) struct FactoredCoefficient {
    /// Dense ascending numerator.
    pub numerator: Vec<Integer>,
    /// `denominator = content * prod basis[factor]^exponent`.
    pub content: Integer,
    /// `(factor, exponent)`, sorted by factor.
    pub factors: Vec<(u32, u32)>,
}

/// Prime-independent factorization of the polynomial coefficients of one
/// trace (indexed like `Trace::polynomials`).
#[derive(Debug, Default)]
pub(in super::super) struct Basis {
    /// Dense ascending factors.
    factors: Vec<Vec<Integer>>,
    degree: Vec<u32>,
    /// Index among the linear factors, or `NONLINEAR`.
    linear_index: Vec<u32>,
    linear: usize,
    pub(super) coefficients: Vec<FactoredCoefficient>,
}

impl Basis {
    pub fn factor_count(&self) -> usize {
        self.factors.len()
    }

    pub fn linear_count(&self) -> usize {
        self.linear
    }

    #[inline(always)]
    pub fn degree(&self, factor: u32) -> usize {
        self.degree[factor as usize] as usize
    }

    /// Index among the linear factors.
    #[inline(always)]
    pub fn linear_index(&self, factor: u32) -> Option<usize> {
        let index = self.linear_index[factor as usize];
        (index != NONLINEAR).then_some(index as usize)
    }
}

/// Builds the [`Basis`], factoring every distinct denominator once.
#[derive(Default)]
pub(in super::super) struct BasisBuilder {
    basis: Basis,
    /// Interned factors by id, and their ids.
    polynomials: Vec<CoefficientPolynomial>,
    index: HashMap<CoefficientPolynomial, u32>,
    factored: HashMap<CoefficientPolynomial, (Integer, Vec<(u32, u32)>)>,
}

impl BasisBuilder {
    /// Factor the next polynomial coefficient of the trace.
    pub fn push(&mut self, coefficient: &Coefficient) -> Result<(), ReductionError> {
        let denominator = &coefficient.denominator;
        let (content, factors) = match self.factored.get(denominator) {
            Some(factored) => factored.clone(),
            None => {
                let factored = self.factor(denominator)?;
                self.factored.insert(denominator.clone(), factored.clone());
                factored
            }
        };
        self.basis.coefficients.push(FactoredCoefficient {
            numerator: dense(&coefficient.numerator),
            content,
            factors,
        });
        Ok(())
    }

    pub fn finish(self) -> Basis {
        self.basis
    }

    fn factor(
        &mut self,
        denominator: &CoefficientPolynomial,
    ) -> Result<(Integer, Vec<(u32, u32)>), ReductionError> {
        if denominator.is_zero() {
            return Err(error("a step coefficient has a zero denominator"));
        }
        let mut content = Integer::one();
        let mut exponents: Vec<(u32, u32)> = vec![];
        if !denominator.is_constant() {
            let factors =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| denominator.factor()))
                    .map_err(|_| error("factorization of a step coefficient denominator failed"))?;
            for (factor, power) in factors {
                let power = u32::try_from(power)
                    .map_err(|_| error("a denominator factor power exceeds u32"))?;
                if factor.is_constant() {
                    content *= &factor.get_constant().pow(u64::from(power));
                    continue;
                }
                let factor_content = factor.content();
                let mut primitive = factor.div_coeff(&factor_content);
                let mut scale = factor_content;
                if primitive.lcoeff().is_negative() {
                    primitive = -primitive;
                    scale = -scale;
                }
                content *= &scale.pow(u64::from(power));
                let id = self.intern(primitive)?;
                match exponents.iter_mut().find(|(seen, _)| *seen == id) {
                    Some((_, exponent)) => *exponent += power,
                    None => exponents.push((id, power)),
                }
            }
            exponents.sort_unstable();
        } else {
            content = denominator.get_constant();
        }
        // Never trust a factorization that does not multiply back.
        let mut product = denominator.constant(content.clone());
        for &(id, power) in &exponents {
            product = &product * &self.polynomials[id as usize].pow(power as usize);
        }
        if product != *denominator {
            return Err(error(
                "denominator factorization does not reproduce the denominator",
            ));
        }
        Ok((content, exponents))
    }

    fn intern(&mut self, factor: CoefficientPolynomial) -> Result<u32, ReductionError> {
        if let Some(&id) = self.index.get(&factor) {
            return Ok(id);
        }
        let id = u32::try_from(self.basis.factors.len())
            .ok()
            .filter(|id| *id < NONLINEAR)
            .ok_or_else(|| error("too many denominator factors"))?;
        // The context has one variable (checked before recording).
        let coefficients = dense(&factor);
        let degree = coefficients.len() - 1;
        let linear = if degree == 1 {
            self.basis.linear += 1;
            (self.basis.linear - 1) as u32
        } else {
            NONLINEAR
        };
        self.basis.degree.push(
            u32::try_from(degree).map_err(|_| error("a denominator factor degree exceeds u32"))?,
        );
        self.basis.linear_index.push(linear);
        self.basis.factors.push(coefficients);
        self.polynomials.push(factor.clone());
        self.index.insert(factor, id);
        Ok(id)
    }
}

/// The basis modulo one prime.
pub(super) struct LaneBasis {
    /// Monic image of every factor (ascending, last coefficient one).
    pub monic: Vec<Poly>,
    /// Root of every linear factor (zero for the others).
    pub root: Vec<Fe>,
    /// `(root_a - root_b)^-1` at `a * L + b` for distinct linear factors `a`,
    /// `b` (by linear index; `L` linear factors).
    pub inverse_difference: Vec<Fe>,
    /// Leading coefficient of every factor.
    lead: Vec<Fe>,
}

impl LaneBasis {
    /// `None` when the prime is bad for the basis (see the module docs).
    fn new(lane: &Lane, basis: &Basis) -> Option<Self> {
        let f = &lane.field;
        let mut monic = Vec::with_capacity(basis.factors.len());
        let mut lead = Vec::with_capacity(basis.factors.len());
        let mut root = Vec::with_capacity(basis.factors.len());
        let mut roots = Vec::with_capacity(basis.linear);
        for coefficients in &basis.factors {
            let mut image: Poly = coefficients.iter().map(|c| field::reduce(f, c)).collect();
            let leading = *image.last().expect("nonconstant factor");
            if f.is_zero(&leading) {
                return None;
            }
            field::make_monic(f, &mut image);
            if image.len() == 2 {
                let r = f.neg(&image[0]);
                roots.push(r);
                root.push(r);
            } else {
                root.push(f.zero());
            }
            lead.push(leading);
            monic.push(image);
        }
        // Distinct linear roots.
        let mut sorted: Vec<u64> = roots.iter().map(|r| *r.inner()).collect();
        sorted.sort_unstable();
        if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        // Nonlinear factors: no linear root, pairwise coprime.
        let nonlinear: Vec<usize> = (0..monic.len()).filter(|&i| monic[i].len() > 2).collect();
        for (position, &i) in nonlinear.iter().enumerate() {
            if roots
                .iter()
                .any(|r| f.is_zero(&field::horner(f, &monic[i], *r)))
            {
                return None;
            }
            for &j in &nonlinear[position + 1..] {
                if field::gcd(f, &monic[i], &monic[j]).len() > 1 {
                    return None;
                }
            }
        }
        let count = roots.len();
        let mut inverse_difference = vec![f.zero(); count * count];
        for (a, ra) in roots.iter().enumerate() {
            for (b, rb) in roots.iter().enumerate() {
                if a != b {
                    inverse_difference[a * count + b] = f.sub(ra, rb);
                }
            }
        }
        // Zero entries are the diagonal only (roots are distinct).
        batch_invert(f, &mut inverse_difference, |_| {});
        Some(Self {
            monic,
            root,
            inverse_difference,
            lead,
        })
    }
}

/// Everything one replay needs modulo one prime.
pub(super) struct LaneTables {
    pub lane: Lane,
    pub basis: LaneBasis,
    /// Shoup form of every linear factor's root (zero for the others).
    pub root_scalars: Vec<Scalar>,
    /// Constant coefficients.
    pub constants: Vec<Fe>,
    pub constant_scalars: Vec<Scalar>,
    /// Numerators of the polynomial coefficients divided by
    /// `content * prod lead(f)^e`, so that the denominator is the product of
    /// the monic factor images.
    pub numerators: Vec<Poly>,
    pub numerator_scalars: Vec<Vec<Scalar>>,
}

impl LaneTables {
    /// `None` when the prime is bad for this request (see the module docs).
    pub fn new(prime: u64, trace: &Trace, basis: &Basis) -> Option<Self> {
        let lane = Lane::new(prime);
        let f: &Zp64 = &lane.field;
        let lane_basis = LaneBasis::new(&lane, basis)?;
        // Constants n/d: one batched inversion of every denominator.
        let mut denominators: Vec<Fe> = trace
            .constants
            .iter()
            .map(|(_, d)| field::reduce(f, d))
            .collect();
        let mut bad = false;
        batch_invert(f, &mut denominators, |_| bad = true);
        if bad {
            return None;
        }
        let constants: Vec<Fe> = trace
            .constants
            .iter()
            .zip(&denominators)
            .map(|((n, _), inverse)| f.mul(&field::reduce(f, n), inverse))
            .collect();
        let constant_scalars = constants.iter().map(|c| lane.scalar(*c)).collect();
        // Polynomial coefficients: scale by (content * prod lead^e)^-1.
        let mut scales: Vec<Fe> = basis
            .coefficients
            .iter()
            .map(|coefficient| {
                let mut scale = field::reduce(f, &coefficient.content);
                for &(factor, exponent) in &coefficient.factors {
                    let lead = lane_basis.lead[factor as usize];
                    for _ in 0..exponent {
                        scale = f.mul(&scale, &lead);
                    }
                }
                scale
            })
            .collect();
        batch_invert(f, &mut scales, |_| bad = true);
        if bad {
            return None;
        }
        let numerators: Vec<Poly> = basis
            .coefficients
            .iter()
            .zip(&scales)
            .map(|(coefficient, scale)| {
                let mut numerator: Poly = coefficient
                    .numerator
                    .iter()
                    .map(|c| f.mul(&field::reduce(f, c), scale))
                    .collect();
                field::trim(f, &mut numerator);
                numerator
            })
            .collect();
        let numerator_scalars = numerators
            .iter()
            .map(|numerator| numerator.iter().map(|c| lane.scalar(*c)).collect())
            .collect();
        let root_scalars = lane_basis.root.iter().map(|r| lane.scalar(*r)).collect();
        Some(Self {
            root_scalars,
            lane,
            basis: lane_basis,
            constants,
            constant_scalars,
            numerators,
            numerator_scalars,
        })
    }
}
