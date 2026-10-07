use std::sync::Arc;

use crate::algebra::Coefficient;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::PolyVariable;

use crate::algebra::CoefficientPolynomial;
use crate::family::IntegralFamily;
use crate::identity::{ParametricIbpGenerator, ParametricRelation, RowId};

use super::{Integral, PolynomialRow, Power, SolverError, Term};

/// Immutable polynomial source templates shared by sector solvers.
///
/// The index-to-variable map is resolved once. Native Symbolica polynomials
/// share their variable map; individual terms carry neither family seals nor
/// a second coefficient wrapper.
#[derive(Debug)]
pub struct SourceSystem<const N: usize> {
    pub(super) rows: Vec<PolynomialRow<N>>,
    pub(super) indices: [usize; N],
    pub(super) variable_count: usize,
    variables: Arc<Vec<PolyVariable>>,
    active_arity: usize,
    fixed: [Option<i16>; N],
    coefficient_priority: Vec<usize>,
    /// Conditions inherited from the family preparation, before denominator
    /// clearing. They are not rediscovered by scanning every seeded row.
    pub(super) conditions: Vec<CoefficientPolynomial>,
}

impl<const N: usize> SourceSystem<N> {
    pub fn new(rows: Vec<PolynomialRow<N>>, indices: [usize; N]) -> Result<Self, SolverError> {
        Self::new_with_fixed(rows, indices, [None; N])
    }

    /// Construct prepared sources with a common fixed-coordinate pattern.
    ///
    /// A fixed coordinate is an absolute numeric power in every term, not an
    /// offset to add to a seed. Its coefficient variable must already have
    /// been substituted. Every other coordinate remains a symbolic offset.
    /// An entirely empty input cannot supply the native coefficient variable
    /// map and is rejected; preparation of an existing system may yield zero
    /// rows without losing that metadata.
    pub fn new_with_fixed(
        rows: Vec<PolynomialRow<N>>,
        indices: [usize; N],
        fixed: [Option<i16>; N],
    ) -> Result<Self, SolverError> {
        let first = rows
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| SolverError::InvalidInput("the source system is empty".into()))?;
        let variables = first.coefficient.variables().clone();
        let variable_count = variables.len();
        let mut seen = vec![false; variable_count];
        for position in indices {
            if position >= variable_count || seen[position] {
                return Err(SolverError::InvalidInput(
                    "index-variable positions must be distinct and in range".into(),
                ));
            }
            seen[position] = true;
        }
        validate_prepared_rows(&rows, &indices, &variables, &fixed)?;
        let coefficient_priority = indices
            .iter()
            .copied()
            .chain((0..variable_count).filter(|i| !indices.contains(i)))
            .collect();
        Ok(Self {
            rows,
            indices,
            variable_count,
            variables,
            active_arity: N,
            fixed,
            coefficient_priority,
            conditions: Vec::new(),
        })
    }

    /// Generate ordinary and Lorentz-invariance identities, matching SpIRed's
    /// default `generateIBPs(true)`. Vacuum families have no LI rows.
    ///
    /// This uses the existing topology-generic exact generator and lowers
    /// each relation once to compact polynomial rows. Cut-derivative
    /// elimination is a separate preparation step, not performed here.
    pub fn from_family(family: &IntegralFamily) -> Result<Self, SolverError> {
        Self::from_family_with_lorentz(family, true)
    }

    /// As [`Self::from_family`], with explicit control of LI source inclusion.
    ///
    /// Ordinary rows follow the reference's differentiated-loop-major order:
    /// reversed external contractions, then loop contractions. LI rows follow
    /// the reference's reversed external-pair traversal. The underlying
    /// generic RustRed generator keeps its existing public row ordering.
    pub fn from_family_with_lorentz(
        family: &IntegralFamily,
        include_lorentz: bool,
    ) -> Result<Self, SolverError> {
        if !crate::fits_storage(family.denominator_count(), N) {
            return Err(SolverError::InvalidInput(format!(
                "expected {N} denominator coordinates, got {}",
                family.denominator_count()
            )));
        }
        Self::from_family_with_capacity(family, include_lorentz)
    }

    /// Prepare a physical family in larger inline storage. Extra coordinates
    /// are numeric zero, fixed before search, and never physical denominators.
    pub fn from_family_with_capacity(
        family: &IntegralFamily,
        include_lorentz: bool,
    ) -> Result<Self, SolverError> {
        let generator = ParametricIbpGenerator::try_new(family)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let context = generator.context();
        if context.index_count() == 0 || context.index_count() > N {
            return Err(SolverError::InvalidInput(format!(
                "capacity {N} cannot hold {} denominator coordinates",
                context.index_count()
            )));
        }
        let active_arity = context.index_count();
        let offset = context.base().parameter_names().len();
        let indices = std::array::from_fn(|i| offset + i);
        let batch = generator
            .prepare_ordinary_ibp()
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
        let completed = batch
            .complete(generated)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let lorentz = if include_lorentz {
            // The existing generator uses the loop Lorentz operator on an
            // ascending external pair. C++ differentiates external momenta
            // and visits the reversed pair. Total Lorentz invariance and
            // pair antisymmetry cancel both sign reversals: no extra minus.
            let batch = generator
                .prepare_lorentz_invariance(&completed)
                .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
            (0..batch.len())
                .map(|ordinal| batch.generate(ordinal))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| SolverError::InvalidInput(error.to_string()))?
        } else {
            Vec::new()
        };
        let mut relations = completed.into_relations();
        relations.extend(lorentz);
        relations.sort_unstable_by_key(|relation| {
            reference_source_order(
                relation.row_id(),
                family.loop_count(),
                family.external_count(),
            )
        });
        let mut rows = Vec::with_capacity(relations.len());
        let mut conditions = Vec::new();
        for relation in &relations {
            rows.push(lower_relation(relation, &mut conditions)?);
        }
        let mut fixed = [None; N];
        if active_arity < N {
            let variables = rows
                .iter()
                .flatten()
                .next()
                .ok_or_else(|| SolverError::InvalidInput("the source system is empty".into()))?
                .coefficient
                .variables();
            let mut padding = Vec::new();
            let mut temporary = 0;
            while padding.len() < N - active_arity {
                let variable = PolyVariable::Temporary(temporary);
                temporary += 1;
                if !variables.contains(&variable) {
                    padding.push(variable);
                }
            }
            for term in rows.iter_mut().flatten() {
                let mut powers = *term.integral.powers();
                for power in &mut powers[active_arity..] {
                    *power = Power::new(false, 0)?;
                }
                term.integral = super::Integral::new(powers);
                term.coefficient.add_variables(&padding);
            }
            for value in &mut fixed[active_arity..] {
                *value = Some(0);
            }
            for condition in &mut conditions {
                condition.add_variables(&padding);
            }
        }
        let mut system = Self::new_with_fixed(rows, indices, fixed)?;
        system.active_arity = active_arity;
        system.conditions = conditions;
        // The reference's coefficient priority is indices, dimension, scalars.
        // Do not mistake caller parameter registration order for that priority.
        let dimension = family.dimension();
        if dimension.denominator.is_one() && dimension.numerator.nterms() == 1 {
            let term = (&dimension.numerator).into_iter().next().unwrap();
            if term.coefficient.is_one()
                && term.exponents.iter().map(|x| u32::from(*x)).sum::<u32>() == 1
            {
                let position = term.exponents.iter().position(|x| *x == 1).unwrap();
                system.coefficient_priority = indices
                    .iter()
                    .copied()
                    .chain(std::iter::once(position))
                    .chain((0..offset).filter(|i| *i != position))
                    .collect();
            }
        }
        Ok(system)
    }

    /// Number of physical coordinates, independently of inline capacity N.
    pub fn active_arity(&self) -> usize {
        self.active_arity
    }

    /// Resize already prepared sources. This does not regenerate identities,
    /// precondition rows, or change their ordinals and physical variable map.
    #[cfg(feature = "capacity-dispatch")]
    pub(super) fn resize<const K: usize>(&self) -> Result<SourceSystem<K>, SolverError> {
        if self.active_arity > K {
            return Err(SolverError::InvalidInput(
                "solver capacity is smaller than its physical arity".into(),
            ));
        }
        let physical_variables = self.variables.len() - (N - self.active_arity);
        let mut variables = self.variables[..physical_variables].to_vec();
        let mut temporary = 0;
        while variables.len() < physical_variables + K - self.active_arity {
            let variable = PolyVariable::Temporary(temporary);
            temporary += 1;
            if !variables.contains(&variable) {
                variables.push(variable);
            }
        }
        let variables = Arc::new(variables);
        let indices = std::array::from_fn(|i| {
            if i < self.active_arity {
                self.indices[i]
            } else {
                physical_variables + i - self.active_arity
            }
        });
        let rows = self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|term| {
                        Ok(Term {
                            integral: super::capacity::integral(&term.integral)?,
                            coefficient: term
                                .coefficient
                                .rearrange_with_growth(&variables)
                                .map_err(SolverError::InvalidInput)?,
                        })
                    })
                    .collect()
            })
            .collect::<Result<Vec<_>, SolverError>>()?;
        let fixed = std::array::from_fn(|i| {
            if i < self.active_arity {
                self.fixed[i]
            } else {
                Some(0)
            }
        });
        let conditions = self
            .conditions
            .iter()
            .map(|p| {
                p.rearrange_with_growth(&variables)
                    .map_err(SolverError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let coefficient_priority = self
            .coefficient_priority
            .iter()
            .copied()
            .filter(|&i| i < physical_variables)
            .chain(physical_variables..variables.len())
            .collect();
        Ok(SourceSystem {
            rows,
            indices,
            variable_count: variables.len(),
            variables,
            active_arity: self.active_arity,
            fixed,
            coefficient_priority,
            conditions,
        })
    }

    /// Remove unused padding variables while rejecting any dependence on them.
    pub fn physical_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<Coefficient, SolverError> {
        let variables = &self.variables[..self.variables.len() - (N - self.active_arity)];
        Ok(Coefficient::from_num_den(
            coefficient
                .numerator
                .rearrange_with_growth(variables)
                .map_err(SolverError::InvalidInput)?,
            coefficient
                .denominator
                .rearrange_with_growth(variables)
                .map_err(SolverError::InvalidInput)?,
            &symbolica::domains::integer::Z,
            true,
        ))
    }

    pub fn rows(&self) -> &[PolynomialRow<N>] {
        &self.rows
    }
    pub fn index_variables(&self) -> &[usize; N] {
        &self.indices
    }
    /// Shared native coefficient variables, including physical parameters and
    /// symbolic indices, retained even when preparation removes every row.
    pub fn coefficient_variables(&self) -> &[PolyVariable] {
        &self.variables
    }
    pub fn fixed(&self) -> &[Option<i16>; N] {
        &self.fixed
    }
    pub fn conditions(&self) -> &[CoefficientPolynomial] {
        &self.conditions
    }

    pub(super) fn coefficient_order(&self) -> &[usize] {
        &self.coefficient_priority
    }

    /// Replace sources after exact preparation without changing their native
    /// variable map, ordering priority, or inherited nonzero conditions.
    /// Empty prepared systems are valid and retain their original metadata.
    pub(super) fn replace_prepared_rows(
        mut self,
        rows: Vec<PolynomialRow<N>>,
        fixed: [Option<i16>; N],
    ) -> Result<Self, SolverError> {
        validate_prepared_rows(&rows, &self.indices, &self.variables, &fixed)?;
        self.rows = rows;
        self.fixed = fixed;
        Ok(self)
    }
}

fn validate_prepared_rows<const N: usize>(
    rows: &[PolynomialRow<N>],
    indices: &[usize; N],
    variables: &Arc<Vec<PolyVariable>>,
    fixed: &[Option<i16>; N],
) -> Result<(), SolverError> {
    for value in fixed.iter().flatten() {
        Power::new(false, *value)?;
    }
    for term in rows.iter().flatten() {
        if term.coefficient.variables() != variables {
            return Err(SolverError::InvalidInput(
                "source coefficients must share the original variable map".into(),
            ));
        }
        for (axis, power) in term.integral.powers().iter().enumerate() {
            if let Some(value) = fixed[axis] {
                if power.is_symbolic() || power.value() != value {
                    return Err(SolverError::InvalidInput(format!(
                        "prepared source coordinate {axis} must have absolute numeric power {value}"
                    )));
                }
                if term.coefficient.contains(indices[axis]) {
                    return Err(SolverError::InvalidInput(format!(
                        "prepared source coefficient still depends on fixed coordinate {axis}"
                    )));
                }
            } else if !power.is_symbolic() {
                return Err(SolverError::InvalidInput(format!(
                    "free source coordinate {axis} must have a symbolic integral index"
                )));
            }
        }
    }
    Ok(())
}

/// Adapter-only scheduling key. Rust external axis i maps to C++ p(i+1),
/// encoded as -(i+1), so increasing signed indices visit names in reverse.
fn reference_source_order(row: &RowId, loops: usize, externals: usize) -> (usize, usize, usize) {
    match *row {
        RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => (
            0,
            differentiated_loop,
            if contraction_momentum < loops {
                externals + contraction_momentum
            } else {
                externals - 1 - (contraction_momentum - loops)
            },
        ),
        RowId::LorentzInvariance {
            first_external,
            second_external,
        } => (
            1,
            externals - 1 - second_external,
            externals - 1 - first_external,
        ),
        RowId::Derived { .. } => {
            unreachable!("fresh ordinary/LI preparation cannot emit a derived source")
        }
    }
}

/// One shared native-polynomial lowering path for ordinary and LI relations.
/// Retain zero rows too: the reference keeps their source ordinals, and sector
/// preconditioning already handles empty rows.
fn lower_relation<const N: usize>(
    relation: &ParametricRelation,
    conditions: &mut Vec<CoefficientPolynomial>,
) -> Result<PolynomialRow<N>, SolverError> {
    for condition in relation.nonzero_conditions() {
        let polynomial = condition.polynomial().raw();
        if !polynomial.is_constant() && !conditions.contains(polynomial) {
            conditions.push(polynomial.clone());
        }
    }
    let mut row = Vec::with_capacity(relation.terms().len());
    for (shift, coefficient) in relation.terms() {
        let mut powers = [0_i16; N];
        for (out, value) in powers.iter_mut().zip(shift.values()) {
            *out = i16::try_from(*value).map_err(|_| {
                SolverError::InvalidInput("source shift exceeds compact range".into())
            })?;
        }
        row.push(Term {
            integral: Integral::symbolic(powers)?,
            coefficient: coefficient.raw().clone(),
        });
    }
    super::row::clear_denominators(row)
}

#[cfg(test)]
mod tests;
