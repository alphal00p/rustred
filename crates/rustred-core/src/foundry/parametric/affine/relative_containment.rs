//! Whole-exclusion containment relative to an affine target and an exact box.
//! Only native consistent equal-rank augmentation proves implication. This is
//! a sufficient rational-affine proof for integer points, not a lattice chart.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use symbolica::prelude::Integer;

use crate::{
    algebra::{
        CoefficientPolynomial, IndexedGuardLimits,
        indexed::{ceil_log2, integer_magnitude_bits},
    },
    foundry::completion::LatticeBox,
    solver::canonical_equalities,
};

use super::{
    AffineApplicationDomain,
    box_bounds::{MAX_MATRIX_CELLS, MAX_POLYNOMIAL_CELLS},
};

const MAX_EQUATIONS: usize = 32;
type Proof = Result<bool, &'static str>;

impl AffineApplicationDomain {
    /// Prove `box AND self` lies in ONE complete exclusion. The caller binds
    /// the original predicates to its source context. Every original sibling
    /// is admitted before any proof; cached matrices/charts are not authority.
    /// Resource failures identify an exhausted resource; unsupported inputs or
    /// native errors/panics are inconclusive. Limits are shared by this query,
    /// never reset for an exclusion. No altered predicate escapes this method.
    pub(crate) fn is_proved_excluded_in_box(
        &self,
        cell: &LatticeBox,
        exclusions: &[Arc<Self>],
        limits: IndexedGuardLimits,
    ) -> Proof {
        self.excluded_using(cell, exclusions, limits, |fixed, equations, indices| {
            canonical_equalities(fixed, equations, indices)
                .map(|result| result.map(|(matrix, _)| matrix.nrows()))
                .map_err(|_| ())
        })
    }

    fn excluded_using<F>(
        &self,
        cell: &LatticeBox,
        exclusions: &[Arc<Self>],
        limits: IndexedGuardLimits,
        mut native: F,
    ) -> Proof
    where
        F: FnMut(&[Option<i16>], &[CoefficientPolynomial], &[usize]) -> Result<Option<usize>, ()>,
    {
        catch_unwind(AssertUnwindSafe(|| {
            let n = cell.arity();
            let Some(template) = self.equations.first() else {
                return Ok(false);
            };
            if n == 0
                || self.sector.len() != n
                || self.indices.len() != n
                || exclusions.is_empty()
                || self.indices.iter().enumerate().any(|(axis, &index)| {
                    index >= template.nvars() || self.indices[..axis].contains(&index)
                })
            {
                return Ok(false);
            }
            let columns = n.checked_add(1).ok_or("relative affine matrix cells")?;
            let mut work = Work::default();
            let mut cells = 0usize;
            let mut terms = 0usize;
            let mut original_bits = 0usize;
            let mut prospective_bits = 0usize;
            let mut entry_bits = 1usize;
            // No native substitution, row allocation, or successful shortcut
            // precedes full original support/map/shape/bit admission.
            for (ordinal, domain) in std::iter::once(self)
                .chain(exclusions.iter().map(Arc::as_ref))
                .enumerate()
            {
                if domain.sector != self.sector
                    || domain.indices != self.indices
                    || domain.fixed.len() != n
                    || domain.equations.is_empty()
                    || domain
                        .fixed
                        .iter()
                        .zip(&domain.sector)
                        .any(|(value, &active)| value.is_some_and(|v| (v > 0) != active))
                {
                    return Ok(false);
                }
                let rows = (if ordinal == 0 {
                    0usize
                } else {
                    self.equations.len()
                })
                .checked_add(domain.equations.len())
                .ok_or("relative affine equations")?;
                check(
                    rows,
                    MAX_EQUATIONS.min(limits.max_coefficient_equations),
                    "relative affine equations",
                )?;
                check(
                    rows.checked_mul(columns)
                        .ok_or("relative affine matrix cells")?,
                    MAX_MATRIX_CELLS,
                    "relative affine matrix cells",
                )?;
                for equation in &domain.equations {
                    if equation.variables() != template.variables()
                        || equation.coefficients.len().checked_mul(equation.nvars())
                            != Some(equation.exponents.len())
                    {
                        return Ok(false);
                    }
                    add(
                        &mut cells,
                        equation.exponents.len(),
                        MAX_POLYNOMIAL_CELLS,
                        "relative affine polynomial cells",
                    )?;
                    add(
                        &mut terms,
                        equation.nterms(),
                        limits.max_input_terms,
                        "relative affine input terms",
                    )?;
                    let mut maximum = 1usize;
                    for (term, coefficient) in equation.coefficients.iter().enumerate() {
                        let bits = usize::try_from(integer_magnitude_bits(coefficient))
                            .map_err(|_| "relative affine integer bits")?;
                        add(
                            &mut original_bits,
                            bits,
                            limits.max_total_integer_bits,
                            "relative affine integer bits",
                        )?;
                        let mut variable = None;
                        for (position, &power) in equation.exponents(term).iter().enumerate() {
                            if power == 0 {
                                continue;
                            }
                            if power != 1 || variable.is_some() {
                                return Ok(false);
                            }
                            let Some(axis) = self.indices.iter().position(|&i| i == position)
                            else {
                                return Ok(false);
                            };
                            variable = Some(axis);
                        }
                        let endpoint_bits = variable
                            .filter(|&axis| singleton(cell, axis))
                            .map_or(0, |axis| {
                                physical_bits(cell.lower()[axis], self.sector[axis])
                            });
                        maximum = maximum.max(
                            bits.checked_add(endpoint_bits)
                                .ok_or("relative affine integer bits")?,
                        );
                    }
                    // Affine scalar substitution cannot increase term count.
                    // All merged constants are bounded without cancellation.
                    let bound = maximum
                        .checked_add(ceil_log2(equation.nterms().max(1)))
                        .ok_or("relative affine integer bits")?;
                    entry_bits = entry_bits.max(bound);
                    add(
                        &mut prospective_bits,
                        equation
                            .nterms()
                            .checked_mul(bound)
                            .ok_or("relative affine integer bits")?,
                        limits.max_total_integer_bits,
                        "relative affine integer bits",
                    )?;
                }
            }
            work.charge(cells, limits)?;
            if !fixed_face_holds(self, cell) {
                return Ok(false);
            }
            let count = (0..n).filter(|&axis| singleton(cell, axis)).count();
            // Charge before allocating native integers, cloned polynomials or
            // matrices. Positive local u64::MAX becomes exact 2^64, not zero.
            let mut equations = materialize(self, cell, count, entry_bits, limits, &mut work)?;
            let fixed = vec![None; n];
            work.matrix(equations.len(), columns, entry_bits, limits)?;
            let base_rank = match native(&fixed, &equations, &self.indices) {
                Ok(None) => return Ok(true),
                Ok(Some(rank)) => rank,
                Err(()) => return Ok(false),
            };
            let base_len = equations.len();
            for exclusion in exclusions {
                if !fixed_face_holds(exclusion, cell) {
                    continue;
                }
                let extra = materialize(exclusion, cell, count, entry_bits, limits, &mut work)?;
                work.matrix(base_len + extra.len(), columns, entry_bits, limits)?;
                equations.extend(extra);
                let result = native(&fixed, &equations, &self.indices);
                equations.truncate(base_len);
                match result {
                    Ok(Some(rank)) if rank == base_rank => return Ok(true),
                    // An inconsistent augmentation means the exclusion misses
                    // the target; it does NOT prove relative containment.
                    Ok(_) => (),
                    Err(()) => return Ok(false),
                }
            }
            Ok(false)
        }))
        .unwrap_or(Ok(false))
    }
}

fn materialize(
    domain: &AffineApplicationDomain,
    cell: &LatticeBox,
    singleton_count: usize,
    bits: usize,
    limits: IndexedGuardLimits,
    work: &mut Work,
) -> Result<Vec<CoefficientPolynomial>, &'static str> {
    let terms = domain
        .equations
        .iter()
        .try_fold(0usize, |n, e| n.checked_add(e.nterms()))
        .ok_or("relative affine replay terms")?;
    let substitutions = domain
        .equations
        .len()
        .checked_mul(singleton_count)
        .ok_or("relative affine substitutions")?;
    add(
        &mut work.substitutions,
        substitutions,
        limits.max_exact_hyperplane_replay_substitutions,
        "relative affine substitutions",
    )?;
    let traversed = terms
        .checked_mul(singleton_count + 1)
        .ok_or("relative affine replay terms")?;
    add(
        &mut work.terms,
        traversed,
        limits.max_exact_hyperplane_replay_terms,
        "relative affine replay terms",
    )?;
    work.charge(
        traversed
            .checked_mul(bits.max(1))
            .ok_or("relative affine work")?,
        limits,
    )?;
    let mut equations = domain.equations.to_vec();
    for axis in 0..cell.arity() {
        if singleton(cell, axis) {
            let local = Integer::from(cell.lower()[axis]);
            let value = if domain.sector[axis] {
                local + Integer::one()
            } else {
                -local
            };
            for equation in &mut equations {
                *equation = equation.replace(domain.indices[axis], &value);
            }
        }
    }
    Ok(equations)
}

#[derive(Default)]
struct Work {
    operations: usize,
    substitutions: usize,
    terms: usize,
}

impl Work {
    fn charge(&mut self, amount: usize, limits: IndexedGuardLimits) -> Result<(), &'static str> {
        add(
            &mut self.operations,
            amount,
            limits.max_exact_hyperplane_replay_work,
            "relative affine work",
        )
    }

    fn matrix(
        &mut self,
        rows: usize,
        columns: usize,
        bits: usize,
        limits: IndexedGuardLimits,
    ) -> Result<(), &'static str> {
        let cells = rows
            .checked_mul(columns)
            .ok_or("relative affine matrix cells")?;
        check(cells, MAX_MATRIX_CELLS, "relative affine matrix cells")?;
        let dimension = rows.min(columns).max(1);
        // Integer minors have <= d*(B+ceil(log2 d))+1 bits. Admit rational
        // numerator/denominator products and carries conservatively as well.
        let temporary = bits
            .checked_add(ceil_log2(dimension))
            .and_then(|v| v.checked_mul(dimension))
            .and_then(|v| v.checked_add(1))
            .and_then(|v| v.checked_mul(8))
            .and_then(|v| v.checked_add(4))
            .ok_or("relative affine matrix bits")?;
        check(
            cells
                .checked_mul(temporary)
                .ok_or("relative affine matrix bits")?,
            limits.max_total_integer_bits,
            "relative affine matrix bits",
        )?;
        let limbs = temporary.div_ceil(64);
        self.charge(
            cells
                .checked_mul(dimension)
                .and_then(|v| v.checked_mul(limbs))
                .and_then(|v| v.checked_mul(limbs))
                .ok_or("relative affine work")?,
            limits,
        )
    }
}

fn fixed_face_holds(domain: &AffineApplicationDomain, cell: &LatticeBox) -> bool {
    domain.fixed.iter().enumerate().all(|(axis, value)| {
        value.is_none_or(|value| {
            let local = if domain.sector[axis] {
                value as u64 - 1
            } else {
                u64::from(value.unsigned_abs())
            };
            singleton(cell, axis) && cell.lower()[axis] == local
        })
    })
}

fn singleton(cell: &LatticeBox, axis: usize) -> bool {
    cell.upper()[axis] == Some(cell.lower()[axis])
}
fn physical_bits(local: u64, active: bool) -> usize {
    if active {
        local
            .checked_add(1)
            .map_or(65, |v| (u64::BITS - v.leading_zeros()) as usize)
    } else {
        (u64::BITS - local.leading_zeros()) as usize
    }
}
fn check(value: usize, limit: usize, resource: &'static str) -> Result<(), &'static str> {
    if value > limit { Err(resource) } else { Ok(()) }
}
fn add(
    value: &mut usize,
    amount: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), &'static str> {
    *value = value.checked_add(amount).ok_or(resource)?;
    check(*value, limit, resource)
}

#[cfg(test)]
#[path = "relative_containment/tests.rs"]
mod tests;
