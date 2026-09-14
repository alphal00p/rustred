use crate::algebra::CoefficientPolynomial;
use crate::family::IntegralFamily;
use crate::identity::{ParametricIbpGenerator, ParametricRelation, RowId};

use super::{Integral, PolynomialRow, SolverError, Term};

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
    coefficient_priority: Vec<usize>,
    /// Conditions inherited from the family preparation, before denominator
    /// clearing. They are not rediscovered by scanning every seeded row.
    pub(super) conditions: Vec<CoefficientPolynomial>,
}

impl<const N: usize> SourceSystem<N> {
    pub fn new(rows: Vec<PolynomialRow<N>>, indices: [usize; N]) -> Result<Self, SolverError> {
        let first = rows
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| SolverError::InvalidInput("the source system is empty".into()))?;
        let variables = first.coefficient.variables();
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
        for term in rows.iter().flatten() {
            if term.coefficient.variables() != variables {
                return Err(SolverError::InvalidInput(
                    "source coefficients must share a variable map".into(),
                ));
            }
            if term.integral.powers().iter().any(|p| !p.is_symbolic()) {
                return Err(SolverError::InvalidInput(
                    "source templates must have symbolic integral indices".into(),
                ));
            }
        }
        let coefficient_priority = indices
            .iter()
            .copied()
            .chain((0..variable_count).filter(|i| !indices.contains(i)))
            .collect();
        Ok(Self {
            rows,
            indices,
            variable_count,
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
        let generator = ParametricIbpGenerator::try_new(family)
            .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        let context = generator.context();
        if context.index_count() != N {
            return Err(SolverError::InvalidInput(format!(
                "expected {N} denominator coordinates, got {}",
                context.index_count()
            )));
        }
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
        let mut system = Self::new(rows, indices)?;
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

    pub fn rows(&self) -> &[PolynomialRow<N>] {
        &self.rows
    }
    pub fn index_variables(&self) -> &[usize; N] {
        &self.indices
    }
    pub fn conditions(&self) -> &[CoefficientPolynomial] {
        &self.conditions
    }

    pub(super) fn coefficient_order(&self) -> &[usize] {
        &self.coefficient_priority
    }
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
    let Some((_, first)) = relation.terms().first_key_value() else {
        return Ok(Vec::new());
    };
    let mut denominator = first.raw().denominator.one();
    for coefficient in relation.terms().values() {
        let d = &coefficient.raw().denominator;
        let gcd = denominator.gcd(d);
        denominator = &denominator
            * &d.try_div_exact(&gcd)
                .expect("native polynomial GCD divides the denominator");
    }
    let mut row = Vec::with_capacity(relation.terms().len());
    for (shift, coefficient) in relation.terms() {
        let mut powers = [0_i16; N];
        for (out, value) in powers.iter_mut().zip(shift.values()) {
            *out = i16::try_from(*value).map_err(|_| {
                SolverError::InvalidInput("source shift exceeds compact range".into())
            })?;
        }
        let scale = denominator
            .try_div_exact(&coefficient.raw().denominator)
            .expect("common denominator is exactly divisible");
        row.push(Term {
            integral: Integral::symbolic(powers)?,
            coefficient: &coefficient.raw().numerator * &scale,
        });
    }
    Ok(row)
}

#[cfg(test)]
mod tests;
