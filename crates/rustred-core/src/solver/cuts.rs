//! Exact, once-per-family removal of derivatives of independent linear cuts.
//!
//! The exported pre-rules apply away from cut power one. The prepared source
//! frame instead lives *at* power one; these are different domains, not two
//! interchangeable presentations of an unconditional rule.

mod admission;

use std::collections::BTreeMap;
use std::fmt;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, IntegerRing, Z};

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;

use super::instantiate::{canonicalize, translate};
use super::row::clear_denominators;
use super::{ExactRow, Integral, Power, SolverError, SourceSystem, Term};

#[derive(Debug)]
pub enum LinearCutError {
    Unsupported { axis: usize, reason: &'static str },
    Source(SolverError),
}

impl fmt::Display for LinearCutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { axis, reason } => {
                write!(f, "unsupported linear cut {axis}: {reason}")
            }
            Self::Source(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for LinearCutError {}
impl From<SolverError> for LinearCutError {
    fn from(error: SolverError) -> Self {
        Self::Source(error)
    }
}
impl From<super::PowerError> for LinearCutError {
    fn from(error: super::PowerError) -> Self {
        Self::Source(error.into())
    }
}

/// Canonical equation `I(n) = rhs`, valid only when `n[axis] != 1` and the
/// prepared source system's inherited parameter conditions are nonzero.
/// The index guard is explicit even if the RHS vanishes or its poles cancel.
#[derive(Debug)]
pub struct LinearCutRule<const N: usize> {
    pub axis: usize,
    /// Ordinal in the original ordinary-plus-LI source frame. The prepared
    /// frame drops zero rows, so this is not an index into `sources.rows()`.
    pub source_ordinal: usize,
    pub target: Integral<N>,
    pub rhs: ExactRow<N>,
}

#[derive(Debug)]
pub struct LinearCutPreparation<const N: usize> {
    pub sources: SourceSystem<N>,
    pub rules: Vec<LinearCutRule<N>>,
}

/// Generate and improve sources for independent, unshifted linear cuts.
/// All coefficient operations use native Symbolica arithmetic. No reference
/// rule, numerical sample, or topology name participates in this preparation.
pub fn prepare_linear_cuts<const N: usize>(
    family: &IntegralFamily,
    removed: [bool; N],
    include_lorentz: bool,
) -> Result<LinearCutPreparation<N>, LinearCutError> {
    if family.denominator_count() != N {
        return Err(SolverError::InvalidInput("linear-cut mask has the wrong size".into()).into());
    }
    let contractions = admission::contractions(family, &removed)?;
    let mut system = SourceSystem::from_family_with_lorentz(family, include_lorentz)?;
    let mut rules = Vec::with_capacity(contractions.len());
    for (axis, source_ordinal) in contractions {
        let mut pivot_shift = [0; N];
        pivot_shift[axis] = 1;
        let pivot_key = Integral::symbolic(pivot_shift)?;
        let mut row: ExactRow<N> = system.rows[source_ordinal]
            .iter()
            .map(|term| Term {
                integral: term.integral,
                coefficient: term.coefficient.clone().into(),
            })
            .collect();
        let Some(pivot_position) = row.iter().position(|term| term.integral == pivot_key) else {
            return Err(unsupported(axis, "ordinary identity has no +1 cut pivot"));
        };
        row.swap(0, pivot_position);
        let pivot = &row[0].coefficient.numerator;
        let variable = system.indices[axis];
        // Admission implies pivot = n_i times a nonzero parameter polynomial.
        // Verify this at the representation boundary and retain that factor
        // even if subsequent exact simplification cancels it from every RHS.
        if pivot.degree(variable) != 1
            || !pivot.replace(variable, &Integer::from(0)).is_zero()
            || system
                .indices
                .iter()
                .enumerate()
                .any(|(other, index)| other != axis && pivot.contains(*index))
        {
            return Err(unsupported(
                axis,
                "cut pivot is not a parameter multiple of its index",
            ));
        }
        let condition = pivot.replace(variable, &Integer::from(1));
        if !condition.is_constant() && !system.conditions.contains(&condition) {
            system.conditions.push(condition);
        }
        let (target, rhs) = canonicalize(row, &system.indices)?;
        if rhs.iter().any(|term| {
            term.integral[axis].value() >= 0
                || removed
                    .iter()
                    .enumerate()
                    .any(|(i, cut)| *cut && term.integral[i].value() > 0)
        }) {
            return Err(unsupported(
                axis,
                "replacement can reintroduce a removed cut derivative",
            ));
        }
        rules.push(LinearCutRule {
            axis,
            source_ordinal,
            target,
            rhs,
        });
    }
    if rules.is_empty() {
        return Ok(LinearCutPreparation {
            sources: system,
            rules,
        });
    }
    let mut rows = Vec::with_capacity(system.rows.len());
    for original in &system.rows {
        let mut row: ExactRow<N> = original
            .iter()
            .map(|term| Term {
                integral: term.integral,
                coefficient: term.coefficient.clone().into(),
            })
            .collect();
        for rule in &rules {
            let mut merged = BTreeMap::new();
            for term in row {
                if term.integral[rule.axis].value() > 0 {
                    let shifts = std::array::from_fn(|i| term.integral[i].value());
                    for replacement in &rule.rhs {
                        let integral = replacement.integral.shifted(shifts)?;
                        let coefficient = &term.coefficient
                            * &translate(&replacement.coefficient, &system.indices, &shifts);
                        accumulate(&mut merged, integral, coefficient);
                    }
                } else {
                    accumulate(&mut merged, term.integral, term.coefficient);
                }
            }
            row = merged
                .into_iter()
                .filter(|(_, coefficient)| !coefficient.is_zero())
                .map(|(integral, coefficient)| Term {
                    integral,
                    coefficient,
                })
                .collect();
        }
        let mut specialized = Vec::with_capacity(row.len());
        for term in row {
            let mut powers = *term.integral.powers();
            let mut vanishes = false;
            for (axis, cut) in removed.iter().enumerate() {
                if !cut {
                    continue;
                }
                if powers[axis].value() > 0 {
                    return Err(unsupported(
                        axis,
                        "prepared row still contains a cut derivative",
                    ));
                }
                if powers[axis].value() < 0 {
                    vanishes = true;
                    break;
                }
                powers[axis] = Power::new(false, 1)?;
            }
            if vanishes {
                continue;
            }
            let mut numerator = term.coefficient.numerator;
            let mut denominator = term.coefficient.denominator;
            for (axis, cut) in removed.iter().enumerate() {
                if *cut {
                    numerator = numerator.replace(system.indices[axis], &Integer::from(1));
                    denominator = denominator.replace(system.indices[axis], &Integer::from(1));
                    if denominator.is_zero() {
                        return Err(unsupported(
                            axis,
                            "replacement has a pole on the fixed cut frame",
                        ));
                    }
                }
            }
            if !numerator.is_zero() {
                let coefficient = <Coefficient as FromNumeratorAndDenominator<
                    IntegerRing,
                    IntegerRing,
                    u16,
                >>::from_num_den(numerator, denominator, &Z, true);
                specialized.push(Term {
                    integral: Integral::new(powers),
                    coefficient,
                });
            }
        }
        if !specialized.is_empty() {
            rows.push(clear_denominators(specialized)?);
        }
    }
    let sources = system.replace_prepared_rows(rows, removed.map(|cut| cut.then_some(1)))?;
    Ok(LinearCutPreparation { sources, rules })
}

fn accumulate<const N: usize>(
    row: &mut BTreeMap<Integral<N>, Coefficient>,
    integral: Integral<N>,
    coefficient: Coefficient,
) {
    row.entry(integral)
        .and_modify(|current| *current = &*current + &coefficient)
        .or_insert(coefficient);
}

fn unsupported(axis: usize, reason: &'static str) -> LinearCutError {
    LinearCutError::Unsupported { axis, reason }
}

#[cfg(test)]
mod tests;
