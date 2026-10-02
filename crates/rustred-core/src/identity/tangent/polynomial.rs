//! Native polynomial operations with conservative structural preflight.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use symbolica::{
    atom::{NamespacedSymbol, SymbolBuilder},
    prelude::*,
};

use super::error::{add, limit, mul, reserve};
use super::{TangentSourceError as Error, TangentSourceLimits};
use crate::algebra::is_exact_plain_symbol;
use crate::family::{DenominatorExpansion, IntegralFamily};

pub(super) type RawPolynomial =
    MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

pub(super) struct PolynomialWork<'a> {
    pub family: &'a IntegralFamily,
    pub template: RawPolynomial,
    pub limits: TangentSourceLimits,
    operations: usize,
}

fn native<T>(f: impl FnOnce() -> T) -> Result<T, Error> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| Error::NativePanic)
}
fn degree(p: &RawPolynomial) -> Result<u16, Error> {
    p.exponents_iter().try_fold(0, |maximum, exponents| {
        let sum = exponents.iter().try_fold(0u16, |a, &b| {
            a.checked_add(b).ok_or(Error::ResourceOverflow {
                resource: "polynomial total degree",
            })
        })?;
        Ok(maximum.max(sum))
    })
}

impl<'a> PolynomialWork<'a> {
    pub fn new(family: &'a IntegralFamily, limits: TangentSourceLimits) -> Result<Self, Error> {
        let arity = family.denominator_count();
        limit("denominator variables", arity, limits.max_denominators)?;
        let mut variables = Vec::new();
        reserve(&mut variables, arity, "denominator variables")?;
        for axis in 0..arity {
            let name = format!("rustred_tangent_source_v1::D{axis}");
            let namespaced =
                NamespacedSymbol::try_parse(&name).ok_or(Error::SymbolCollision { axis })?;
            let symbol = SymbolBuilder::new(namespaced)
                .build()
                .map_err(|e| Error::Symbolica {
                    detail: e.to_string(),
                })?;
            let variable = PolyVariable::Symbol(symbol);
            if !is_exact_plain_symbol(symbol, &name)
                || family.coefficient_context().variables().contains(&variable)
            {
                return Err(Error::SymbolCollision { axis });
            }
            variables.push(variable);
        }
        let template = native(|| {
            RawPolynomial::new(&RationalPolynomialField::new(Z), None, Arc::new(variables))
        })?;
        Ok(Self {
            family,
            template,
            limits,
            operations: 0,
        })
    }
    fn admit(&mut self, terms: usize, maximum_degree: u16, work: usize) -> Result<(), Error> {
        limit("polynomial terms", terms, self.limits.max_polynomial_terms)?;
        limit(
            "polynomial total degree",
            usize::from(maximum_degree),
            usize::from(self.limits.max_polynomial_degree),
        )?;
        limit(
            "polynomial exponent entries",
            mul(
                "polynomial exponent entries",
                terms,
                self.family.denominator_count(),
            )?,
            self.limits.max_exponent_entries,
        )?;
        self.operations = add("polynomial work", self.operations, work)?;
        limit(
            "polynomial work",
            self.operations,
            self.limits.max_term_operations,
        )
    }
    fn validate(&self, p: &RawPolynomial) -> Result<(), Error> {
        if p.variables() != self.template.variables() {
            return Err(Error::ScopeMismatch);
        }
        limit(
            "retained polynomial terms",
            p.nterms(),
            self.limits.max_polynomial_terms,
        )?;
        limit(
            "retained polynomial degree",
            usize::from(degree(p)?),
            usize::from(self.limits.max_polynomial_degree),
        )?;
        limit(
            "retained exponent entries",
            mul(
                "retained exponent entries",
                p.nterms(),
                self.family.denominator_count(),
            )?,
            self.limits.max_exponent_entries,
        )?;
        for c in &p.coefficients {
            self.family
                .coefficient_context()
                .validate_with_limits(c, self.limits.exact_algebra)?;
        }
        Ok(())
    }
    pub fn affine(&mut self, expansion: &DenominatorExpansion) -> Result<RawPolynomial, Error> {
        let n = self.family.denominator_count();
        if expansion.denominator_coefficients().len() != n {
            return Err(Error::ScopeMismatch);
        }
        let count = add(
            "affine terms",
            usize::from(!expansion.constant().is_zero()),
            expansion
                .denominator_coefficients()
                .iter()
                .filter(|c| !c.is_zero())
                .count(),
        )?;
        self.admit(count, 1, count)?;
        self.family
            .coefficient_context()
            .validate_with_limits(expansion.constant(), self.limits.exact_algebra)?;
        for c in expansion.denominator_coefficients() {
            self.family
                .coefficient_context()
                .validate_with_limits(c, self.limits.exact_algebra)?;
        }
        let mut exponents = Vec::new();
        reserve(&mut exponents, n, "affine exponent buffer")?;
        exponents.resize(n, 0);
        let p = native(|| {
            let mut p = self.template.constant(expansion.constant().clone());
            for (axis, c) in expansion.denominator_coefficients().iter().enumerate() {
                if c.is_zero() {
                    continue;
                }
                exponents[axis] = 1;
                p.append_monomial(c.clone(), &exponents);
                exponents[axis] = 0;
            }
            p
        })?;
        self.validate(&p)?;
        Ok(p)
    }
    pub fn multiply(
        &mut self,
        a: &RawPolynomial,
        b: &RawPolynomial,
    ) -> Result<RawPolynomial, Error> {
        let terms = mul("polynomial products", a.nterms(), b.nterms())?;
        let d = degree(a)?
            .checked_add(degree(b)?)
            .ok_or(Error::ResourceOverflow {
                resource: "polynomial degree",
            })?;
        self.admit(terms, d, terms)?;
        let p = native(|| a * b)?;
        self.validate(&p)?;
        Ok(p)
    }
    pub fn sum(&mut self, a: &RawPolynomial, b: &RawPolynomial) -> Result<RawPolynomial, Error> {
        let terms = add("polynomial sum terms", a.nterms(), b.nterms())?;
        self.admit(terms, degree(a)?.max(degree(b)?), terms)?;
        let p = native(|| a + b)?;
        self.validate(&p)?;
        Ok(p)
    }
    pub fn minor(
        &mut self,
        entries: [&RawPolynomial; 4],
        negative: bool,
    ) -> Result<RawPolynomial, Error> {
        let terms = add(
            "minor terms",
            mul("minor terms", entries[0].nterms(), entries[3].nterms())?,
            mul("minor terms", entries[1].nterms(), entries[2].nterms())?,
        )?;
        // Inputs are affine; the only supported native matrix is explicitly 2x2.
        self.admit(terms, 2, add("minor work", terms, terms)?)?;
        let mut data = Vec::new();
        reserve(&mut data, 4, "minor matrix entries")?;
        for p in entries {
            self.validate(p)?;
            data.push(p.clone());
        }
        let matrix = Matrix::from_linear(data, 2, 2, PolynomialRing::from_poly(&self.template))
            .map_err(|e| Error::Symbolica {
                detail: e.to_string(),
            })?;
        let p = native(|| matrix.det())?.map_err(|e| Error::Symbolica {
            detail: e.to_string(),
        })?;
        let p = if negative { native(|| -p)? } else { p };
        self.validate(&p)?;
        Ok(p)
    }
}
