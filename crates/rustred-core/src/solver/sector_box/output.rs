//! Materialised reductions: [`BoxSolution`] (finite-field lane),
//! [`ExactSolution`] (exact lane) and their [`DynamicSolution`] form.

use symbolica::domains::rational::{Rational, RationalField};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{IntegerRing, Q, Z};

use crate::algebra::reconstruct::RationalFunction;
use crate::algebra::{Coefficient, CoefficientContext};

use super::super::SolverError;
use super::super::bridge::{
    DynamicPower, DynamicRule, DynamicSolution, DynamicSolveStats, DynamicTerm,
};
use super::ff::FfReport;

/// Provenance of a materialisation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxProvenance {
    /// blake3 of the selected rows' provenance (the modular selection).
    pub selected_hash: String,
    pub master_hash: String,
    /// Kira sector ids of masters whose sector has no seeded box (a
    /// routed representative that was never seeded; such a master is an
    /// unreduced integral, correct but not minimal). Empty when every
    /// master sector was seeded.
    pub unseeded_master_sectors: Vec<u32>,
}

/// Every requested target reduced onto the masters, with coefficients
/// exact in the free parameter at fixed values of the others.
#[derive(Clone, Debug, PartialEq)]
pub struct BoxSolution<const N: usize> {
    /// Family parameters, in coefficient-variable order.
    pub parameters: Vec<String>,
    /// Fixed parameters and their exact values.
    pub fixed: Vec<(String, Rational)>,
    /// The free parameter.
    pub free: String,
    pub targets: Vec<[i16; N]>,
    /// Masters in column order (hardest first).
    pub masters: Vec<[i16; N]>,
    /// `coefficients[t][m]`: coefficient of `masters[m]` in `targets[t]`.
    pub coefficients: Vec<Vec<RationalFunction>>,
    /// Statistics; `report.reconstruction` lists every reconstruction and
    /// verification prime.
    pub report: FfReport,
    pub provenance: BoxProvenance,
    /// The family arity (native slots used in exchange formats).
    pub arity: usize,
}

impl<const N: usize> BoxSolution<N> {
    /// Nonzero `(master, coefficient)` terms of target `t`.
    pub fn terms(&self, target: usize) -> impl Iterator<Item = (&[i16; N], &RationalFunction)> {
        self.masters
            .iter()
            .zip(&self.coefficients[target])
            .filter(|(_, coefficient)| !coefficient.is_zero())
    }

    /// Primes the reconstruction lifted over, in order.
    pub fn reconstruction_primes(&self) -> Vec<u64> {
        self.report
            .reconstruction
            .primes
            .iter()
            .map(|prime| prime.prime)
            .collect()
    }

    /// Fresh primes the reconstruction was verified at.
    pub fn verification_primes(&self) -> Vec<u64> {
        self.report
            .reconstruction
            .verifications
            .iter()
            .map(|verification| verification.prime)
            .collect()
    }
}

/// The exact lane's result: every requested target reduced onto the
/// masters with coefficients exact in the free parameters.
#[derive(Clone, Debug)]
pub struct ExactSolution<const N: usize> {
    pub parameters: Vec<String>,
    pub fixed: Vec<(String, Rational)>,
    pub free: Vec<String>,
    pub targets: Vec<[i16; N]>,
    pub masters: Vec<[i16; N]>,
    /// `coefficients[t][m]`, in the family's coefficient context.
    pub coefficients: Vec<Vec<Coefficient>>,
    pub rows: usize,
    pub seconds: f64,
    pub arity: usize,
}

/// The univariate function in the coefficient variable `variable` of
/// `context`, as an exact coefficient.
pub(crate) fn coefficient_of(
    function: &RationalFunction,
    context: &CoefficientContext,
    variable: usize,
) -> Coefficient {
    let variables = context.variables().clone();
    let polynomial = |values: &[Rational]| {
        let mut out: MultivariatePolynomial<RationalField, u16> =
            MultivariatePolynomial::new(&Q, Some(values.len()), variables.clone());
        for (degree, value) in values.iter().enumerate() {
            if value.is_zero() {
                continue;
            }
            let mut exponents = vec![0u16; variables.len()];
            exponents[variable] = degree as u16;
            out.append_monomial(value.clone(), &exponents);
        }
        out
    };
    <Coefficient as FromNumeratorAndDenominator<RationalField, IntegerRing, u16>>::from_num_den(
        polynomial(&function.numerator),
        polynomial(&function.denominator),
        &Z,
        true,
    )
}

fn parameter_index(context: &CoefficientContext, name: &str) -> Result<usize, SolverError> {
    context
        .parameter_names()
        .iter()
        .position(|candidate| candidate == name)
        .ok_or_else(|| SolverError::InvalidInput(format!("unknown free parameter {name}")))
}

fn powers<const N: usize>(values: &[i16; N], arity: usize) -> Vec<DynamicPower> {
    values[..arity]
        .iter()
        .map(|&value| DynamicPower {
            symbolic: false,
            value,
        })
        .collect()
}

/// One numeric rule per target that is not a master, onto the masters
/// (residuals), with the coefficient denominators as nonzero conditions.
fn dynamic_solution<const N: usize>(
    targets: &[[i16; N]],
    masters: &[[i16; N]],
    coefficients: &[Vec<Coefficient>],
    arity: usize,
    rows: usize,
) -> DynamicSolution {
    let mut rules = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        if masters.contains(target) {
            continue;
        }
        let mut rhs = Vec::new();
        let mut nonzero_conditions = Vec::new();
        for (master, coefficient) in masters.iter().zip(&coefficients[index]) {
            if coefficient.is_zero() {
                continue;
            }
            if !coefficient.denominator.is_constant() {
                nonzero_conditions.push(coefficient.denominator.clone());
            }
            rhs.push(DynamicTerm {
                powers: powers(master, arity),
                coefficient: coefficient.clone(),
            });
        }
        rules.push(DynamicRule {
            sector: target[..arity].iter().map(|&value| value > 0).collect(),
            target: powers(target, arity),
            rhs,
            nonzero_conditions,
            exceptions: Vec::new(),
        });
    }
    DynamicSolution {
        rules,
        residuals: masters
            .iter()
            .map(|master| master[..arity].to_vec())
            .collect(),
        index_variables: Vec::new(),
        stats: DynamicSolveStats {
            sectors: 0,
            seeds: 0,
            rows,
            exact_trace_rows: 0,
        },
        derivation: Vec::new(),
        requested: targets
            .iter()
            .map(|target| target[..arity].to_vec())
            .collect(),
        basis_change: None,
        depth: 0,
        stable_depth: None,
    }
}

/// `rows` must be `targets x masters`, and the arity must fit the storage.
fn check_table<T>(
    rows: &[Vec<T>],
    targets: usize,
    masters: usize,
    arity: usize,
    storage: usize,
) -> Result<(), SolverError> {
    if arity == 0 || arity > storage {
        return Err(SolverError::InvalidInput(format!(
            "solution arity {arity} does not fit the storage arity {storage}"
        )));
    }
    if rows.len() != targets || rows.iter().any(|row| row.len() != masters) {
        return Err(SolverError::InvalidInput(format!(
            "a solution needs {targets} x {masters} coefficients"
        )));
    }
    Ok(())
}

impl<const N: usize> ExactSolution<N> {
    /// The reduction as RustRed rules (see [`BoxSolution::to_dynamic_solution`]).
    pub fn to_dynamic_solution(&self) -> Result<DynamicSolution, SolverError> {
        check_table(
            &self.coefficients,
            self.targets.len(),
            self.masters.len(),
            self.arity,
            N,
        )?;
        Ok(dynamic_solution(
            &self.targets,
            &self.masters,
            &self.coefficients,
            self.arity,
            self.rows,
        ))
    }
}

impl<const N: usize> BoxSolution<N> {
    /// Check the solution's own consistency (its fields are public): the
    /// coefficient table is `targets x masters`, the arity fits `N`, the free
    /// parameter is a parameter, and every other parameter is fixed exactly
    /// once.
    pub fn check_shape(&self) -> Result<(), SolverError> {
        check_table(
            &self.coefficients,
            self.targets.len(),
            self.masters.len(),
            self.arity,
            N,
        )?;
        let mut names: Vec<&str> = self.fixed.iter().map(|(name, _)| name.as_str()).collect();
        names.push(&self.free);
        names.sort_unstable();
        let mut parameters: Vec<&str> = self.parameters.iter().map(String::as_str).collect();
        parameters.sort_unstable();
        if names != parameters {
            return Err(SolverError::InvalidInput(
                "a finite-field solution fixes every parameter except its free one, once".into(),
            ));
        }
        Ok(())
    }

    /// The reduction as RustRed rules: one numeric rule per target that is
    /// not a master, onto the masters (residuals), with the coefficient
    /// denominators as nonzero conditions.
    pub fn to_dynamic_solution(
        &self,
        context: &CoefficientContext,
    ) -> Result<DynamicSolution, SolverError> {
        self.check_shape()?;
        let variable = parameter_index(context, &self.free)?;
        let coefficients: Vec<Vec<Coefficient>> = self
            .coefficients
            .iter()
            .map(|row| {
                row.iter()
                    .map(|f| coefficient_of(f, context, variable))
                    .collect()
            })
            .collect();
        Ok(dynamic_solution(
            &self.targets,
            &self.masters,
            &coefficients,
            self.arity,
            self.report.rows,
        ))
    }

    /// `(target, master)` positions where this solution differs from the
    /// exact lane's (both at the same fixed values).
    pub fn mismatches(
        &self,
        exact: &ExactSolution<N>,
        context: &CoefficientContext,
    ) -> Result<Vec<(usize, usize)>, SolverError> {
        if exact.targets != self.targets
            || exact.masters != self.masters
            || exact.fixed != self.fixed
        {
            return Err(SolverError::InvalidInput(
                "the solutions have different targets, masters or fixed values".into(),
            ));
        }
        self.check_shape()?;
        check_table(
            &exact.coefficients,
            exact.targets.len(),
            exact.masters.len(),
            exact.arity,
            N,
        )?;
        let variable = parameter_index(context, &self.free)?;
        let mut out = Vec::new();
        for (t, row) in self.coefficients.iter().enumerate() {
            for (m, function) in row.iter().enumerate() {
                let difference =
                    &coefficient_of(function, context, variable) - &exact.coefficients[t][m];
                if !difference.is_zero() {
                    out.push((t, m));
                }
            }
        }
        Ok(out)
    }
}
