//! Modular critical-point ideals of one sector polynomial and their quotient
//! dimensions.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, Zp};
use symbolica::poly::groebner::GroebnerBasis;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::poly::{GrevLexOrder, PolyVariable};
use symbolica::prelude::{Ring, RingOps};

use super::error::MasterCountError;
use super::staircase::{Staircase, standard_monomials};

/// Largest quotient dimension admitted. Quotient dimensions are master
/// counts, so this also bounds every value entering the Euler sum.
pub(super) const MAX_QUOTIENT_DIMENSION: usize = 1 << 20;

pub(super) type ModularElement = FiniteFieldElement<u32>;

/// `F4` reads the fast 32-bit path only for exactly [`Zp`]; grevlex keeps the
/// bases small.
type ModularPolynomial = MultivariatePolynomial<Zp, u16, GrevLexOrder>;

/// `G_T` over `Zp[x_T, t]`, with the Rabinowitsch variable `t` last.
///
/// Invariant: every exponent vector has `active + 1` entries and a zero
/// `t` exponent.
pub(super) struct SectorPolynomial<'a> {
    field: &'a Zp,
    variables: Arc<Vec<PolyVariable>>,
    terms: Vec<(ModularElement, Vec<u16>)>,
}

impl<'a> SectorPolynomial<'a> {
    /// Restrict a specialization of `G` over all Feynman parameters to the
    /// face `x_i = 0` for every `i` outside `active`.
    pub(super) fn restrict(
        field: &'a Zp,
        g: &[(ModularElement, Box<[u16]>)],
        active: &[usize],
        feynman_variables: &[PolyVariable],
        saturation: &PolyVariable,
    ) -> Self {
        let mut variables = Vec::with_capacity(active.len() + 1);
        variables.extend(
            active
                .iter()
                .map(|&parameter| feynman_variables[parameter].clone()),
        );
        variables.push(saturation.clone());
        let terms = g
            .iter()
            .filter(|(_, exponents)| {
                exponents
                    .iter()
                    .enumerate()
                    .all(|(parameter, &exponent)| exponent == 0 || active.contains(&parameter))
            })
            .map(|(coefficient, exponents)| {
                let mut restricted = Vec::with_capacity(active.len() + 1);
                restricted.extend(active.iter().map(|&parameter| exponents[parameter]));
                restricted.push(0);
                (*coefficient, restricted)
            })
            .collect();
        Self {
            field,
            variables: Arc::new(variables),
            terms,
        }
    }

    fn active(&self) -> usize {
        self.variables.len() - 1
    }

    /// `dim Zp[x_T, t] / <dG/dx_i, 1 - t G>`: the critical points of `G_T` on
    /// the affine space with `G_T != 0`, counted with multiplicity.
    pub(super) fn morse(&self) -> Result<Staircase, MasterCountError> {
        let mut ideal = Vec::with_capacity(self.variables.len());
        for variable in 0..self.active() {
            let mut derivative = self.zero();
            for (coefficient, exponents) in &self.terms {
                let power = exponents[variable];
                if power == 0 {
                    continue;
                }
                let mut lowered = exponents.clone();
                lowered[variable] -= 1;
                derivative.append_monomial(
                    self.field
                        .mul(coefficient, &self.field.to_element(u32::from(power))),
                    &lowered,
                );
            }
            ideal.push(derivative);
        }
        ideal.push(self.saturation());
        self.quotient(ideal)
    }

    /// `dim Zp[x_T, t] / <x_i dG/dx_i + u_i G, 1 - t G>`: the critical points
    /// of `log G + sum_i u_i log x_i` with `G_T != 0`. On such points
    /// `x_i = 0` would force `u_i G = 0`, so they lie on the torus without an
    /// explicit `prod x_i` saturation.
    ///
    /// Invariant: `exponents` holds one value per active parameter.
    pub(super) fn euler(
        &self,
        exponents: &[ModularElement],
    ) -> Result<Staircase, MasterCountError> {
        debug_assert_eq!(exponents.len(), self.active());
        let mut ideal = Vec::with_capacity(self.variables.len());
        for (variable, exponent) in exponents.iter().enumerate() {
            let mut equation = self.zero();
            for (coefficient, monomial) in &self.terms {
                let weight = self.field.add(
                    &self.field.to_element(u32::from(monomial[variable])),
                    exponent,
                );
                equation.append_monomial(self.field.mul(coefficient, &weight), monomial);
            }
            ideal.push(equation);
        }
        ideal.push(self.saturation());
        self.quotient(ideal)
    }

    fn zero(&self) -> ModularPolynomial {
        ModularPolynomial::new(self.field, Some(self.terms.len()), self.variables.clone())
    }

    /// `1 - t G_T`.
    fn saturation(&self) -> ModularPolynomial {
        let mut saturation = self.zero();
        saturation.append_monomial(self.field.one(), &vec![0; self.variables.len()]);
        let t = self.active();
        for (coefficient, exponents) in &self.terms {
            let mut shifted = exponents.clone();
            shifted[t] = 1;
            saturation.append_monomial(self.field.neg(coefficient), &shifted);
        }
        saturation
    }

    fn quotient(&self, ideal: Vec<ModularPolynomial>) -> Result<Staircase, MasterCountError> {
        let ideal: Vec<_> = ideal
            .into_iter()
            .filter(|polynomial| !polynomial.is_zero())
            .collect();
        let basis =
            catch_unwind(AssertUnwindSafe(|| GroebnerBasis::new(&ideal, false))).map_err(|_| {
                MasterCountError::SymbolicaPanic {
                    stage: "modular Groebner basis",
                }
            })?;
        let leading: Vec<Vec<u16>> = basis
            .system
            .iter()
            .filter(|polynomial| !polynomial.is_zero())
            .map(|polynomial| polynomial.max_exp().to_vec())
            .collect();
        standard_monomials(&leading, self.variables.len(), MAX_QUOTIENT_DIMENSION).map_err(|_| {
            MasterCountError::QuotientTooLarge {
                limit: MAX_QUOTIENT_DIMENSION,
            }
        })
    }
}
